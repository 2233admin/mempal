use std::collections::BTreeMap;

use mempal_aaak::{
    AaakCodec, AaakDocument, AaakHeader, AaakLine, AaakMeta, ArcLine, Tunnel, Zettel,
};

fn meta() -> AaakMeta {
    AaakMeta {
        wing: "myapp".to_string(),
        room: "auth".to_string(),
        date: "2026-04-08".to_string(),
        source: "readme".to_string(),
    }
}

#[test]
fn test_aaak_encode() {
    let codec = AaakCodec::default();
    let output = codec.encode(
        "Kai recommended Clerk over Auth0 based on pricing and DX",
        &meta(),
    );

    let encoded = output.document.to_string();
    assert!(encoded.contains("KAI"));
    assert!(encoded.contains("DECISION"));
}

#[test]
fn test_aaak_encode_synthesizes_placeholder_entity_code() {
    let codec = AaakCodec::default();
    let output = codec.encode("alpha beta gamma delta", &meta());

    assert_eq!(output.document.zettels[0].entities, vec!["UNK"]);
    let reparsed = AaakDocument::parse(&output.document.to_string())
        .expect("encoded output should remain valid AAAK");
    assert_eq!(reparsed.zettels[0].entities, vec!["UNK"]);
}

#[test]
fn test_aaak_encode_entity_codes_stay_within_bnf() {
    let codec = AaakCodec::default();
    let output = codec.encode("R2D2 recommended Clerk for auth.", &meta());

    assert!(
        output.document.zettels[0]
            .entities
            .iter()
            .all(|entity| entity.len() == 3 && entity.chars().all(|ch| ch.is_ascii_uppercase()))
    );
    AaakDocument::parse(&output.document.to_string()).expect("encoded output should parse");
}

#[test]
fn test_aaak_decode() {
    let codec = AaakCodec::with_entity_aliases(BTreeMap::from([
        ("Kai".to_string(), "KAI".to_string()),
        ("Clerk".to_string(), "CLK".to_string()),
    ]));
    let doc = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI+CLK|clerk_auth|\"Kai chose Clerk\"|★★★★|determ|DECISION",
    )
    .expect("document should parse");

    let decoded = codec.decode(&doc);
    assert!(decoded.contains("Kai"));
    assert!(decoded.contains("Clerk"));
}

#[test]
fn test_aaak_decode_replaces_codes_with_punctuation() {
    let codec = AaakCodec::with_entity_aliases(BTreeMap::from([
        ("Kai".to_string(), "KAI".to_string()),
        ("Clerk".to_string(), "CLK".to_string()),
    ]));
    let doc = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI+CLK|clerk_auth|\"KAI, chose CLK.\"|★★★★|determ|DECISION",
    )
    .expect("document should parse");

    let decoded = codec.decode(&doc);
    assert_eq!(decoded, "Kai, chose Clerk.");
}

#[test]
fn test_aaak_roundtrip() {
    let codec = AaakCodec::default();
    let original = "Kai recommended Clerk over Auth0. Pricing was better. Developer experience improved. The migration reduced auth bugs. The team documented the decision.";
    let output = codec.encode(original, &meta());
    let report = codec.verify_roundtrip(original, &output.document);

    assert!(report.coverage >= 0.8);
    assert!(report.lost.len() <= 1);
}

#[test]
fn test_aaak_roundtrip_reports_lost_assertions() {
    let codec = AaakCodec::default();
    let document = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"Kai recommended Clerk over Auth0.\"|★★★★|determ|DECISION",
    )
    .expect("document should parse");

    let report = codec.verify_roundtrip(
        "Kai recommended Clerk over Auth0. Pricing was better.",
        &document,
    );

    assert!(report.coverage < 1.0);
    assert_eq!(report.lost, vec!["Pricing was better".to_string()]);
}

