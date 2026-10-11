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
use llama_cpp_2::context::session::{LlamaStateSeqFlags, SeqState};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::token::data::LlamaTokenData;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
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
/// The confidence a model "same" needs before it merges anything; chosen on
/// half the benchmark set and checked on the other half (DECISIONS.md).
pub const SAME_MIN: f32 = 0.9;

#[derive(Debug)]
pub enum LoadError {
    Missing,
    HashMismatch,
    /// An Intel Mac without AVX2 (the 2013 Mac Pro): the app runs with the
    /// model off.
    UnsupportedCpu,
    Llama(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Missing => write!(f, "model file not found"),
            LoadError::HashMismatch => write!(f, "model file doesn't match its checksum"),
            LoadError::UnsupportedCpu => write!(f, "this Mac's processor can't run the model"),
            LoadError::Llama(e) => write!(f, "model failed to load: {e}"),
        }
    }
}

/// Greedy choice under the grammar: the highest-logit token the grammar
/// allows. Checks the 40 likeliest tokens first and the whole vocabulary
/// only when none of them fits; the result is the same either way, and
/// skipping the full check saves about a third of generation time.
///
/// Also returns the chosen token's probability among the tokens the grammar
/// allows: 1.0 when the grammar forces it, lower when the model hesitated.
fn pick(grammar: &mut LlamaSampler, logits: &[f32]) -> Option<(LlamaToken, f32)> {
    const TOP: usize = 40;
    let best = |data: Vec<LlamaTokenData>, grammar: &mut LlamaSampler| {
        let mut arr = LlamaTokenDataArray::new(data, false);
        grammar.apply(&mut arr);
        let allowed: Vec<&LlamaTokenData> =
            arr.data.iter().filter(|d| d.logit().is_finite()).collect();
        let top = allowed
            .iter()
            .max_by(|a, b| a.logit().total_cmp(&b.logit()))?;
        let total: f32 = allowed
            .iter()
            .map(|d| (d.logit() - top.logit()).exp())
            .sum();
        Some((top.id(), 1.0 / total))
    };
    let mut ids: Vec<usize> = (0..logits.len()).collect();
    let top = TOP.min(ids.len());
    ids.select_nth_unstable_by(top.saturating_sub(1), |&a, &b| {
        logits[b].total_cmp(&logits[a])
    });
    let shortlist = ids[..top]
        .iter()
        .map(|&i| LlamaTokenData::new(LlamaToken(i as i32), logits[i], 0.0))
        .collect();
    let token = best(shortlist, grammar).or_else(|| {
        let all = logits
            .iter()
            .enumerate()
            .map(|(i, &l)| LlamaTokenData::new(LlamaToken(i as i32), l, 0.0))
            .collect();
        best(all, grammar)
    })?;
    grammar.accept(token.0);
    Some(token)
}

/// T5's groups as one entry per person per day, in the line's order.
fn expand_groups(v: &Value) -> Vec<TextEntry> {
    let mut out = Vec::new();
    for g in v["groups"].as_array().into_iter().flatten() {
        let (Some(site), Some(days), Some(people)) = (
            g["site"].as_str(),
            g["days"].as_array(),
            g["people"].as_array(),
        ) else {
            continue;
        };
        for day in days.iter().filter_map(Value::as_str) {
            for person in people.iter().filter_map(Value::as_str) {
                out.push(TextEntry {
                    person: person.to_string(),
                    day: day.to_string(),
                    site: site.to_string(),
                    time: g["time"].as_str().map(str::to_string),
                });
            }
        }
    }
    out
}

/// Where the CPU modules (`ggml-cpu-*`) live: beside the program when
/// installed, else the build's own directory (development).
#[cfg(all(target_arch = "x86_64", target_os = "windows"))]
fn load_cpu_modules() {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .filter(|dir| {
            std::fs::read_dir(dir).is_ok_and(|mut entries| {
                entries
                    .any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().contains("ggml-cpu-")))
            })
        });
    match beside {
        Some(dir) => llama_cpp_2::llama_backend::load_backends_from_path(&dir),
        None => llama_cpp_2::llama_backend::load_backends(),
    }
}

