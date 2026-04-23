use mempal::ingest::noise::{strip_claude_jsonl_noise, strip_codex_rollout_noise};

#[test]
fn test_claude_jsonl_strips_system_reminder() {
    let content = "hello <system-reminder>mcp info</system-reminder> world";

    let stripped = strip_claude_jsonl_noise(content);

    assert_eq!(stripped, "hello  world");
}

#[test]
fn test_code_block_preserved_verbatim() {
    let code = "```rust\nfn main() {}\n```";
    let content = format!("{code}\n<system-reminder>x</system-reminder>");

    let stripped = strip_claude_jsonl_noise(&content);

    assert!(stripped.contains(code));
    assert!(!stripped.contains("system-reminder"));
}

#[test]
fn test_user_message_angle_brackets_preserved() {
    let content = r#"user: "I prefer Vec<T> over [T]""#;

    let stripped = strip_claude_jsonl_noise(content);

    assert_eq!(stripped.as_bytes(), content.as_bytes());
    assert!(stripped.contains("Vec<T>"));
    assert!(stripped.contains("[T]"));
}

#[test]
fn test_codex_rollout_session_markers_stripped() {
    let content = "[session 12345 started]\nwork\n[session 12345 ended]";

    let stripped = strip_codex_rollout_noise(content);

    assert_eq!(stripped, "work\n");
}

#[test]
fn test_strip_no_match_returns_identity() {
    let content = "plain text no markers";

    let stripped = strip_claude_jsonl_noise(content);

    assert_eq!(stripped.as_bytes(), content.as_bytes());
}

#[test]
fn test_strip_preserves_unicode_bytes() {
    let content = "决策 🎯 <system-reminder>x</system-reminder> 完成 ✅";

    let stripped = strip_claude_jsonl_noise(content);

    assert!(stripped.contains("决策 🎯"));
    assert!(stripped.contains(" 完成 ✅"));
    assert!(!stripped.contains("system-reminder"));
}
