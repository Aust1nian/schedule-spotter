//! Model layer: llama.cpp in process, used only as a tiebreaker on single
//! closed questions the script layer hands it (spec tasks T1–T5).
//!
//! Every call is forced into a fixed JSON shape by a grammar built from the
//! task's schema, so malformed output is impossible. Calls run greedy
//! (temperature 0); one over the time limit counts as unsure. Answers are
//! cached per question. Cell text is passed as quoted JSON data, never as
//! instructions. A model file whose SHA-256 doesn't match its pin is not
//! loaded, and then every question goes to the owner.

mod prompts;

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use spotter_core::matching::{TextEntry, Tiebreaker, Verdict};

pub use prompts::Task;

/// Status line shown in the app.
pub fn status_line(loaded: bool) -> &'static str {
    if loaded {
        "Smart matching ready"
    } else {
        "Smart matching off, I'll ask you instead"
    }
}

/// Context capped at 2,048 tokens to keep the KV cache small (spec).
const N_CTX: u32 = 2048;
/// A call that takes longer counts as unsure (spec).
pub const CALL_LIMIT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum LoadError {
    Missing,
    HashMismatch,
    Llama(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Missing => write!(f, "model file not found"),
            LoadError::HashMismatch => write!(f, "model file doesn't match its checksum"),
            LoadError::Llama(e) => write!(f, "model failed to load: {e}"),
        }
    }
}

fn backend() -> &'static LlamaBackend {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        let mut b = LlamaBackend::init().expect("llama backend initializes once");
        b.void_logs();
        b
    })
}

/// Physical cores minus one, capped at 4 (spec), unless overridden.
pub fn default_threads() -> i32 {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    // available_parallelism counts logical CPUs; assume two per core above 4.
    let physical = if n > 4 { n / 2 } else { n };
    (physical.saturating_sub(1)).clamp(1, 4) as i32
}

/// SHA-256 of a file, streamed.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 8 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

struct Inner {
    ctx: LlamaContext<'static>,
    /// Tokens currently in the KV cache (sequence 0), for prefix reuse.
    cached: Vec<LlamaToken>,
    /// Answers already given, by task and question.
    answers: HashMap<String, Option<Value>>,
}

/// A loaded model, ready to answer tiebreaker questions.
pub struct Engine {
    model: &'static LlamaModel,
    template: LlamaChatTemplate,
    template_text: String,
    prompts: prompts::Prompts,
    grammars: HashMap<Task, String>,
    inner: Mutex<Inner>,
    limit: Duration,
    /// Seconds from start of load (hash + weights + context).
    pub load_secs: f64,
}

