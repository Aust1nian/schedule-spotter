//! The five tasks, their output schemas, and the prompts for each model
//! variant (`llm/prompts/<variant>/<task>.json`, embedded at build time).

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Task {
    /// T1: are two spellings one person?
    SamePerson,
    /// T2: are two site strings one site?
    SameSite,
    /// T3: what is this cell?
    CellMeaning,
    /// T4: what role does this column play?
    ColumnRole,
    /// T5: the records in one free-text line.
    FreeText,
}

impl Task {
    pub const ALL: [Task; 5] = [
        Task::SamePerson,
        Task::SameSite,
        Task::CellMeaning,
        Task::ColumnRole,
        Task::FreeText,
    ];

    pub fn file(self) -> &'static str {
        match self {
            Task::SamePerson => "t1_same_person",
            Task::SameSite => "t2_same_site",
            Task::CellMeaning => "t3_cell_meaning",
            Task::ColumnRole => "t4_column_role",
            Task::FreeText => "t5_free_text",
        }
    }

    /// The only shapes an answer can take; the grammar is built from this.
    pub fn schema(self) -> Value {
        let answer = |options: &[&str]| json!({"type": "object", "properties": {"answer": {"type": "string", "enum": options}}, "required": ["answer"], "additionalProperties": false});
        match self {
            Task::SamePerson | Task::SameSite => answer(&["same", "different", "unsure"]),
            Task::CellMeaning => answer(&[
                "person",
                "site",
                "status",
                "unstaffed",
                "note",
                "time",
                "empty",
            ]),
            Task::ColumnRole => answer(&[
                "person", "site", "date", "day", "time", "status", "note", "ignore",
            ]),
            // Groups, not one entry per person per day: "Mon-Fri: Mike, Jose
            // @ Smith" is one short group instead of ten entries, which
            // keeps the output (and the time) small. Empty means unreadable.
            Task::FreeText => json!({
                "type": "object",
                "properties": {
                    "groups": {
                        "type": "array",
                        "maxItems": 6,
                        "items": {
                            "type": "object",
                            "properties": {
                                "days": {"type": "array", "minItems": 1, "maxItems": 7, "items": {"type": "string", "enum": ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]}},
                                "people": {"type": "array", "minItems": 1, "maxItems": 8, "items": {"type": "string", "maxLength": 40}},
                                "site": {"type": "string", "maxLength": 60},
                                "time": {"type": ["string", "null"], "pattern": "^[0-2][0-9]:[0-5][0-9]-[0-2][0-9]:[0-5][0-9]$"}
                            },
                            "required": ["days", "people", "site", "time"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["groups"],
                "additionalProperties": false
            }),
        }
    }

    pub fn max_tokens(self) -> usize {
        match self {
            Task::FreeText => 200,
            _ => 24,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Prompt {
    pub system: String,
    /// (user, assistant) worked examples.
    pub examples: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Prompts {
    by_task: Vec<(Task, Prompt)>,
}

macro_rules! variant_prompts {
    ($v:literal) => {
        [
            include_str!(concat!("../prompts/", $v, "/t1_same_person.json")),
            include_str!(concat!("../prompts/", $v, "/t2_same_site.json")),
            include_str!(concat!("../prompts/", $v, "/t3_cell_meaning.json")),
            include_str!(concat!("../prompts/", $v, "/t4_column_role.json")),
            include_str!(concat!("../prompts/", $v, "/t5_free_text.json")),
        ]
    };
}

impl Prompts {
    /// Variant A (Phi-4-mini), B (Gemma 4) or C (Llama 3.2 1B).
    pub fn for_variant(variant: &str) -> Prompts {
        let files = match variant.chars().next() {
            Some('A') => variant_prompts!("A"),
            Some('B') => variant_prompts!("B"),
            _ => variant_prompts!("C"),
        };
        let by_task = Task::ALL
            .iter()
            .zip(files)
            .map(|(t, text)| {
                (
                    *t,
                    serde_json::from_str(text)
                        .unwrap_or_else(|e| panic!("prompt {}: {e}", t.file())),
                )
            })
            .collect();
        Prompts { by_task }
    }

    pub fn get(&self, task: Task) -> &Prompt {
        &self
            .by_task
            .iter()
            .find(|(t, _)| *t == task)
            .expect("every task has a prompt")
            .1
    }
}
