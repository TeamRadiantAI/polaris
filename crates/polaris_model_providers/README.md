# polaris_model_providers

LLM provider plugins for Polaris.

## Available Providers

| Provider | Plugin | Feature Flag | Model ID Format |
|----------|--------|--------------|-----------------|
| Anthropic | `AnthropicPlugin` | `anthropic` (default) | `anthropic/*` |
| AWS Bedrock | `BedrockPlugin` | `bedrock` | `bedrock/*` |

## Feature Flags

Each provider is gated behind a feature flag to avoid pulling in unnecessary dependencies.

```toml
# Enable only Anthropic (default)
polaris_model_providers = { path = "../polaris_model_providers" }

# Enable only Bedrock
polaris_model_providers = { path = "../polaris_model_providers", default-features = false, features = ["bedrock"] }

# Enable both providers
polaris_model_providers = { path = "../polaris_model_providers", features = ["bedrock"] }
```

## Usage

All provider plugins depend on `ModelsPlugin`, which must be added first to register the `ModelRegistry` resource.

```rust
use polaris_model_providers::AnthropicPlugin;
use polaris_models::ModelsPlugin;
use polaris_system::server::Server;

let mut server = Server::new();
server.add_plugins(ModelsPlugin);
server.add_plugins(AnthropicPlugin::from_env("ANTHROPIC_API_KEY"));
```

For AWS Bedrock, credentials are loaded from the default AWS credential chain:

```rust
use polaris_model_providers::BedrockPlugin;
use polaris_models::ModelsPlugin;
use polaris_system::server::Server;

let mut server = Server::new();
server.add_plugins(ModelsPlugin);
server.add_plugins(BedrockPlugin::from_env());
```

See the [polaris_models README](../polaris_models/README.md) for more usage examples.
