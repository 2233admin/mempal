use anyhow::{Result, bail};
use serde_json::Value;

use crate::detect::{Format, extract_content_text, extract_message_text};

pub fn normalize_content(content: &str, format: Format) -> Result<String> {
    match format {
        Format::PlainText => Ok(content.trim().to_string()),
        Format::ClaudeJsonl => normalize_claude_jsonl(content),
        Format::ChatGptJson => normalize_chatgpt_json(content),
    }
}

fn normalize_claude_jsonl(content: &str) -> Result<String> {
    let mut lines = Vec::new();

    for raw_line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let value: Value = serde_json::from_str(raw_line)?;
        let role = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("assistant");
        let message = extract_message_text(&value).unwrap_or_default();

        if message.trim().is_empty() {
            continue;
        }

        if matches!(role, "human" | "user") {
            lines.push(format!("> {}", message.trim()));
        } else {
            lines.push(message.trim().to_string());
        }
    }

    Ok(lines.join("\n"))
}

fn normalize_chatgpt_json(content: &str) -> Result<String> {
    let value: Value = serde_json::from_str(content)?;

    if let Some(messages) = value.as_array() {
        return normalize_chatgpt_messages(messages);
    }

    if let Some(messages) = value.get("messages").and_then(Value::as_array) {
        return normalize_chatgpt_messages(messages);
    }

    if let Some(mapping) = value.get("mapping").and_then(Value::as_object) {
        let mut ordered = mapping
            .values()
            .filter_map(|node| node.get("message"))
            .filter_map(|message| {
                let role = message
                    .get("author")
                    .and_then(|author| author.get("role"))
                    .and_then(Value::as_str)?;
                let content = message.get("content").and_then(extract_content_text)?;
                Some((role.to_string(), content))
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|left, right| left.0.cmp(&right.0));

        return Ok(render_transcript(ordered));
    }

    bail!("unsupported ChatGPT JSON shape")
}

fn normalize_chatgpt_messages(messages: &[Value]) -> Result<String> {
    let transcript = render_transcript(messages.iter().filter_map(|message| {
        let role = message.get("role").and_then(Value::as_str)?;
        let content = message.get("content").and_then(extract_content_text)?;
        Some((role.to_string(), content))
    }));

    Ok(transcript)
}

fn render_transcript(items: impl IntoIterator<Item = (String, String)>) -> String {
    let mut lines = Vec::new();

    for (role, content) in items {
        if content.trim().is_empty() {
            continue;
        }

        if matches!(role.as_str(), "user" | "human") {
            lines.push(format!("> {}", content.trim()));
        } else {
            lines.push(content.trim().to_string());
        }
    }

    lines.join("\n")
}
