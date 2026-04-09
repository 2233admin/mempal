use async_trait::async_trait;
use mempal_core::config::Config;

use crate::{
    EMBEDDING_DIMENSIONS, EmbedError, Embedder, Result, api::ApiEmbedder, onnx::OnnxEmbedder,
};

#[async_trait]
pub trait EmbedderFactory: Send + Sync {
    async fn build(&self) -> Result<Box<dyn Embedder>>;
}

#[derive(Clone)]
pub struct ConfiguredEmbedderFactory {
    config: Config,
}

impl ConfiguredEmbedderFactory {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl EmbedderFactory for ConfiguredEmbedderFactory {
    async fn build(&self) -> Result<Box<dyn Embedder>> {
        match self.config.embed.backend.as_str() {
            "onnx" => Ok(Box::new(OnnxEmbedder::new_or_download().await?)),
            "api" => Ok(Box::new(ApiEmbedder::new(
                self.config
                    .embed
                    .api_endpoint
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434/api/embeddings".to_string()),
                self.config.embed.api_model.clone(),
                EMBEDDING_DIMENSIONS,
            ))),
            backend => Err(EmbedError::UnsupportedBackend(backend.to_string())),
        }
    }
}
