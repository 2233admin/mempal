use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    ClaudeJsonl,
    ChatGptJson,
    PlainText,
}

pub fn detect_format(content: &str) -> Format {
    if is_claude_jsonl(content) {
        return Format::ClaudeJsonl;
    }

    if is_chatgpt_json(content) {
        return Format::ChatGptJson;
    }

    Format::PlainText
}

fn is_claude_jsonl(content: &str) -> bool {
    let mut saw_line = false;

    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return false;
        };

        if value.get("type").and_then(Value::as_str).is_none() {
            return false;
        }
        if extract_message_text(&value).is_none() {
            return false;
        }

        saw_line = true;
    }

    saw_line
}

fn is_chatgpt_json(content: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return false;
    };

    matches!(value, Value::Array(_))
        || value.get("messages").is_some()
        || value.get("mapping").is_some()
}

pub(crate) fn extract_message_text(value: &Value) -> Option<String> {
    value
        .get("message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| value.get("content").and_then(extract_content_text))
}

pub(crate) fn extract_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Value::Object(map) => map
            .get("parts")
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|text| !text.is_empty()),
        _ => None,
    }
}
