//! Parameter extraction traits for tools.
//!
//! - [`FunctionParam`] — base trait for all extractable parameters
//! - [`InputParam`] — LLM-visible parameters that appear in JSON schema

use crate::error::ToolError;
use crate::schema::ParameterInfo;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// A function call request with name and JSON parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionCall {
    /// Function name.
    pub name: String,
    /// Parameters as a JSON object map.
    pub parameters: serde_json::Map<String, serde_json::Value>,
}

impl FunctionCall {
    /// Creates a new function call.
    pub fn new(
        name: impl Into<String>,
        parameters: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        Self {
            name: name.into(),
            parameters,
        }
    }

    /// Creates a function call from a [`serde_json::Value`], returning an error
    /// if `parameters` is not a JSON object.
    pub fn from_value(
        name: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Result<Self, ToolError> {
        match parameters {
            serde_json::Value::Object(map) => Ok(Self::new(name, map)),
            _ => Err(ToolError::parameter_error("Parameters must be an object")),
        }
    }

    /// Deserializes a required parameter by name.
    pub fn get_param<T: DeserializeOwned>(&self, name: &str) -> Result<T, ToolError> {
        let value = self
            .parameters
            .get(name)
            .ok_or_else(|| ToolError::parameter_error(format!("Missing parameter: {name}")))?;

        serde_json::from_value(value.clone()).map_err(|err| {
            ToolError::parameter_error(format!("Failed to deserialize parameter '{name}': {err}"))
        })
    }

    /// Deserializes an optional parameter by name. Returns `None` if missing or null.
    pub fn get_optional_param<T: DeserializeOwned>(
        &self,
        name: &str,
    ) -> Result<Option<T>, ToolError> {
        match self.parameters.get(name) {
            None => Ok(None),
            Some(value) if value.is_null() => Ok(None),
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(|err| {
                    ToolError::parameter_error(format!(
                        "Failed to deserialize parameter '{name}': {err}"
                    ))
                }),
        }
    }
}

/// Base trait for types that can be extracted as tool parameters.
pub trait FunctionParam {
    /// Extracts a required parameter from the function call.
    fn extract(call: &FunctionCall, param_name: &str) -> Result<Self, ToolError>
    where
        Self: Sized;

    /// Extracts an optional parameter from the function call.
    ///
    /// Returns `Ok(None)` when the parameter is missing or null.
    /// Used by `Option<T>` parameters and `#[default]` parameters.
    fn extract_optional(call: &FunctionCall, param_name: &str) -> Result<Option<Self>, ToolError>
    where
        Self: Sized;
}

/// Trait for parameters that come from LLM input and appear in JSON schema.
pub trait InputParam: FunctionParam {
    /// Returns schema information for this parameter type.
    fn schema_info(param_name: &str) -> ParameterInfo;
}

// ─────────────────────────────────────────────────────────────────────
// Blanket implementations
// ─────────────────────────────────────────────────────────────────────

impl<T: DeserializeOwned + JsonSchema> FunctionParam for T {
    fn extract(call: &FunctionCall, param_name: &str) -> Result<Self, ToolError> {
        call.get_param(param_name)
    }

    fn extract_optional(call: &FunctionCall, param_name: &str) -> Result<Option<Self>, ToolError> {
        call.get_optional_param(param_name)
    }
}

impl<T: DeserializeOwned + JsonSchema> InputParam for T {
    fn schema_info(param_name: &str) -> ParameterInfo {
        let mut generator = schemars::SchemaGenerator::default();
        let schema = T::json_schema(&mut generator);
        let schema_value = serde_json::to_value(schema).unwrap_or_else(|_| serde_json::json!({}));
        let mut info = ParameterInfo::new(param_name, schema_value);
        info.required = true;
        info
    }
}
