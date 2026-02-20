//! Integration tests for the `Storable` trait and `#[derive(Storable)]` macro.
use polaris_core_plugins::persistence::Storable;

// Derive macro with key and version
#[derive(Storable, Debug, PartialEq)]
#[storable(key = "ConversationMemory", schema_version = "2.0.0")]
struct ConversationMemory {
    messages: Vec<String>,
}

// Derive macro with key only (default version)
#[derive(Storable, Debug, PartialEq)]
#[storable(key = "AgentNotes")]
struct AgentNotes {
    notes: Vec<String>,
}

#[test]
fn derive_macro_key_and_version() {
    assert_eq!(ConversationMemory::storage_key(), "ConversationMemory");
    assert_eq!(ConversationMemory::schema_version(), "2.0.0");
}

#[test]
fn derive_macro_key_only_default_version() {
    assert_eq!(AgentNotes::storage_key(), "AgentNotes");
    assert_eq!(AgentNotes::schema_version(), "1.0.0");
}
