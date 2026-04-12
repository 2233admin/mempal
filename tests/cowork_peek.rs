//! Integration tests for P6 cowork peek-and-decide.
//!
//! Run with:
//!   cargo test --test cowork_peek --no-default-features --features model2vec
//!
//! These tests build a fake HOME dir with Claude/Codex fixture sessions and
//! verify the peek_partner orchestration end-to-end. They do NOT touch the
//! real ~/.claude or ~/.codex directories — `home_override` on PeekRequest
//! injects a tempdir as the resolved HOME.

use mempal::cowork::{PeekError, PeekRequest, Tool, peek_partner};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Build a fake HOME dir containing Claude and Codex fixture sessions for the
/// given cwd. Returns the TempDir guard (keep alive for the test) and the
/// HOME path to pass into `home_override`.
fn build_fake_home(cwd: &Path) -> (TempDir, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let home = tmp.path().to_path_buf();

    // Claude: ~/.claude/projects/<encoded>/session.jsonl
    let encoded = cwd.to_string_lossy().replace('/', "-");
    let claude_dir = home.join(".claude/projects").join(&encoded);
    fs::create_dir_all(&claude_dir).unwrap();
    let cwd_str = cwd.to_string_lossy();
    let claude_jsonl = format!(
        r#"{{"type":"permission-mode","permissionMode":"default"}}
{{"parentUuid":null,"isSidechain":false,"type":"user","message":{{"role":"user","content":"Claude user msg"}},"uuid":"u1","timestamp":"2026-04-13T10:00:00Z","cwd":"{cwd_str}"}}
{{"parentUuid":"u1","isSidechain":false,"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Claude reply"}}]}},"uuid":"a1","timestamp":"2026-04-13T10:00:05Z","cwd":"{cwd_str}"}}
"#
    );
    fs::write(claude_dir.join("session.jsonl"), claude_jsonl).unwrap();

    // Codex: ~/.codex/sessions/2026/04/13/rollout-*.jsonl
    let codex_dir = home.join(".codex/sessions/2026/04/13");
    fs::create_dir_all(&codex_dir).unwrap();
    let codex_jsonl = format!(
        r#"{{"timestamp":"2026-04-13T12:00:00Z","type":"session_meta","payload":{{"id":"x","timestamp":"2026-04-13T12:00:00Z","cwd":"{cwd_str}","originator":"codex-tui"}}}}
{{"timestamp":"2026-04-13T12:00:10Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"Codex user msg"}}]}}}}
{{"timestamp":"2026-04-13T12:00:20Z","type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"Codex reply"}}]}}}}
"#
    );
    fs::write(
        codex_dir.join("rollout-2026-04-13T12-00-00-x.jsonl"),
        codex_jsonl,
    )
    .unwrap();

    (tmp, home)
}

#[test]
fn test_peek_partner_claude_reads_codex_session() {
    let cwd = PathBuf::from("/tmp/fake-project-1");
    let (_tmp, home) = build_fake_home(&cwd);

    let req = PeekRequest {
        tool: Tool::Codex,
        limit: 30,
        since: None,
        cwd,
        caller_tool: Some(Tool::Claude),
        home_override: Some(home),
    };
    let resp = peek_partner(req).expect("peek");

    assert_eq!(resp.partner_tool, Tool::Codex);
    assert_eq!(resp.messages.len(), 2);
    assert_eq!(resp.messages[0].text, "Codex user msg");
    assert_eq!(resp.messages[1].text, "Codex reply");
    assert!(resp.messages[0].at <= resp.messages[1].at);
    assert!(!resp.truncated);
    assert!(resp.session_path.is_some());
}

#[test]
fn test_peek_partner_auto_mode_infers_partner() {
    let cwd = PathBuf::from("/tmp/fake-project-2");
    let (_tmp, home) = build_fake_home(&cwd);

    let req = PeekRequest {
        tool: Tool::Auto,
        limit: 30,
        since: None,
        cwd,
        caller_tool: Some(Tool::Claude),
        home_override: Some(home),
    };
    let resp = peek_partner(req).expect("peek");

    assert_eq!(resp.partner_tool, Tool::Codex);
    assert_eq!(resp.messages.len(), 2);
}

#[test]
fn test_peek_partner_auto_mode_errors_without_client_info() {
    let cwd = PathBuf::from("/tmp/fake-project-3");
    let (_tmp, home) = build_fake_home(&cwd);

    let req = PeekRequest {
        tool: Tool::Auto,
        limit: 30,
        since: None,
        cwd,
        caller_tool: None,
        home_override: Some(home),
    };
    let err = peek_partner(req).unwrap_err();
    assert!(matches!(err, PeekError::CannotInferPartner));
}

#[test]
fn test_peek_partner_reports_inactive_session() {
    let cwd = PathBuf::from("/tmp/fake-project-4");
    let (tmp, home) = build_fake_home(&cwd);

    // Backdate the Codex jsonl to well over 30 minutes ago via `touch -t`.
    let codex_path = tmp
        .path()
        .join(".codex/sessions/2026/04/13/rollout-2026-04-13T12-00-00-x.jsonl");
    Command::new("touch")
        .arg("-t")
        .arg("198001010000")
        .arg(&codex_path)
        .status()
        .expect("touch");

    let req = PeekRequest {
        tool: Tool::Codex,
        limit: 30,
        since: None,
        cwd,
        caller_tool: Some(Tool::Claude),
        home_override: Some(home),
    };
    let resp = peek_partner(req).expect("peek");
    assert!(!resp.partner_active);
    assert!(!resp.messages.is_empty(), "still returns recent content");
}

#[test]
fn test_peek_partner_filters_by_project_cwd() {
    let cwd_a = PathBuf::from("/tmp/project-a-xyz");
    let (_tmp, home) = build_fake_home(&cwd_a);

    // Add a second Codex jsonl for a different cwd, in a newer date dir.
    let other_dir = home.join(".codex/sessions/2026/04/14");
    fs::create_dir_all(&other_dir).unwrap();
    fs::write(
        other_dir.join("rollout-2026-04-14T12-00-00-other.jsonl"),
        r#"{"timestamp":"2026-04-14T12:00:00Z","type":"session_meta","payload":{"id":"other","timestamp":"2026-04-14T12:00:00Z","cwd":"/tmp/project-b-xyz","originator":"codex-tui"}}
{"timestamp":"2026-04-14T12:00:10Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"should not appear"}]}}
"#,
    )
    .unwrap();

    let req = PeekRequest {
        tool: Tool::Codex,
        limit: 30,
        since: None,
        cwd: cwd_a,
        caller_tool: Some(Tool::Claude),
        home_override: Some(home),
    };
    let resp = peek_partner(req).expect("peek");
    let path_str = resp.session_path.expect("path");
    // Returned session must be the 04/13 one (matching project-a), NOT the
    // newer 04/14 rollout (which belongs to project-b).
    assert!(
        path_str.contains("2026/04/13"),
        "expected 04/13 session, got {path_str}"
    );
    assert!(
        !path_str.contains("2026/04/14"),
        "must not return 04/14 (belongs to project-b), got {path_str}"
    );
    for m in &resp.messages {
        assert!(!m.text.contains("should not appear"));
    }
}

#[test]
fn test_peek_partner_has_no_mempal_side_effects() {
    // Invariant by construction: peek_partner never touches Database.
    // This test exercises it 3× to ensure no stateful leak across calls.
    let cwd = PathBuf::from("/tmp/fake-project-5");
    let (_tmp, home) = build_fake_home(&cwd);

    let req = PeekRequest {
        tool: Tool::Codex,
        limit: 30,
        since: None,
        cwd,
        caller_tool: Some(Tool::Claude),
        home_override: Some(home),
    };
    for _ in 0..3 {
        let _ = peek_partner(req.clone()).expect("peek");
    }
}

#[test]
fn test_peek_partner_returns_empty_when_no_session() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    // Do not create any fixture files.

    let req = PeekRequest {
        tool: Tool::Claude,
        limit: 30,
        since: None,
        cwd: PathBuf::from("/tmp/no-session-project"),
        caller_tool: Some(Tool::Codex),
        home_override: Some(home),
    };
    let resp = peek_partner(req).expect("peek");
    assert_eq!(resp.messages.len(), 0);
    assert!(!resp.partner_active);
    assert!(resp.session_path.is_none());
}
