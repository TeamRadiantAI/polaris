//! Auto-detection of crate paths for generated code.
//!
//! The `#[system]` macro needs to emit fully-qualified paths into generated code.
//!
//! This module determines the correct path for `polaris_system` by checking:
//! 1. If the consuming crate is `polaris_system`, it uses a direct path.
//! 2. If the consuming crate depends on `polaris_system`, it emits `polaris_system::` paths.
//! 3. If the consuming crate depends on the `polaris` umbrella, it emits `polaris::polaris_system::` paths.
//!
//! This allows `#[system]` to work regardless of how the user imports Polaris,
//! including when dependencies are renamed in `Cargo.toml`.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Returns the token path for `polaris_system` in the consuming crate.
pub(crate) fn polaris_system_path() -> TokenStream {
    match crate_name("polaris_system") {
        Ok(FoundCrate::Itself) => quote!(polaris_system),
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            quote!(#ident)
        }
        Err(_) => match crate_name("polaris") {
            Ok(FoundCrate::Name(name)) => {
                let ident = format_ident!("{}", name);
                quote!(#ident::polaris_system)
            }
            _ => quote!(polaris_system),
        },
    }
}
