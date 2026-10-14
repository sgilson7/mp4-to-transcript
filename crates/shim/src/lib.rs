//! tool_shim: forwards calls between the page and `tool_core::api::Handle`.
//!
//! Builder requirement: the shim decides nothing. Each method is one call
//! into the core. `crates/core/tests/boundary.rs` fails if a branch, a loop or
//! the `?` operator appears here; if a feature seems to need one, the rule
//! belongs in the core.
//!
//! The template forwards free functions and the page holds the state as JSON.
//! This tool holds a recording's worth of model answers, so the shim holds
//! the core's handle (SECOND-ORDER.md R1.3).

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Tool(tool_core::api::Handle);

#[wasm_bindgen]
impl Tool {
    #[wasm_bindgen(constructor)]
    pub fn new(strings_json: &str, glossary_md: &str, files_json: &str, abilities_json: &str) -> Tool {
        Tool(tool_core::api::Handle::new(
            strings_json,
            glossary_md,
            files_json,
            abilities_json,
        ))
    }

    pub fn ui(&self) -> String {
        self.0.ui()
    }

    pub fn act(&mut self, name: &str, value: &str) -> String {
        self.0.act(name, value)
    }

    pub fn result(&mut self, report: &str, bytes: &[u8]) -> String {
        self.0.result(report, bytes)
    }

    pub fn file(&self) -> String {
        self.0.file()
    }
}
