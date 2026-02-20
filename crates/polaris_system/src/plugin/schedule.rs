//! Schedule identifiers for tick-based plugin updates.
//!
//! This module provides [`ScheduleId`], which identifies tick schedules that
//! plugins can register for. Layer 2 defines schedule marker types, and
//! Layer 3 plugins declare interest in them.
//!
//! # Schedule Trait
//!
//! The [`Schedule`] trait marks types as schedule identifiers. Layer 2 defines
//! schedule markers and associates them with events at the Layer 2 level.
//!
//! ```ignore
//! // Layer 2 defines:
//! pub struct OnSystemStart;
//! impl Schedule for OnSystemStart {}
//!
//! // Layer 3 can then:
//! hooks.register_observer::<OnSystemStart>("logger", |event| { ... })?;
//! ```
//!
//! # Multi-Schedule Registration
//!
//! [`IntoScheduleIds`] enables registering hooks for multiple schedules at once:
//!
//! ```ignore
//! hooks.register_observer::<(OnSystemStart, OnSystemComplete)>(
//!     "lifecycle_logger",
//!     |event: &GraphEvent| { ... },
//! )?;
//! ```

use core::any::TypeId;
use variadics_please::all_tuples;

/// Identifier for a tick schedule, based on a marker type.
///
/// Layer 2 defines schedule marker types (e.g., `PostAgentRun`, `PreTurn`),
/// and Layer 3 plugins reference them via `ScheduleId` to declare which
/// schedules they want to receive updates on.
///
/// # Architecture
///
/// The tick system follows a layered responsibility model:
///
/// - **Layer 1 (`polaris_system`)**: Provides the tick mechanism via [`ScheduleId`]
///   and [`Server::tick()`](crate::server::Server::tick)
/// - **Layer 2 (`polaris_agent`)**: Defines schedule marker types and triggers ticks
///   at appropriate times during agent execution
/// - **Layer 3 (plugins)**: Declare which schedules they want via
///   [`Plugin::tick_schedules()`](super::Plugin::tick_schedules)
///
/// # Example
///
/// ```ignore
/// // Layer 2 defines schedule marker types:
/// pub struct PostAgentRun;
/// pub struct PreTurn;
///
/// // Layer 3 plugin declares interest:
/// impl Plugin for TracingPlugin {
///     fn tick_schedules(&self) -> Vec<ScheduleId> {
///         vec![ScheduleId::of::<PostAgentRun>()]
///     }
///
///     fn update(&self, server: &mut Server, schedule: ScheduleId) {
///         // Flush traces after each agent run
///     }
/// }
///
/// // Layer 2 executor triggers the tick:
/// server.tick::<PostAgentRun>();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScheduleId {
    type_id: TypeId,
    type_name: &'static str,
}

impl ScheduleId {
    /// Creates a `ScheduleId` for the given schedule marker type.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Define a schedule marker type
    /// pub struct PostAgentRun;
    ///
    /// // Create an identifier for it
    /// let schedule = ScheduleId::of::<PostAgentRun>();
    /// ```
    #[must_use]
    pub fn of<S: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<S>(),
            type_name: core::any::type_name::<S>(),
        }
    }

    /// Returns the underlying `TypeId`.
    #[must_use]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the type name for debugging.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Schedule Trait
// ─────────────────────────────────────────────────────────────────────────────

/// Marker trait for schedule types.
///
/// Layer 2 implements this trait for schedule markers. The trait is intentionally
/// minimal (a pure marker) to keep Layer 1 generic. Event types are defined and
/// associated with schedules at Layer 2.
///
/// # Example
///
/// ```ignore
/// // Layer 2 defines:
/// pub struct OnSystemStart;
/// impl Schedule for OnSystemStart {}
///
/// // Layer 3 can then use type-safe registration:
/// hooks.register_observer::<OnSystemStart>("logger", |event: &GraphEvent| {
///     if let GraphEvent::SystemStart { system_name, .. } = event {
///         tracing::info!("System {} starting", system_name);
///     }
/// })?;
/// ```
pub trait Schedule: 'static {}

// ─────────────────────────────────────────────────────────────────────────────
// IntoScheduleIds Trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait for types that can be converted into a list of schedule IDs.
///
/// This enables registering hooks for multiple schedules at once using tuple syntax:
///
/// ```ignore
/// // Single schedule
/// hooks.register_observer::<OnSystemStart>("hook", |event| { ... })?;
///
/// // Multiple schedules
/// hooks.register_observer::<(OnSystemStart, OnSystemComplete)>("hook", |event| { ... })?;
/// ```
pub trait IntoScheduleIds {
    /// Returns the schedule IDs for this type.
    fn schedule_ids() -> Vec<ScheduleId>;
}

/// Single schedule implements `IntoScheduleIds`.
impl<S: Schedule> IntoScheduleIds for S {
    fn schedule_ids() -> Vec<ScheduleId> {
        vec![ScheduleId::of::<S>()]
    }
}

/// Macro to implement `IntoScheduleIds` for tuples of schedules.
macro_rules! impl_into_schedule_ids_for_tuple {
    ($($S:ident),*) => {
        impl<$($S: Schedule),*> IntoScheduleIds for ($($S,)*) {
            fn schedule_ids() -> Vec<ScheduleId> {
                vec![$(ScheduleId::of::<$S>()),*]
            }
        }
    };
}

// Generate implementations for tuples from 2 to 16 elements
all_tuples!(impl_into_schedule_ids_for_tuple, 2, 16, S);

#[cfg(test)]
mod tests {
    use super::*;

    struct ScheduleA;
    impl Schedule for ScheduleA {}

    struct ScheduleB;
    impl Schedule for ScheduleB {}

    struct ScheduleC;
    impl Schedule for ScheduleC {}

    #[test]
    fn schedule_id_equality() {
        let id1 = ScheduleId::of::<ScheduleA>();
        let id2 = ScheduleId::of::<ScheduleA>();
        let id3 = ScheduleId::of::<ScheduleB>();

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn schedule_id_type_name() {
        let id = ScheduleId::of::<ScheduleA>();
        assert!(id.type_name().contains("ScheduleA"));
    }

    #[test]
    fn schedule_id_type_id() {
        let id = ScheduleId::of::<ScheduleA>();
        assert_eq!(id.type_id(), TypeId::of::<ScheduleA>());
    }

    #[test]
    fn into_schedule_ids_single() {
        let ids = ScheduleA::schedule_ids();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], ScheduleId::of::<ScheduleA>());
    }

    #[test]
    fn into_schedule_ids_tuple() {
        let ids = <(ScheduleA, ScheduleB, ScheduleC)>::schedule_ids();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], ScheduleId::of::<ScheduleA>());
        assert_eq!(ids[1], ScheduleId::of::<ScheduleB>());
        assert_eq!(ids[2], ScheduleId::of::<ScheduleC>());
    }
}
