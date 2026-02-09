//! Shared JSON Schema utilities for model providers.

use serde_json::Value;

/// Supported string formats for strict JSON Schema validation.
const SUPPORTED_FORMATS: &[&str] = &[
    "date-time",
    "time",
    "date",
    "duration",
    "email",
    "hostname",
    "uri",
    "ipv4",
    "ipv6",
    "uuid",
];

/// Normalizes a JSON schema for strict mode APIs.
///
/// This function:
/// - Sets `additionalProperties: false` on all object types.
/// - Removes unsupported properties like `minimum`, `maximum`, `multipleOf`, etc.
/// - Filters string formats to only supported values.
/// - Removes `minItems` values greater than 1.
/// - Removes external `$ref` URLs (only internal refs are supported).
/// - Removes `$ref` entries from `allOf` arrays (unsupported combination).
pub fn normalize_schema_for_strict_mode(mut schema: Value) -> Value {
    // Handle arrays at the top level (e.g., tuple validation schemas)
    if let Value::Array(ref mut arr) = schema {
        for item in arr.iter_mut() {
            *item = normalize_schema_for_strict_mode(item.take());
        }
        return schema;
    }

    if let Value::Object(ref mut obj) = schema {
        const UNSUPPORTED_PROPS: &[&str] = &[
            "minimum",
            "maximum",
            "exclusiveMinimum",
            "exclusiveMaximum",
            "multipleOf",
            "minLength",
            "maxLength",
            "maxItems",
            "uniqueItems",
            "minProperties",
            "maxProperties",
            "$schema",
            "title",
        ];

        for prop in UNSUPPORTED_PROPS {
            if obj.remove(*prop).is_some() {
                tracing::warn!(
                    property = *prop,
                    "Removed unsupported JSON schema property for strict mode"
                );
            }
        }

        // Handle format: keep only supported formats
        if let Some(format_val) = obj.get("format") {
            if let Some(format_str) = format_val.as_str() {
                if !SUPPORTED_FORMATS.contains(&format_str) {
                    tracing::warn!(
                        format = format_str,
                        "Removed unsupported string format for strict mode"
                    );
                    obj.remove("format");
                }
            } else {
                // Invalid format value, remove it
                tracing::warn!("Removed invalid (non-string) format value from JSON schema");
                obj.remove("format");
            }
        }

        // Handle minItems: only values 0 and 1 are supported
        if let Some(min_items) = obj.get("minItems") {
            if let Some(n) = min_items.as_u64() {
                if n > 1 {
                    tracing::warn!(
                        min_items = n,
                        "Removed minItems > 1 constraint (unsupported in strict mode)"
                    );
                    obj.remove("minItems");
                }
            } else {
                // Invalid minItems value, remove it
                tracing::warn!("Removed invalid (non-integer) minItems value from JSON schema");
                obj.remove("minItems");
            }
        }

        // Check if this is an object type schema
        let is_object = obj.get("type") == Some(&Value::String("object".to_string()))
            || obj.contains_key("properties");

        if is_object {
            obj.insert("additionalProperties".to_string(), Value::Bool(false));
        }

        // Recursively process nested schemas
        if let Some(Value::Object(props)) = obj.get_mut("properties") {
            for (_key, value) in props.iter_mut() {
                *value = normalize_schema_for_strict_mode(value.take());
            }
        }

        // Process items in array types
        if let Some(items) = obj.get_mut("items") {
            *items = normalize_schema_for_strict_mode(items.take());
        }

        // Process allOf, anyOf, oneOf
        for key in ["allOf", "anyOf", "oneOf"] {
            if let Some(Value::Array(arr)) = obj.get_mut(key) {
                for item in arr.iter_mut() {
                    *item = normalize_schema_for_strict_mode(item.take());
                }
            }
        }

        // Process $defs / definitions
        for key in ["$defs", "definitions"] {
            if let Some(Value::Object(defs)) = obj.get_mut(key) {
                for (_key, value) in defs.iter_mut() {
                    *value = normalize_schema_for_strict_mode(value.take());
                }
            }
        }

        // Remove external $ref (URLs)
        if let Some(ref_val) = obj.get("$ref") {
            if let Some(ref_str) = ref_val.as_str() {
                if ref_str.starts_with("http://") || ref_str.starts_with("https://") {
                    tracing::warn!(
                        ref_url = ref_str,
                        "Removed external $ref URL (only internal refs supported in strict mode)"
                    );
                    obj.remove("$ref");
                }
            }
        }

        if let Some(Value::Array(all_of)) = obj.get_mut("allOf") {
            let original_len = all_of.len();
            all_of.retain(|item| {
                if let Value::Object(item_obj) = item {
                    let has_ref = item_obj.contains_key("$ref");
                    if has_ref {
                        tracing::warn!(
                            "Removed $ref entry from allOf array (unsupported in strict mode)"
                        );
                    }
                    !has_ref
                } else {
                    true
                }
            });

            if all_of.is_empty() && original_len > 0 {
                tracing::warn!(
                    "Removed empty allOf array after filtering unsupported $ref entries"
                );
                obj.remove("allOf");
            }
        }
    }

    schema
}