fn backend() -> &'static LlamaBackend {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
        load_cpu_modules();
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
    /// Each task's fixed prefix (instructions and examples), evaluated once
    /// and restored when the task comes up again.
    prefixes: HashMap<Task, (Vec<LlamaToken>, SeqState)>,
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
    /// The confidence a "same" needs to merge (T1 / T2).
    same_min: f32,
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
        #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
        if !std::arch::is_x86_feature_detected!("avx2")
            || !std::arch::is_x86_feature_detected!("fma")
        {
            return Err(LoadError::UnsupportedCpu);
        }
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
                prefixes: HashMap::new(),
            }),
            limit: CALL_LIMIT,
            same_min: SAME_MIN,
            load_secs: started.elapsed().as_secs_f64(),
        })
    }

    /// Changes the confidence a "same" needs (benchmarks sweep it).
    pub fn with_same_min(mut self, same_min: f32) -> Engine {
        self.same_min = same_min;
        self
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

    /// Evaluates the tasks' fixed prefixes ahead of time so the first
    /// question of each task is as fast as the rest. Run on the loading
    /// thread after `load`, in the order questions come (reading: T4, T3;
    /// matching: T1, T2; T5 only when text is pasted). Returns seconds.
    pub fn warm(&self, tasks: &[Task]) -> f64 {
        let started = Instant::now();
        for &task in tasks {
            // One task at a time, so a question can get in between.
            if let Ok(mut inner) = self.inner.lock() {
                self.prefix(&mut inner, task);
            }
        }
        started.elapsed().as_secs_f64()
    }

    /// The whole prompt for one question, rendered with the model's template.
    fn render(&self, task: Task, question: &str) -> Option<String> {
        let p = self.prompts.get(task);
        let mut chat = vec![LlamaChatMessage::new("system".into(), p.system.clone()).ok()?];
        for (user, assistant) in &p.examples {
            chat.push(LlamaChatMessage::new("user".into(), user.clone()).ok()?);
            chat.push(LlamaChatMessage::new("assistant".into(), assistant.clone()).ok()?);
        }
        chat.push(LlamaChatMessage::new("user".into(), question.to_string()).ok()?);
        match self.model.apply_chat_template(&self.template, &chat, true) {
            Ok(p) => Some(p),
            // llama.cpp's built-in template engine doesn't know every new
            // format yet; render the ones we've verified ourselves.
            Err(e) => {
                let rendered = render_known(&self.template_text, &p.system, &p.examples, question);
                if rendered.is_none() {
                    debug(&format!("chat template failed: {e}"));
                }
                rendered
            }
        }
    }

    fn tokenize(&self, text: &str) -> Vec<LlamaToken> {
        let vocab = self.model.vocab();
        let mut tokens = vocab.tokenize(text.as_bytes(), false, true);
        if vocab.should_add_bos() && tokens.first() != Some(&vocab.bos()) {
            tokens.insert(0, vocab.bos());
        }
        tokens
    }

    /// Evaluates `tokens` from what's already cached on; logits only for
    /// the last token. Returns the batch index holding those logits.
    fn eval(&self, inner: &mut Inner, tokens: &[LlamaToken]) -> Option<i32> {
        // Reuse the cached prefix; keep at least one token to evaluate for
        // fresh logits.
        let common = inner
            .cached
            .iter()
            .zip(tokens)
            .take_while(|(a, b)| a == b)
            .count()
            .min(tokens.len() - 1);
        inner
            .ctx
            .clear_kv_cache_seq(Some(0), Some(common as u32), None)
            .ok()?;
        inner.cached.truncate(common);
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
                inner.cached.clear();
                return None;
            }
        }
        inner.cached.extend_from_slice(fresh);
        Some(batch.n_tokens() - 1)
    }

    /// The task's fixed prefix in the cache: restored if saved, else
    /// evaluated and saved. Returns its tokens.
    fn prefix(&self, inner: &mut Inner, task: Task) -> Option<Vec<LlamaToken>> {
        if let Some((tokens, state)) = inner.prefixes.get(&task) {
            let tokens = tokens.clone();
            if !inner.cached.starts_with(&tokens) {
                inner.ctx.clear_kv_cache_seq(Some(0), None, None).ok()?;
                let state = state.clone();
                inner.ctx.state_seq_set(&state, 0).ok()?;
                inner.cached = tokens.clone();
            }
            return Some(tokens);
        }
        // The prefix is what two different questions share.
        let a = self.tokenize(&self.render(task, "{}")?);
        let b = self.tokenize(&self.render(task, "[]")?);
        let n = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        let tokens = a[..n].to_vec();
        self.eval(inner, &tokens)?;
        // eval leaves the last token for fresh logits only when it was
        // already cached; the full prefix is in the cache now.
        let state = inner
            .ctx
            .state_seq_get(0, LlamaStateSeqFlags::default())
            .ok()?;
        inner.prefixes.insert(task, (tokens.clone(), state));
        Some(tokens)
    }

    fn complete(&self, inner: &mut Inner, task: Task, question: &Value) -> Option<Value> {
        let started = Instant::now();
        let prompt = self.render(task, &question.to_string())?;
        let tokens = self.tokenize(&prompt);
        if tokens.len() + task.max_tokens() >= N_CTX as usize {
            return None;
        }
        if let Some(prefix) = self.prefix(inner, task)
            && !tokens.starts_with(&prefix)
        {
            debug(&format!("{task:?}: prompt doesn't start with its prefix"));
        }
        let mut idx = self.eval(inner, &tokens)?;
        let prompt_done = started.elapsed();
        let fresh_n = tokens.len();
        let vocab = self.model.vocab();

        let mut grammar = LlamaSampler::grammar(self.model, &self.grammars[&task], "root").ok()?;
        let mut out: Vec<u8> = Vec::new();
        let mut batch = LlamaBatch::new(1, 1);
        let mut sample_time = Duration::ZERO;
        // The lowest probability among the model's real choices.
        let mut confidence = 1.0f32;
        for pos in (tokens.len() as i32..).take(task.max_tokens()) {
            if started.elapsed() > self.limit {
                debug(&format!("{task:?} over the time limit: {question}"));
                return None;
            }
            let t0 = Instant::now();
            let (token, p) = pick(&mut grammar, inner.ctx.get_logits_ith(idx))?;
            confidence = confidence.min(p);
            sample_time += t0.elapsed();
            if vocab.is_eog(token) {
                break;
            }
            out.extend(vocab.token_to_piece(token, false, None));
            if let Ok(text) = std::str::from_utf8(&out)
                && let Ok(mut v) = serde_json::from_str::<Value>(text.trim())
                && v.is_object()
            {
                v["confidence"] = json!((confidence * 1000.0).round() / 1000.0);
                debug(&format!(
                    "{task:?} prompt {fresh_n} tok {:.2}s, output {} tok {:.2}s (sampling {:.2}s): {question} -> {v}",
                    prompt_done.as_secs_f64(),
                    pos - tokens.len() as i32 + 1,
                    (started.elapsed() - prompt_done).as_secs_f64(),
                    sample_time.as_secs_f64()
                ));
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

    /// T1 / T2 as the model answered: ("same" | "different" | "unsure",
    /// confidence), before the confidence rule.
    pub fn same_answer(
        &self,
        task: Task,
        a: &str,
        b: &str,
        others: &[String],
    ) -> Option<(String, f32)> {
        let others: Vec<&String> = others
            .iter()
            .filter(|n| *n != a && *n != b)
            .take(10)
            .collect();
        let question = match task {
            Task::SamePerson => json!({"a": a, "b": b, "others_on_roster": others}),
            _ => json!({"a": a, "b": b, "other_sites": others}),
        };
        let v = self.ask(task, &question)?;
        Some((
            v["answer"].as_str()?.to_string(),
            v["confidence"].as_f64().unwrap_or(0.0) as f32,
        ))
    }

    /// A merge needs a confident "same": below `same_min` it goes to the
    /// owner as a question instead (DECISIONS.md, Phase 4).
    fn verdict(&self, task: Task, a: &str, b: &str, others: &[String]) -> Verdict {
        match self.same_answer(task, a, b, others) {
            Some((answer, confidence)) if answer == "same" && confidence >= self.same_min => {
                Verdict::Same
            }
            Some((answer, _)) if answer == "different" => Verdict::Different,
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
        self.verdict(Task::SamePerson, a, b, roster)
    }

    fn same_site(&self, a: &str, b: &str, sites: &[String]) -> Verdict {
        self.verdict(Task::SameSite, a, b, sites)
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
        let entries = expand_groups(&v);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_expand_to_one_entry_per_person_per_day() {
        let v = json!({"groups": [
            {"days": ["mon", "tue"], "people": ["Tony", "Sam"], "site": "Birch", "time": null},
            {"days": ["wed"], "people": ["Kyle"], "site": "Hale job", "time": "07:00-15:00"}
        ]});
        let got: Vec<String> = expand_groups(&v)
            .iter()
            .map(|e| format!("{} {} {} {:?}", e.day, e.person, e.site, e.time))
            .collect();
        assert_eq!(
            got,
            [
                "mon Tony Birch None",
                "mon Sam Birch None",
                "tue Tony Birch None",
                "tue Sam Birch None",
                "wed Kyle Hale job Some(\"07:00-15:00\")"
            ]
        );
        assert!(expand_groups(&json!({"groups": []})).is_empty());
    }

    #[test]
    fn every_prompt_parses_and_stays_small() {
        for v in ["A", "B-E2B", "C"] {
            let prompts = prompts::Prompts::for_variant(v);
            for task in Task::ALL {
                let p = prompts.get(task);
                assert!(
                    (4..=6).contains(&p.examples.len()),
                    "{v} {task:?}: 4-6 examples"
                );
                let chars: usize = p.examples.iter().map(|(u, a)| u.len() + a.len()).sum();
                // Spec: examples under 600 tokens; ~3.5 characters a token.
                assert!(
                    chars < 600 * 3,
                    "{v} {task:?}: examples too long ({chars} chars)"
                );
            }
        }
    }
}
