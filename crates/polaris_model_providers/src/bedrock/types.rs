//! Primitive type conversions between Polaris and Bedrock types.

use aws_sdk_bedrockruntime::types as bedrock;
use aws_smithy_types::Document;
use polaris_models::llm::{self as polaris_llm, GenerationError};

/// Converts a Polaris image media type to a Bedrock image format.
pub fn convert_image_format(
    media_type: &polaris_llm::ImageMediaType,
) -> Result<bedrock::ImageFormat, GenerationError> {
    match media_type {
        polaris_llm::ImageMediaType::JPEG => Ok(bedrock::ImageFormat::Jpeg),
        polaris_llm::ImageMediaType::PNG => Ok(bedrock::ImageFormat::Png),
        polaris_llm::ImageMediaType::GIF => Ok(bedrock::ImageFormat::Gif),
        polaris_llm::ImageMediaType::WEBP => Ok(bedrock::ImageFormat::Webp),
        other => Err(GenerationError::UnsupportedContent(format!(
            "unsupported image media type for Bedrock: {other:?}"
        ))),
    }
}

/// Converts a Polaris document media type to a Bedrock document format.
pub fn convert_document_format(
    media_type: &polaris_llm::DocumentMediaType,
) -> bedrock::DocumentFormat {
    match media_type {
        polaris_llm::DocumentMediaType::PDF => bedrock::DocumentFormat::Pdf,
        polaris_llm::DocumentMediaType::TXT => bedrock::DocumentFormat::Txt,
        polaris_llm::DocumentMediaType::HTML => bedrock::DocumentFormat::Html,
        polaris_llm::DocumentMediaType::CSV => bedrock::DocumentFormat::Csv,
        polaris_llm::DocumentMediaType::MARKDOWN => bedrock::DocumentFormat::Md,
    }
}

/// Converts a Polaris tool result status to a Bedrock tool result status.
pub fn convert_tool_result_status(
    status: &polaris_llm::ToolResultStatus,
) -> bedrock::ToolResultStatus {
    match status {
        polaris_llm::ToolResultStatus::Success => bedrock::ToolResultStatus::Success,
        polaris_llm::ToolResultStatus::Error => bedrock::ToolResultStatus::Error,
    }
}

/// Decodes a base64-encoded document source into raw bytes.
pub fn decode_base64_source(
    source: &polaris_llm::DocumentSource,
) -> Result<Vec<u8>, GenerationError> {
    match source {
        polaris_llm::DocumentSource::Base64(data) => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|err| {
                    GenerationError::InvalidRequest(format!("failed to decode base64 data: {err}"))
                })
        }
    }
}

/// Converts a `serde_json::Value` to an AWS Smithy `Document`.
pub fn json_to_document(value: &serde_json::Value) -> Document {
    match value {
        serde_json::Value::Null => Document::Null,
        serde_json::Value::Bool(b) => Document::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Document::Number(aws_smithy_types::Number::PosInt(u))
            } else if let Some(i) = n.as_i64() {
                Document::Number(aws_smithy_types::Number::NegInt(i))
            } else if let Some(f) = n.as_f64() {
                Document::Number(aws_smithy_types::Number::Float(f))
            } else {
                tracing::warn!("cannot convert {n} to Document number, using null");
                Document::Null
            }
        }
        serde_json::Value::String(s) => Document::String(s.clone()),
        serde_json::Value::Array(arr) => {
            Document::Array(arr.iter().map(json_to_document).collect())
        }
        serde_json::Value::Object(obj) => Document::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_to_document(v)))
                .collect(),
        ),
    }
}

/// Converts an AWS Smithy `Document` to a `serde_json::Value`.
pub fn document_to_json(doc: &Document) -> serde_json::Value {
    match doc {
        Document::Null => serde_json::Value::Null,
        Document::Bool(b) => serde_json::Value::Bool(*b),
        Document::Number(n) => match n {
            aws_smithy_types::Number::PosInt(i) => serde_json::json!(*i),
            aws_smithy_types::Number::NegInt(i) => serde_json::json!(*i),
            aws_smithy_types::Number::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or_else(|| {
                    tracing::warn!(
                        "cannot convert {f} to JSON (NaN/Infinity not supported), using null"
                    );
                    serde_json::Value::Null
                }),
        },
        Document::String(s) => serde_json::Value::String(s.clone()),
        Document::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(document_to_json).collect())
        }
        Document::Object(obj) => serde_json::Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), document_to_json(v)))
                .collect(),
        ),
    }
}
