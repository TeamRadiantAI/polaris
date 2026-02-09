//! Schedule identifiers for tick-based plugin updates.
//!
//! This module provides [`ScheduleId`], which identifies tick schedules that
//! plugins can register for. Layer 2 defines schedule marker types, and
//! Layer 3 plugins declare interest in them.

use core::any::TypeId;

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

#[cfg(test)]
mod tests {
    use super::*;

    struct ScheduleA;
    struct ScheduleB;

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
}