#[test]
fn test_entity_bimap() {
    let codec =
        AaakCodec::with_entity_aliases(BTreeMap::from([("Alice".to_string(), "ALC".to_string())]));
    let output = codec.encode("Alice finalized the rollout plan", &meta());
    let encoded = output.document.to_string();
    assert!(encoded.contains("ALC"));

    let decoded = codec.decode(&output.document);
    assert!(decoded.contains("Alice"));
}

#[test]
fn test_aaak_parse() {
    let document = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION",
    )
    .expect("document should parse");

    assert_eq!(document.header.version, 1);
    assert_eq!(document.zettels.len(), 1);
    assert_eq!(document.zettels[0].entities, vec!["KAI"]);
}

#[test]
fn test_aaak_parse_accepts_tunnel_and_arc_lines() {
    let input = "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION\n1:CLK|auth_rollout|\"roll out Clerk\"|★★★|relief|TECHNICAL\nT:0<->1|auth_link\nARC:anx->determ->relief";
    let document = AaakDocument::parse(input).expect("document should parse");

    assert_eq!(document.zettels.len(), 2);
    assert_eq!(document.zettels[0].id, 0);
    assert_eq!(document.zettels[1].id, 1);
    assert_eq!(document.body.len(), 4);
    assert!(matches!(document.body[0], AaakLine::Zettel(_)));
    assert!(matches!(document.body[1], AaakLine::Zettel(_)));
    assert!(matches!(document.body[2], AaakLine::Tunnel(_)));
    assert!(matches!(document.body[3], AaakLine::Arc(_)));
}

#[test]
fn test_aaak_display_preserves_tunnel_and_arc_lines() {
    let input = "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION\n1:CLK|auth_rollout|\"roll out Clerk\"|★★★|relief|TECHNICAL\nT:0<->1|auth_link\nARC:anx->determ->relief";
    let document = AaakDocument::parse(input).expect("document should parse");

    assert_eq!(document.to_string(), input);
}

#[test]
fn test_aaak_decode_prefers_body_over_stale_zettels() {
    let codec =
        AaakCodec::with_entity_aliases(BTreeMap::from([("Kai".to_string(), "KAI".to_string())]));
    let document = AaakDocument {
        header: AaakHeader {
            version: 1,
            wing: "myapp".to_string(),
            room: "auth".to_string(),
            date: "2026-04-08".to_string(),
            source: "readme".to_string(),
        },
        body: vec![
            AaakLine::Zettel(Zettel {
                id: 0,
                entities: vec!["KAI".to_string()],
                topics: vec!["auth".to_string()],
                quote: "KAI chose Clerk".to_string(),
                weight: 4,
                emotions: vec!["determ".to_string()],
                flags: vec!["DECISION".to_string()],
            }),
            AaakLine::Tunnel(Tunnel {
                left: 0,
                right: 1,
                label: "auth_link".to_string(),
            }),
            AaakLine::Arc(ArcLine {
                emotions: vec!["anx".to_string(), "relief".to_string()],
            }),
        ],
        zettels: vec![Zettel {
            id: 99,
            entities: vec!["UNK".to_string()],
            topics: vec!["stale".to_string()],
            quote: "stale".to_string(),
            weight: 1,
            emotions: vec!["anx".to_string()],
            flags: vec!["CORE".to_string()],
        }],
    };

    assert_eq!(codec.decode(&document), "Kai chose Clerk");
}

#[test]
fn test_aaak_parse_invalid() {
    assert!(AaakDocument::parse("this is not aaak").is_err());
}

