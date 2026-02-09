//! Resource and output storage.
//!
//! This module provides containers for system data and marker traits for
//! resource scoping:
//!
//! - [`Resources`] - Type-safe storage for resources
//! - [`Outputs`] - Ephemeral system return values (cleared between runs)
//! - [`GlobalResource`] - Marker for read-only, server-lifetime resources
//! - [`LocalResource`] - Marker for mutable, per-context resources
//!
//! # Hierarchical Resource Model
//!
//! Resources can be scoped at different levels:
//!
//! | Scope | Marker Trait | Lifetime | Mutability |
//! |-------|--------------|----------|------------|
//! | Global | [`GlobalResource`] | Server | Read-only (`Res<T>`) |
//! | Local | [`LocalResource`] | Per-context | Mutable (`ResMut<T>`) |
//!
//! Global resources are shared across all execution contexts (agents, sessions).
//! Local resources are isolated per-context, enabling safe concurrent agent execution.
//!
//! # Example
//!
//! ```
//! use polaris_system::resource::{Resources, GlobalResource, LocalResource};
//! use polaris_system::param::{Res, ResMut};
//!
//! // Global resource - shared, read-only
//! struct Config { name: String }
//! impl GlobalResource for Config {}
//!
//! // Local resource - per-context, mutable
//! struct Memory { messages: Vec<String> }
//! impl LocalResource for Memory {}
//!
//! // In systems:
//! fn my_system(
//!     config: Res<Config>,      // OK - reads global
//!     mut memory: ResMut<Memory>, // OK - mutates local
//!     // mut cfg: ResMut<Config>, // Compile error! GlobalResource is read-only
//! ) {}
//! ```

mod output;
#[expect(
    clippy::module_inception,
    reason = "resource.rs contains the core Resource trait and Resources container logic"
)]
mod resource;

pub use output::{Output, OutputError, OutputId, OutputRef, Outputs};
pub use resource::{
    GlobalResource, LocalResource, Resource, ResourceError, ResourceId, ResourceRef,
    ResourceRefMut, Resources,
};
