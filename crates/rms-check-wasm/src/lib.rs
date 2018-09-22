#![feature(use_extern_macros)]
extern crate wasm_bindgen;
extern crate serde_json;
#[macro_use] extern crate serde_derive;
extern crate rms_check;

use wasm_bindgen::prelude::*;
use rms_check::check as check_internal;
use rms_check::{Pos, Severity, Suggestion, Warning};

#[wasm_bindgen]
#[derive(Clone, Copy, Serialize)]
struct WasmPos {
    index: u32,
    line: u32,
    column: u32,
}
impl From<Pos> for WasmPos {
    fn from(pos: Pos) -> Self {
        WasmPos {
            index: pos.index() as u32,
            line: pos.line(),
            column: pos.column(),
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Serialize)]
struct WasmSuggestion {
    start: WasmPos,
    end: WasmPos,
    message: String,
    replacement: Option<String>,
}
impl<'a> From<&'a Suggestion> for WasmSuggestion {
    fn from(suggestion: &Suggestion) -> Self {
        WasmSuggestion {
            start: suggestion.start().into(),
            end: suggestion.end().into(),
            message: suggestion.message().into(),
            replacement: suggestion.replacement().clone(),
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Serialize)]
struct WasmWarning {
    severity: u8,
    start: WasmPos,
    end: WasmPos,
    message: String,
    suggestions: Vec<WasmSuggestion>,
}

impl<'a> From<&'a Warning> for WasmWarning {
    fn from(warn: &'a Warning) -> Self {
        WasmWarning {
            severity: match warn.severity() {
                Severity::Warning => 1,
                Severity::Error => 2,
            },
            start: warn.start().into(),
            end: warn.end().into(),
            message: warn.message().into(),
            suggestions: warn.suggestions().iter()
                .map(|s| s.into())
                .collect(),
        }
    }
}

#[wasm_bindgen]
pub fn check(source: &str) -> String {
    let warnings = check_internal(source)
        .iter()
        .map(|w| w.into())
        .collect::<Vec<WasmWarning>>();
    serde_json::to_string(&warnings).unwrap()
}
