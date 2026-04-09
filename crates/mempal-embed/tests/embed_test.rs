use std::sync::Arc;

use axum::{Json, Router, routing::post};
use mempal_embed::{Embedder, api::ApiEmbedder, onnx::OnnxEmbedder};
use serde_json::Value;
use tokio::sync::OnceCell;

async fn shared_onnx_embedder() -> Arc<OnnxEmbedder> {
    static EMBEDDER: OnceCell<Arc<OnnxEmbedder>> = OnceCell::const_new();

    EMBEDDER
        .get_or_init(|| async {
            Arc::new(
                OnnxEmbedder::new_or_download()
                    .await
                    .expect("onnx embedder should initialize"),
            )
        })
        .await
        .clone()
}

#[tokio::test]
async fn test_embed_empty() {
    let embedder = shared_onnx_embedder().await;
    let result = embedder
        .embed(&[])
        .await
        .expect("empty embedding batch should succeed");

    assert!(result.is_empty());
}

#[tokio::test]
async fn test_onnx_dimensions() {
    let embedder = shared_onnx_embedder().await;

    assert_eq!(embedder.dimensions(), 384);
}

#[tokio::test]
async fn test_onnx_embed_single() {
    let embedder = shared_onnx_embedder().await;
    let vectors = embedder
        .embed(&["hello world"])
        .await
        .expect("single embedding should succeed");

    assert_eq!(vectors.len(), 1);
    assert_eq!(vectors[0].len(), 384);
    assert!(
        vectors[0]
            .iter()
            .all(|value| *value >= -1.0 && *value <= 1.0)
    );
}

#[tokio::test]
async fn test_onnx_batch() {
    let embedder = shared_onnx_embedder().await;
    let vectors = embedder
        .embed(&["text a", "text b", "text c"])
        .await
        .expect("batch embedding should succeed");

    assert_eq!(vectors.len(), 3);
    assert!(vectors.iter().all(|vector| vector.len() == 384));
}

#[tokio::test]
async fn test_api_embedder_config() {
    let embedder = ApiEmbedder::new(
        "http://localhost:11434/api/embeddings".into(),
        Some("nomic-embed-text".into()),
        384,
    );

    assert_eq!(embedder.dimensions(), 384);
    assert_eq!(embedder.name(), "api");
    assert_eq!(embedder.endpoint(), "http://localhost:11434/api/embeddings");
    assert_eq!(embedder.model(), Some("nomic-embed-text"));
}

#[tokio::test]
async fn test_api_embedder_openai_compatible() {
    async fn handler(Json(payload): Json<Value>) -> Json<Value> {
        assert_eq!(payload.get("model").and_then(Value::as_str), Some("test-model"));
        assert_eq!(
            payload.get("input").and_then(Value::as_array).map(Vec::len),
            Some(2)
        );
        Json(serde_json::json!({
            "data": [
                {"embedding": vec![0.1_f32; 4]},
                {"embedding": vec![0.2_f32; 4]}
            ]
        }))
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should expose address");
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/v1/embeddings", post(handler)))
            .await
            .expect("mock server should run");
    });

    let embedder = ApiEmbedder::new(
        format!("http://{addr}/v1/embeddings"),
        Some("test-model".into()),
        4,
    );

    let vectors = embedder
        .embed(&["hello", "world"])
        .await
        .expect("OpenAI-compatible response should parse");

    assert_eq!(vectors, vec![vec![0.1_f32; 4], vec![0.2_f32; 4]]);
}

#[tokio::test]
async fn test_api_embedder_ollama_compatible() {
    async fn handler(Json(payload): Json<Value>) -> Json<Value> {
        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("nomic-embed-text")
        );
        assert_eq!(payload.get("prompt").and_then(Value::as_str), Some("hello"));
        Json(serde_json::json!({
            "embedding": vec![0.3_f32; 3]
        }))
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should expose address");
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/api/embeddings", post(handler)))
            .await
            .expect("mock server should run");
    });

    let embedder = ApiEmbedder::new(
        format!("http://{addr}/api/embeddings"),
        Some("nomic-embed-text".into()),
        3,
    );

    let vectors = embedder
        .embed(&["hello"])
        .await
        .expect("Ollama-compatible response should parse");

    assert_eq!(vectors, vec![vec![0.3_f32; 3]]);
}
