use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeSurface {
    Cli,
    App,
    Unknown,
}

impl RuntimeSurface {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RuntimeSurface::Cli => "cli",
            RuntimeSurface::App => "app",
            RuntimeSurface::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeContext {
    pub(crate) effective_prompt: Option<String>,
    pub(crate) surface: RuntimeSurface,
    pub(crate) transcript_source: Option<String>,
    pub(crate) transcript_thread_source: Option<String>,
    pub(crate) transcript_originator: Option<String>,
}

#[derive(Clone, Debug)]
struct TranscriptMeta {
    source: Option<String>,
    thread_source: Option<String>,
    originator: Option<String>,
}

pub(crate) fn runtime_context(payload: &Value) -> RuntimeContext {
    let effective_prompt = effective_prompt_from_payload(payload);
    let transcript_meta = payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .and_then(|path| transcript_meta(Path::new(path)));
    let surface = classify_surface(transcript_meta.as_ref(), payload);

    RuntimeContext {
        effective_prompt,
        surface,
        transcript_source: transcript_meta
            .as_ref()
            .and_then(|meta| meta.source.clone()),
        transcript_thread_source: transcript_meta
            .as_ref()
            .and_then(|meta| meta.thread_source.clone()),
        transcript_originator: transcript_meta.and_then(|meta| meta.originator),
    }
}

pub(crate) fn effective_prompt_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("prompt")
        .and_then(Value::as_str)
        .map(effective_prompt_text)
}

pub(crate) fn effective_prompt_text(prompt: &str) -> String {
    if let Some(input) = extract_delegated_input(prompt) {
        return clean_effective_prompt(input);
    }
    let unescaped = html_unescape_basic(prompt);
    if unescaped != prompt {
        if let Some(input) = extract_delegated_input(&unescaped) {
            return clean_effective_prompt(input);
        }
    }
    prompt.to_string()
}

fn clean_effective_prompt(prompt: &str) -> String {
    html_unescape_basic(prompt).trim().to_string()
}

fn extract_delegated_input(prompt: &str) -> Option<&str> {
    extract_tag_body(prompt, "<input>", "</input>")
}

fn extract_tag_body<'a>(text: &'a str, start_tag: &str, end_tag: &str) -> Option<&'a str> {
    let start = text.find(start_tag)? + start_tag.len();
    let end = text[start..]
        .find(end_tag)
        .map(|offset| start + offset)
        .unwrap_or(text.len());
    text.get(start..end)
}

fn html_unescape_basic(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn classify_surface(meta: Option<&TranscriptMeta>, payload: &Value) -> RuntimeSurface {
    if let Some(source) = meta.and_then(|meta| meta.source.as_deref()) {
        if source.eq_ignore_ascii_case("exec") {
            return RuntimeSurface::Cli;
        }
        if source.eq_ignore_ascii_case("vscode") {
            return RuntimeSurface::App;
        }
    }
    if payload
        .get("prompt")
        .and_then(Value::as_str)
        .is_some_and(|prompt| prompt.contains("<codex_delegation"))
    {
        return RuntimeSurface::App;
    }
    RuntimeSurface::Unknown
}

fn transcript_meta(path: &Path) -> Option<TranscriptMeta> {
    let file = File::open(path).ok()?;
    for line in BufReader::new(file).lines().map_while(Result::ok).take(32) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("session_meta") {
            continue;
        }
        let payload = value.get("payload")?;
        return Some(TranscriptMeta {
            source: string_field(payload, "source"),
            thread_source: string_field(payload, "thread_source"),
            originator: string_field(payload, "originator"),
        });
    }
    None
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}
