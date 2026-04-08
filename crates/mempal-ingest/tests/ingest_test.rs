use mempal_ingest::{
    chunk::{chunk_conversation, chunk_text},
    detect::{Format, detect_format},
    normalize::normalize_content,
};

#[test]
fn test_fixed_window_chunk() {
    let text = "a".repeat(2000);
    let chunks = chunk_text(&text, 800, 100);

    assert!(chunks.len() >= 2);
    assert!(chunks[0].len() <= 800);
}

#[test]
fn test_qa_pair_chunk() {
    let transcript =
        "> How do I fix this?\nTry restarting.\n\n> What about the config?\nCheck settings.toml.";
    let chunks = chunk_conversation(transcript);

    assert_eq!(chunks.len(), 2);
    assert!(chunks[0].contains("How do I fix"));
    assert!(chunks[1].contains("config"));
}

#[test]
fn test_detect_claude_jsonl() {
    let content = r#"{"type":"human","message":"hello"}
{"type":"assistant","message":"hi"}"#;

    assert_eq!(detect_format(content), Format::ClaudeJsonl);
}

#[test]
fn test_detect_plain_text() {
    let content = "This is a regular markdown file.";

    assert_eq!(detect_format(content), Format::PlainText);
}

#[test]
fn test_normalize_claude_jsonl() {
    let content = r#"{"type":"human","message":"hello"}
{"type":"assistant","message":"hi"}"#;

    let normalized =
        normalize_content(content, Format::ClaudeJsonl).expect("claude jsonl should normalize");

    assert_eq!(normalized, "> hello\nhi");
}

#[test]
fn test_normalize_chatgpt_json() {
    let content = r#"[
  {"role":"user","content":"how do I fix this?"},
  {"role":"assistant","content":"restart the process"}
]"#;

    let normalized =
        normalize_content(content, Format::ChatGptJson).expect("chatgpt json should normalize");

    assert_eq!(normalized, "> how do I fix this?\nrestart the process");
}
