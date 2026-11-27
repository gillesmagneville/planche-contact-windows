//! What the engine writes on its standard output, for the window: JSON
//! events (one per line) and the generation's plain lines ("PROGRESS:X/100
//! message", "PREVIEW:n/total", messages). Always UTF-8, flushed at once
//! so that the window can follow as it comes.

use std::io::Write;

use serde_json::{Map, Value};

/// Version of the events. Raised when an existing event changes shape (a
/// new event or a new field does not change it).
pub const PROTOCOL_VERSION: u32 = 1;

pub fn line(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{text}");
    let _ = out.flush();
}

/// `{"event": name, ...fields}` on one line.
pub fn emit(name: &str, fields: Value) {
    let mut object = Map::new();
    object.insert("event".into(), Value::String(name.into()));
    if let Value::Object(map) = fields {
        object.extend(map);
    }
    line(&Value::Object(object).to_string());
}

pub fn progress(percent: u32, message: &str) {
    if message.is_empty() {
        line(&format!("PROGRESS:{percent}/100"));
    } else {
        line(&format!("PROGRESS:{percent}/100 {message}"));
    }
}