// SAFETY: the llama context is only ever used behind the mutex, one call at
// a time; the model is read-only after loading.
unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    /// Checks the file against `sha256` (skipped when `None`, for tests),
    /// loads it, and prepares a context with `threads` threads.
    pub fn load(
        path: &Path,
        sha256: Option<&str>,
        variant: &str,
        threads: i32,
    ) -> Result<Engine, LoadError> {
        let started = Instant::now();
        if !path.exists() {
            return Err(LoadError::Missing);
        }
        if let Some(expected) = sha256 {
            let actual = sha256_file(path).map_err(|e| LoadError::Llama(e.to_string()))?;
            if actual != expected {
                return Err(LoadError::HashMismatch);
            }
        }
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(backend(), path, &params)
            .map_err(|e| LoadError::Llama(e.to_string()))?;
        let model: &'static LlamaModel = Box::leak(Box::new(model));
        let template = model
            .chat_template(None)
            .map_err(|e| LoadError::Llama(e.to_string()))?;
        let template_text = template.to_str().map(str::to_string).unwrap_or_default();
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(N_CTX))
            .with_n_batch(512)
            .with_n_threads(threads)
            .with_n_threads_batch(threads);
        let ctx = model
            .new_context(backend(), ctx_params)
            .map_err(|e| LoadError::Llama(e.to_string()))?;
        let prompts = prompts::Prompts::for_variant(variant);
        let mut grammars = HashMap::new();
        for task in Task::ALL {
            let g = llama_cpp_2::json_schema_to_grammar(&task.schema().to_string())
                .map_err(|e| LoadError::Llama(format!("grammar for {task:?}: {e}")))?;
            grammars.insert(task, g);
        }
        Ok(Engine {
            model,
            template,
            template_text,
            prompts,
            grammars,
            inner: Mutex::new(Inner {
                ctx,
                cached: Vec::new(),
                answers: HashMap::new(),
            }),
            limit: CALL_LIMIT,
            load_secs: started.elapsed().as_secs_f64(),
        })
    }

    /// Changes the per-call time limit (benchmarks measure without it).
    pub fn with_limit(mut self, limit: Duration) -> Engine {
        self.limit = limit;
        self
    }

    /// Asks one question. `None` when the call failed or ran over the limit.
    pub fn ask(&self, task: Task, question: &Value) -> Option<Value> {
        let key = format!("{task:?}:{question}");
        let mut inner = self.inner.lock().ok()?;
        if let Some(cached) = inner.answers.get(&key) {
            return cached.clone();
        }
        let answer = self.complete(&mut inner, task, question);
        inner.answers.insert(key, answer.clone());
        answer
    }

    fn complete(&self, inner: &mut Inner, task: Task, question: &Value) -> Option<Value> {
        let started = Instant::now();
        let p = self.prompts.get(task);
        let mut chat = vec![LlamaChatMessage::new("system".into(), p.system.clone()).ok()?];
        for (user, assistant) in &p.examples {
            chat.push(LlamaChatMessage::new("user".into(), user.clone()).ok()?);
            chat.push(LlamaChatMessage::new("assistant".into(), assistant.clone()).ok()?);
        }
        chat.push(LlamaChatMessage::new("user".into(), question.to_string()).ok()?);
        let prompt = match self.model.apply_chat_template(&self.template, &chat, true) {
            Ok(p) => p,
            // llama.cpp's built-in template engine doesn't know every new
            // format yet; render the ones we've verified ourselves.
            Err(e) => match render_known(
                &self.template_text,
                &p.system,
                &p.examples,
                &question.to_string(),
            ) {
                Some(p) => p,
                None => {
                    debug(&format!("chat template failed: {e}"));
                    return None;
                }
            },
        };
        let vocab = self.model.vocab();
        let mut tokens = vocab.tokenize(prompt.as_bytes(), false, true);
        if vocab.should_add_bos() && tokens.first() != Some(&vocab.bos()) {
            tokens.insert(0, vocab.bos());
        }
        if tokens.len() + task.max_tokens() >= N_CTX as usize {
            return None;
        }

        // Reuse the cached prefix (system prompt and examples) when the task
        // repeats; keep at least one token to evaluate for fresh logits.
        let common = inner
            .cached
            .iter()
            .zip(&tokens)
            .take_while(|(a, b)| a == b)
            .count()
            .min(tokens.len() - 1);
        inner
            .ctx
            .clear_kv_cache_seq(Some(0), Some(common as u32), None)
            .ok()?;
        inner.cached.truncate(common);
        // Prompt in chunks of the batch size; logits only for the last token.
        let mut batch = LlamaBatch::new(512, 1);
        let fresh = &tokens[common..];
        for (n, chunk) in fresh.chunks(512).enumerate() {
            batch.clear();
            for (i, t) in chunk.iter().enumerate() {
                let at = common + n * 512 + i;
                batch
                    .add(*t, at as i32, &[0], at == tokens.len() - 1)
                    .ok()?;
            }
            if let Err(e) = inner.ctx.decode(&mut batch) {
                debug(&format!("decode failed: {e}"));
                return None;
            }
        }
        inner.cached.extend_from_slice(fresh);

        let grammar = LlamaSampler::grammar(self.model, &self.grammars[&task], "root").ok()?;
        let mut sampler = LlamaSampler::chain_simple([grammar, LlamaSampler::greedy()]);
        let mut out: Vec<u8> = Vec::new();
        let mut idx = batch.n_tokens() - 1;
        for pos in (tokens.len() as i32..).take(task.max_tokens()) {
            if started.elapsed() > self.limit {
                return None;
            }
            let token = sampler.sample(&inner.ctx, idx);
            if vocab.is_eog(token) {
                break;
            }
            out.extend(vocab.token_to_piece(token, false, None));
            if let Ok(text) = std::str::from_utf8(&out)
                && let Ok(v) = serde_json::from_str::<Value>(text.trim())
                && v.is_object()
            {
                debug(&format!("{task:?} {question} -> {v}"));
                return Some(v);
            }
            batch.clear();
            batch.add(token, pos, &[0], true).ok()?;
            inner.ctx.decode(&mut batch).ok()?;
            inner.cached.push(token);
            idx = 0;
        }
        debug(&format!(
            "{task:?} unparsed output: {}",
            String::from_utf8_lossy(&out)
        ));
        serde_json::from_slice::<Value>(&out)
            .ok()
            .filter(Value::is_object)
    }

    fn verdict(&self, task: Task, question: Value) -> Verdict {
        match self
            .ask(task, &question)
            .as_ref()
            .and_then(|v| v["answer"].as_str())
        {
            Some("same") => Verdict::Same,
            Some("different") => Verdict::Different,
            _ => Verdict::Unsure,
        }
    }
}

