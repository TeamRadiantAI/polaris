//! Error types for the model registry.

/// Error creating a model handle.
#[derive(Debug, thiserror::Error)]
pub enum CreateModelError {
    /// Invalid model ID format.
    #[error("invalid model id '{0}': expected format 'provider/model'")]
    InvalidModelId(String),

    /// The specified provider was not found in the registry.
    #[error("unknown provider: {0}")]
    UnknownProvider(String),

    /// The specified model is not supported by the provider.
    #[error("unsupported model: {0}")]
    UnsupportedModel(String),
}
