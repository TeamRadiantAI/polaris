//! Auto-detection of crate paths for generated code.
//!
//! When `#[tool]`/`#[toolset]` macros are used from a crate that depends on
//! `polaris_tools` directly, the generated code uses direct crate paths.
//! When the consuming crate depends on the `polaris` umbrella crate instead,
//! it routes through `polaris::polaris_*` paths.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Resolved crate paths for all crates referenced by generated tool code.
pub(crate) struct CratePaths {
    /// Path to `polaris_tools` (for `Tool`, `ToolError`, `FunctionMetadata`, etc.)
    pub polaris_tools: TokenStream,
    /// Path to `polaris_models` (for `ToolDefinition`, etc.)
    pub polaris_models: TokenStream,
}

impl CratePaths {
    /// Auto-detects crate paths from the consuming crate's `Cargo.toml`.
    pub fn resolve() -> Self {
        Self {
            polaris_tools: resolve_crate("polaris_tools"),
            polaris_models: resolve_crate("polaris_models"),
        }
    }
}

/// Returns the token path for a crate, checking direct dependency first,
/// then falling back to the `polaris` umbrella crate.
fn resolve_crate(name: &str) -> TokenStream {
    match crate_name(name) {
        Ok(FoundCrate::Itself) => {
            let ident = format_ident!("{}", name);
            quote!(#ident)
        }
        Ok(FoundCrate::Name(found)) => {
            let ident = format_ident!("{}", found);
            quote!(#ident)
        }
        Err(_) => match crate_name("polaris") {
            Ok(FoundCrate::Name(found)) => {
                let polaris = format_ident!("{}", found);
                let ident = format_ident!("{}", name);
                quote!(#polaris::#ident)
            }
            _ => {
                let ident = format_ident!("{}", name);
                quote!(#ident)
            }
        },
    }
}
