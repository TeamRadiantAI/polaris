//! Lifecycle hooks for graph execution.
//!
//! This module provides a hook system that allows plugins to extend the graph
//! executor with lifecycle callbacks. Hooks enable observability and resource
//! injection at various points during graph execution.
//!
//! # Design Principles
//!
//! - Hooks execute in registration order
//! - Primary use case: tracing, metrics, logging, debugging, resource injection
//! - Observer/Provider pattern separates concerns
//!
//! # Observer vs Provider
//!
//! - **Observers**: React to events (logging, metrics)
//! - **Providers**: Inject resources via return value
//!
//! When multiple providers write the same resource type, last-write-wins.
//!
//! # Architecture
//!
//! The hook system consists of three parts:
//!
//! - **Schedule markers** ([`schedule`]): Empty types that identify hook points
//! - **Events** ([`events`]): `GraphEvent` enum carrying context to hooks
//! - **API** ([`api`]): Registration and invocation mechanism
//!
//! # Example
//!
//! ```ignore
//! use polaris_graph::hooks::events::GraphEvent;
//!
//! // Observer: just logs events
//! hooks.register_observer::<OnSystemStart>("logger", |event: &GraphEvent| {
//!     if let GraphEvent::SystemStart { system_name, .. } = event {
//!         tracing::info!("System {} starting", system_name);
//!     }
//! })?;
//!
//! // Multi-schedule observer
//! hooks.register_observer::<(OnSystemStart, OnSystemComplete, OnSystemError)>(
//!     "tracker",
//!     |event: &GraphEvent| match event {
//!         GraphEvent::SystemStart { system_name, .. } => println!("Start: {}", system_name),
//!         GraphEvent::SystemComplete { duration, .. } => println!("Done: {:?}", duration),
//!         GraphEvent::SystemError { error, .. } => println!("Error: {}", error),
//!         _ => {}
//!     },
//! )?;
//!
//! // Provider: injects SystemInfo resource via return value
//! hooks.register_provider::<OnSystemStart, SystemInfo>("devtools", |event: &GraphEvent| {
//!     if let GraphEvent::SystemStart { node_id, system_name } = event {
//!         Some(SystemInfo::new(*node_id, system_name))
//!     } else {
//!         None
//!     }
//! })?;
//!
//! // For direct access to the system context, register directly with register_boxed:
//! let schedule = ScheduleId::of::<OnSystemStart>();
//! hooks.register_boxed(*schedule, "custom", BoxedHook::new(
//!     move |ctx, event: &GraphEvent| {
//!         // custom logic here, with access to ctx for resource injection
//!     },
//!     vec![TypeId::of::<CustomResource>()], // declare provided resource type
//! ))?;
//! ```

pub mod api;
pub mod events;
pub mod schedule;

pub use api::{HookRegistrationError, HooksAPI};
pub use events::GraphEvent;