/// Chat formats llama.cpp's template engine doesn't cover yet. Gemma 4:
/// `<|turn>role\n…<turn|>\n`, the assistant role named "model", and the
/// reply opened with `<|turn>model\n` (thinking off).
fn render_known(
    template: &str,
    system: &str,
    examples: &[(String, String)],
    question: &str,
) -> Option<String> {
    if !template.contains("<|turn>") {
        return None;
    }
    let mut out = format!("<|turn>system\n{system}<turn|>\n");
    for (user, assistant) in examples {
        out.push_str(&format!(
            "<|turn>user\n{user}<turn|>\n<|turn>model\n{assistant}<turn|>\n"
        ));
    }
    out.push_str(&format!("<|turn>user\n{question}<turn|>\n<|turn>model\n"));
    Some(out)
}

/// `SPOTTER_LLM_DEBUG=1` prints each call (development only; never on in
/// the app, which logs no schedule content).
fn debug(msg: &str) {
    if std::env::var_os("SPOTTER_LLM_DEBUG").is_some() {
        eprintln!("[llm] {msg}");
    }
}

fn norm(s: &str) -> String {
    spotter_core::text::key(s)
}

impl Tiebreaker for Engine {
    fn same_person(&self, a: &str, b: &str, roster: &[String]) -> Verdict {
        let others: Vec<&String> = roster
            .iter()
            .filter(|n| *n != a && *n != b)
            .take(10)
            .collect();
        self.verdict(
            Task::SamePerson,
            json!({"a": a, "b": b, "others_on_roster": others}),
        )
    }

    fn same_site(&self, a: &str, b: &str, sites: &[String]) -> Verdict {
        let others: Vec<&String> = sites
            .iter()
            .filter(|n| *n != a && *n != b)
            .take(10)
            .collect();
        self.verdict(
            Task::SameSite,
            json!({"a": a, "b": b, "other_sites": others}),
        )
    }

    fn cell_meaning(&self, text: &str, column_role: &str, neighbors: &[String]) -> Option<String> {
        let q = json!({"cell": text, "column": column_role, "neighbors": neighbors.iter().take(3).collect::<Vec<_>>()});
        self.ask(Task::CellMeaning, &q)
            .and_then(|v| v["answer"].as_str().map(str::to_string))
    }

    fn column_role(&self, header: &str, samples: &[String]) -> Option<String> {
        let q = json!({"header": header, "samples": samples.iter().take(8).collect::<Vec<_>>()});
        self.ask(Task::ColumnRole, &q)
            .and_then(|v| v["answer"].as_str().map(str::to_string))
    }

    fn free_text(&self, line: &str, roster: &[String], sites: &[String]) -> Option<Vec<TextEntry>> {
        let q = json!({"line": line, "known_people": roster, "known_sites": sites});
        let v = self.ask(Task::FreeText, &q)?;
        if v["readable"].as_bool() != Some(true) {
            return None;
        }
        let entries: Vec<TextEntry> = serde_json::from_value(v["entries"].clone()).ok()?;
        // T5 guard: every person and site must appear in the line itself.
        let line_key = norm(line);
        let ok = entries.iter().all(|e| {
            let p = norm(&e.person);
            let s = norm(&e.site);
            !p.is_empty() && !s.is_empty() && line_key.contains(&p) && line_key.contains(&s)
        });
        (ok && !entries.is_empty()).then_some(entries)
    }

    fn available(&self) -> bool {
        true
    }
}
