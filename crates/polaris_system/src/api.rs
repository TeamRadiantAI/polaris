//! API trait for capability registration.
//!
//! APIs are build-time registries that plugins use to expose capabilities
//! to other plugins. Unlike Resources (accessed by systems during execution),
//! APIs are accessed by plugins during the build/ready phases.
//!
//! # API vs Resource
//!
//! | Aspect | API | Resource |
//! |--------|-----|----------|
//! | **Purpose** | Plugin orchestration | System execution |
//! | **Accessed by** | Plugins | Systems |
//! | **Access method** | `server.api::<A>()` | `Res<T>`, `ResMut<T>` |
//! | **Lifetime** | Server lifetime | Global or per-context |
//! | **Phase** | Build/Ready | Execution |
//!
//! # When to Use API
//!
//! - Registering agent types
//! - Managing sessions and groups
//! - Plugin capability discovery
//! - Build-time configuration
//!
//! # When to Use Resource
//!
//! - Runtime state for systems
//! - Agent memory, scratchpad
//! - Configuration read by systems
//! - Tool registries accessed during execution
//!
//! # Example
//!
//! ```ignore
//! use std::sync::RwLock;
//! use hashbrown::HashMap;
//! use polaris_system::api::API;
//!
//! /// Registry for agent types.
//! pub struct AgentAPI {
//!     agents: RwLock<HashMap<String, Box<dyn Agent>>>,
//! }
//!
//! impl API for AgentAPI {}
//!
//! impl AgentAPI {
//!     pub fn new() -> Self {
//!         Self { agents: RwLock::new(HashMap::new()) }
//!     }
//!
//!     pub fn register(&self, name: impl Into<String>, agent: impl Agent) {
//!         self.agents.write().unwrap()
//!             .insert(name.into(), Box::new(agent));
//!     }
//! }
//! ```
//!
//! # Interior Mutability Pattern
//!
//! APIs that need registration typically use interior mutability:
//!
//! ```ignore
//! pub struct MyAPI {
//!     data: RwLock<HashMap<String, Value>>,
//! }
//!
//! impl API for MyAPI {}
//!
//! impl MyAPI {
//!     pub fn register(&self, key: &str, value: Value) {
//!         self.data.write().unwrap().insert(key.into(), value);
//!     }
//!
//!     pub fn get(&self, key: &str) -> Option<Value> {
//!         self.data.read().unwrap().get(key).cloned()
//!     }
//! }
//! ```
//!
//! This allows:
//! - `server.api::<MyAPI>()` returns `&MyAPI`
//! - Multiple plugins can call `register()` concurrently
//! - Thread-safe access without `&mut Server`

/// Marker trait for capability APIs.
///
/// APIs are build-time registries that plugins use to expose
/// capabilities to other plugins. They are NOT accessed by systems.
///
/// # Implementing API
///
/// Simply implement this marker trait for your type:
///
/// ```ignore
/// use polaris_system::api::API;
///
/// pub struct MyAPI { /* ... */ }
///
/// impl API for MyAPI {}
/// ```
///
/// # Usage in Plugins
///
/// ```ignore
/// impl Plugin for MyAPIPlugin {
///     fn build(&self, server: &mut Server) {
///         server.insert_api(MyAPI::new());
///     }
/// }
///
/// impl Plugin for ConsumerPlugin {
///     fn ready(&self, server: &mut Server) {
///         let api = server.api::<MyAPI>()
///             .expect("MyAPI required");
///         api.register("key", value);
///     }
/// }
/// ```
pub trait API: Send + Sync + 'static {}