#[test]
fn test_aaak_parse_rejects_invalid_tunnel_line() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION\nT:0-1|auth_link",
    )
    .expect_err("invalid tunnel should fail");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_parse_rejects_tunnel_reference_to_missing_zettel() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION\nT:0<->9|auth_link",
    )
    .expect_err("tunnel should reference existing zettels");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_parse_rejects_duplicate_zettel_ids() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION\n0:CLK|auth_rollout|\"Clerk rollout\"|★★★|relief|TECHNICAL",
    )
    .expect_err("duplicate zettel ids should fail");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_parse_rejects_invalid_arc_line() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION\nARC:ANX->determ",
    )
    .expect_err("invalid arc should fail");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_parse_rejects_invalid_entity_code() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:Kai|clerk_auth|\"use Clerk\"|★★★★|determ|DECISION",
    )
    .expect_err("invalid entity code should fail");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_parse_rejects_invalid_emotion_code() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determination|DECISION",
    )
    .expect_err("invalid emotion code should fail");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_parse_rejects_invalid_flag() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"|★★★★|determ|GENESIS",
    )
    .expect_err("invalid flag should fail");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_parse_rejects_invalid_weight() {
    let error = AaakDocument::parse(
        "V1|myapp|auth|2026-04-08|readme\n0:KAI|clerk_auth|\"use Clerk\"||determ|DECISION",
    )
    .expect_err("invalid weight should fail");

    assert!(matches!(error, mempal_aaak::ParseError::InvalidZettel(_)));
}

#[test]
fn test_aaak_version() {
    let codec = AaakCodec::default();
    let output = codec.encode("Decision: use Clerk", &meta());

    assert!(output.document.to_string().starts_with("V1|"));
}

#[test]
fn test_aaak_truncation_report() {
    let codec = AaakCodec::default();
    let output = codec.encode(
        "alpha beta gamma delta epsilon zeta eta theta iota kappa",
        &meta(),
    );

    assert_eq!(output.report.topics_truncated, 7);
    assert!(!output.report.key_sentence_truncated);
    assert!(output.report.coverage > 0.0);
}

#[test]
fn test_aaak_detects_extended_emotions() {
    let codec = AaakCodec::default();
    let output = codec.encode(
        "We were grateful, curious, and surprised after the release.",
        &meta(),
    );
    let emotions = &output.document.zettels[0].emotions;

    assert!(emotions.iter().any(|emotion| emotion == "grat"));
    assert!(emotions.iter().any(|emotion| emotion == "curious"));
    assert!(emotions.iter().any(|emotion| emotion == "surpr"));
}

#[test]
fn test_aaak_encode_chinese_text() {
    let codec = AaakCodec::default();
    let output = codec.encode(
        "\u{5f20}\u{4e09}\u{51b3}\u{5b9a}\u{7528}Clerk\u{66ff}\u{6362}Auth0\u{ff0c}\u{56e0}\u{4e3a}\u{4ef7}\u{683c}\u{66f4}\u{4f18}\u{3002}",
        // 张三决定用Clerk替换Auth0，因为价格更优。
        &meta(),
    );

    let zettel = &output.document.zettels[0];

    // Should detect Chinese flag signals
    assert!(
        zettel.flags.contains(&"DECISION".to_string()),
        "should detect DECISION from 决定: {:?}",
        zettel.flags
    );

    // Should detect Chinese emotion signals
    assert!(
        zettel.emotions.contains(&"determ".to_string()),
        "should detect determ from 决定: {:?}",
        zettel.emotions
    );

    // Should extract CJK entities (2-4 char sequences)
    assert!(
        !zettel.entities.iter().all(|e| e == "UNK"),
        "should extract at least one non-UNK entity: {:?}",
        zettel.entities
    );

    // Topics should include CJK bigrams
    assert!(
        zettel.topics.len() > 1,
        "should extract CJK topics: {:?}",
        zettel.topics
    );

    // Roundtrip: Chinese assertions split on 。and ，
    assert!(
        output.report.coverage >= 0.5,
        "Chinese roundtrip coverage should be reasonable: {}",
        output.report.coverage
    );

    // Encoded output should be valid AAAK
    AaakDocument::parse(&output.document.to_string())
        .expect("Chinese-encoded output should remain valid AAAK");
}

