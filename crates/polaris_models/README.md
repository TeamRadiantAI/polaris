# polaris_models

Model provider interface for Polaris. Provides a unified API for interacting with different LLM providers.

## Setup

Add `ModelsPlugin` and at least one provider plugin:

```rust
use polaris_models::ModelsPlugin;
use polaris_model_providers::AnthropicPlugin;

let mut server = Server::new();
server.add_plugins(ModelsPlugin);
server.add_plugins(AnthropicPlugin::from_env("ANTHROPIC_API_KEY"));
```

## Usage

### Basic Generation

```rust
use polaris_models::{ModelRegistry, llm::GenerationRequest};

let registry = server.get_global::<ModelRegistry>().unwrap();
let llm = registry.llm("anthropic/claude-3-5-haiku-20241022")?;

let request = GenerationRequest::with_system(
    "You are a helpful assistant",
    "Hello!"
);

let response = llm.generate(request).await?;
```

### Structured Output

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
struct Person {
    name: String,
    age: u32,
}

let request = GenerationRequest::new("Extract: John is 30 years old");
let person: Person = llm.generate_structured(request).await?;
```

### Tool Calling

```rust
use polaris_models::llm::{GenerationRequest, ToolDefinition, ToolChoice};
use serde_json::json;

let request = GenerationRequest::new("What's the weather in Tokyo?")
    .tool(ToolDefinition {
        name: "get_weather".into(),
        description: "Get current weather for a city".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "required": ["city"]
        }),
    })
    .tool_choice(ToolChoice::Required);

let response = llm.generate(request).await?;
```

## Creating a Provider Plugin

Implement `LlmProvider` and register it in a plugin:

```rust
use polaris_models::llm::{LlmProvider, GenerationRequest, GenerationResponse, GenerationError};
use polaris_models::{ModelRegistry, ModelsPlugin};
use polaris_system::plugin::{Plugin, PluginId};
use async_trait::async_trait;

pub struct MyProvider { /* ... */ }

#[async_trait]
impl LlmProvider for MyProvider {
    async fn generate(
        &self,
        model: &str,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, GenerationError> {
        // Call your provider's API
    }
}

pub struct MyProviderPlugin { /* ... */ }

impl Plugin for MyProviderPlugin {
    fn dependencies(&self) -> Vec<PluginId> {
        vec![PluginId::of::<ModelsPlugin>()]
    }

    fn build(&self, server: &mut Server) {
        let mut registry = server.get_resource_mut::<ModelRegistry>()
            .expect("ModelsPlugin must be added first");
        registry.register_llm_provider("myprovider", Arc::new(MyProvider::new()));
    }
}
```

The registry is available as a mutable resource during the `build()` phase, allowing providers to register themselves. After all plugins are built, it becomes an immutable global for thread-safe access at runtime.

Models are then accessible via `"myprovider/model-name"`.
