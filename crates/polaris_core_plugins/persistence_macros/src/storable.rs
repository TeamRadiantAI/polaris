//! Derive macro for the `Storable` trait.

use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Parsed attributes for the macro.
#[derive(FromDeriveInput)]
#[darling(attributes(storable))]
struct StorableArgs {
    ident: syn::Ident,
    generics: syn::Generics,

    /// The stable storage key for this resource.
    key: String,

    /// The schema version. Defaults to `"1.0.0"` if omitted.
    #[darling(default = "default_version")]
    schema_version: String,
}

/// Returns the default schema version.
fn default_version() -> String {
    "1.0.0".to_string()
}

/// Implementation of the `#[derive(Storable)]` macro.
///
/// Generates an implementation of `polaris_core_plugins::persistence::Storable` for the
/// annotated struct.
///
/// # Example
///
/// ```ignore
/// #[derive(Serialize, Deserialize, Storable)]
/// #[storable(key = "ConversationMemory", version = "2.0.0")]
/// struct ConversationMemory {
///     messages: Vec<String>,
/// }
/// ```
pub(crate) fn derive_storable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let args = match StorableArgs::from_derive_input(&input) {
        Ok(args) => args,
        Err(err) => return err.write_errors().into(),
    };

    let name = &args.ident;
    let (impl_generics, ty_generics, where_clause) = args.generics.split_for_impl();

    let key = &args.key;
    let schema_version = &args.schema_version;

    let pc_crate =
        polaris_macro_utils::resolve_crate_path(polaris_macro_utils::PolarisCrate::CorePlugins);

    let expanded = quote! {
        impl #impl_generics #pc_crate::persistence::Storable for #name #ty_generics #where_clause {
            fn storage_key() -> &'static str {
                #key
            }

            fn schema_version() -> &'static str {
                #schema_version
            }
        }
    };

    expanded.into()
}