#[test]
fn test_aaak_encode_mixed_script_text_extracts_cjk_and_ascii_entities() {
    let codec = AaakCodec::with_entity_aliases(BTreeMap::from([
        ("张三".to_string(), "ZSN".to_string()),
        ("Clerk".to_string(), "CLK".to_string()),
        ("Auth0".to_string(), "AUT".to_string()),
    ]));
    let output = codec.encode("张三决定用Clerk替换Auth0，因为价格更优。", &meta());
    let entities = &output.document.zettels[0].entities;

    assert!(entities.iter().any(|entity| entity == "ZSN"));
    assert!(entities.iter().any(|entity| entity == "CLK"));
    assert!(entities.iter().any(|entity| entity == "AUT"));
}

#[test]
fn test_aaak_encode_pure_chinese_text_extracts_leading_name_entity() {
    let codec =
        AaakCodec::with_entity_aliases(BTreeMap::from([("李四".to_string(), "LSI".to_string())]));
    let output = codec.encode("李四推荐了新的数据库架构", &meta());

    assert!(
        output.document.zettels[0]
            .entities
            .iter()
            .any(|entity| entity == "LSI"),
        "expected 李四 to be preserved as a named entity: {:?}",
        output.document.zettels[0].entities
    );
}

#[test]
fn test_aaak_encode_chinese_topics_avoid_cross_stopword_bigrams() {
    let codec = AaakCodec::default();
    let output = codec.encode("我们部署了新的服务器配置", &meta());
    let topics = &output.document.zettels[0].topics;

    assert!(topics.iter().any(|topic| topic == "部署"));
    assert!(topics.iter().any(|topic| topic == "服务器"));
    assert!(topics.iter().any(|topic| topic == "配置"));
    assert!(!topics.iter().any(|topic| topic == "们部"));
    assert!(!topics.iter().any(|topic| topic == "署新"));
}

#[test]
fn test_aaak_roundtrip_does_not_split_on_chinese_commas() {
    let codec = AaakCodec::default();
    let output = codec.encode("张三决定用Clerk替换Auth0，因为价格更优。", &meta());

    assert_eq!(output.report.coverage, 1.0);
    assert!(output.report.lost_assertions.is_empty());
}

#[test]
fn test_aaak_chinese_entity_code_is_valid() {
    let codec = AaakCodec::default();
    // Pure Chinese text — no ASCII capitalized words
    let output = codec.encode(
        "\u{674e}\u{56db}\u{63a8}\u{8350}\u{4e86}\u{65b0}\u{7684}\u{6570}\u{636e}\u{5e93}\u{67b6}\u{6784}",
        // 李四推荐了新的数据库架构
        &meta(),
    );

    // All entity codes must be valid 3-char uppercase ASCII
    for entity in &output.document.zettels[0].entities {
        assert_eq!(entity.len(), 3, "entity code should be 3 chars: {entity}");
        assert!(
            entity.chars().all(|ch| ch.is_ascii_uppercase()),
            "entity code should be uppercase ASCII: {entity}"
        );
    }

    AaakDocument::parse(&output.document.to_string())
        .expect("output with CJK entity codes should be valid AAAK");
}

#[test]
fn test_aaak_chinese_technical_flags() {
    let codec = AaakCodec::default();
    let output = codec.encode(
        "\u{6211}\u{4eec}\u{90e8}\u{7f72}\u{4e86}\u{65b0}\u{7684}\u{670d}\u{52a1}\u{5668}\u{914d}\u{7f6e}",
        // 我们部署了新的服务器配置
        &meta(),
    );

    let flags = &output.document.zettels[0].flags;
    assert!(
        flags.contains(&"TECHNICAL".to_string()),
        "should detect TECHNICAL from 部署/服务器/配置: {flags:?}"
    );
}

#[test]
fn test_aaak_detects_extended_flags() {
    let codec = AaakCodec::default();
    let output = codec.encode(
        "We founded the API service after a breakthrough and stored private tokens in the vault.",
        &meta(),
    );
    let flags = &output.document.zettels[0].flags;

    assert!(flags.iter().any(|flag| flag == "ORIGIN"));
    assert!(flags.iter().any(|flag| flag == "PIVOT"));
    assert!(flags.iter().any(|flag| flag == "TECHNICAL"));
    assert!(flags.iter().any(|flag| flag == "SENSITIVE"));
}
