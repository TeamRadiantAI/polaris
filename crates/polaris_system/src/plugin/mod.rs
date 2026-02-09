//! Plugin system for extensible server functionality.
//!
//! Plugins are the fundamental unit of composition in Polaris. Every piece of
//! functionality—from core infrastructure to agent-specific features—is delivered
//! through plugins.
//!
//! # Philosophy
//!
//! **Everything is a plugin.** There is no "built-in" functionality that users
//! cannot replace, extend, or disable. The server is just a plugin orchestrator.
//!
//! # Example
//!
//! ```
//! use polaris_system::plugin::{Plugin, PluginId};
//! use polaris_system::server::Server;
//!
//! struct MyPlugin {
//!     config: String,
//! }
//!
//! # struct TracingPlugin;
//! # impl Plugin for TracingPlugin {
//! #     fn build(&self, _server: &mut Server) {}
//! # }
//!
//! # struct MyConfig {
//! #    value: String,
//! # }
//!
//! impl Plugin for MyPlugin {
//!     fn build(&self, server: &mut Server) {
//!         server.insert_resource(MyConfig {
//!             value: self.config.clone(),
//!         });
//!     }
//!
//!     fn dependencies(&self) -> Vec<PluginId> {
//!         vec![PluginId::of::<TracingPlugin>()]
//!     }
//! }
//!
//! Server::new()
//!     .add_plugins(TracingPlugin)
//!     .add_plugins(MyPlugin { config: "test".into() })
//!     .run();
//! ```

mod schedule;

pub use schedule::ScheduleId;

use core::any::TypeId;

use crate::server::Server;

// ─────────────────────────────────────────────────────────────────────────────
// PluginId
// ─────────────────────────────────────────────────────────────────────────────

/// Unique identifier for a plugin type.
///
/// Used for dependency resolution and duplicate detection. Based on [`TypeId`],
/// so each plugin type has exactly one `PluginId`.
///
/// # Example
///
/// ```ignore
/// fn dependencies(&self) -> Vec<PluginId> {
///     vec![
///         PluginId::of::<TracingPlugin>(),
///         PluginId::of::<IOPlugin>(),
///     ]
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId {
    type_id: TypeId,
    type_name: &'static str,
}

impl PluginId {
    /// Creates a `PluginId` for the given plugin type.
    #[must_use]
    pub fn of<P: Plugin>() -> Self {
        Self {
            type_id: TypeId::of::<P>(),
            type_name: core::any::type_name::<P>(),
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
// Plugin Trait
// ─────────────────────────────────────────────────────────────────────────────

/// A collection of resources and sub-plugins that extend server functionality.
///
/// Plugins follow a strict lifecycle managed by the server:
///
/// 1. **Build Phase** - `build()` is called in dependency order
/// 2. **Ready Phase** - `ready()` is called in dependency order
/// 3. **Tick Phase** - `update()` is called when schedules are triggered by Layer 2
/// 4. **Cleanup Phase** - `cleanup()` is called in reverse dependency order
///
/// # Scheduled Updates
///
/// Plugins can register for tick schedules defined by Layer 2. When Layer 2
/// triggers a schedule (e.g., after agent execution), only plugins that
/// registered for that schedule receive an `update()` call.
///
/// ```ignore
/// // Layer 2 defines schedule marker types
/// pub struct PostAgentRun;
///
/// impl Plugin for TracingPlugin {
///     fn build(&self, server: &mut Server) { /* ... */ }
///
///     fn tick_schedules(&self) -> Vec<ScheduleId> {
///         vec![ScheduleId::of::<PostAgentRun>()]
///     }
///
///     fn update(&self, server: &mut Server, schedule: ScheduleId) {
///         // Flush traces after each agent run
///     }
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// pub struct MetricsPlugin {
///     pub collect_interval: Duration,
/// }
///
/// impl Plugin for MetricsPlugin {
///     fn build(&self, server: &mut Server) {
///         server.insert_resource(MetricsCollector::new(self.collect_interval));
///     }
///
///     fn ready(&self, server: &mut Server) {
///         // Validate required resources exist
///         let _config = server.get_resource::<GlobalConfig>()
///             .expect("MetricsPlugin requires GlobalConfig");
///     }
///
///     fn cleanup(&self, server: &mut Server) {
///         // Flush any buffered metrics
///         if let Some(collector) = server.get_resource::<MetricsCollector>() {
///             collector.flush();
///         }
///     }
///
///     fn dependencies(&self) -> Vec<PluginId> {
///         vec![PluginId::of::<TracingPlugin>()]
///     }
/// }
/// ```
pub trait Plugin: Send + Sync + 'static {
    /// Configures the server. Called once when the plugin is added.
    ///
    /// Use this to:
    /// - Register resources with initial values
    /// - Add sub-plugins via `server.add_plugins()`
    ///
    /// # Note
    ///
    /// Keep `build()` lightweight. Heavy initialization should be deferred
    /// to `ready()` or done in systems.
    fn build(&self, server: &mut Server);

    /// Called after all plugins have been built and the server is ready to run.
    ///
    /// Use this for:
    /// - Validation that required resources exist
    /// - One-time initialization that depends on other plugins
    /// - Establishing connections (databases, APIs, etc.)
    fn ready(&self, _server: &mut Server) {}

    /// Called when a schedule this plugin registered for is triggered.
    ///
    /// The `schedule` parameter indicates which schedule triggered this update,
    /// allowing plugins to handle different schedules differently.
    ///
    /// Use this for:
    /// - Periodic maintenance tasks
    /// - Resource cleanup or rotation
    /// - Health checks
    /// - Flushing buffers
    ///
    /// # Note
    ///
    /// Most logic should be in systems, not here. Use sparingly.
    /// Only called if the plugin declared interest via [`tick_schedules()`](Self::tick_schedules).
    fn update(&self, _server: &mut Server, _schedule: ScheduleId) {}

    /// Called when the server is shutting down.
    ///
    /// Use this for:
    /// - Graceful connection termination
    /// - Flushing buffers
    /// - Cleanup of external resources
    ///
    /// Called in **reverse** dependency order (dependents cleanup before dependencies).
    fn cleanup(&self, _server: &mut Server) {}

    /// Declares which schedules this plugin wants to receive updates on.
    ///
    /// Schedules are marker types defined by Layer 2 (e.g., `PostAgentRun`, `PreTurn`).
    /// Return an empty vec to receive no scheduled updates (default).
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn tick_schedules(&self) -> Vec<ScheduleId> {
    ///     vec![
    ///         ScheduleId::of::<PostAgentRun>(),
    ///         ScheduleId::of::<PostTurn>(),
    ///     ]
    /// }
    /// ```
    fn tick_schedules(&self) -> Vec<ScheduleId> {
        Vec::new()
    }

    /// Returns the plugin's name for debugging and error messages.
    ///
    /// Default implementation returns the type name.
    fn name(&self) -> &str {
        core::any::type_name::<Self>()
    }

    /// Declares plugins that must be added before this one.
    ///
    /// The server will panic if dependencies are not satisfied when `run()` is called.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn dependencies(&self) -> Vec<PluginId> {
    ///     vec![
    ///         PluginId::of::<TracingPlugin>(),
    ///         PluginId::of::<IOPlugin>(),
    ///     ]
    /// }
    /// ```
    fn dependencies(&self) -> Vec<PluginId> {
        Vec::new()
    }

    /// Returns true if this plugin can only be added once.
    ///
    /// Default is `true` — adding the same plugin type twice will panic.
    /// Set to `false` for plugins that can be added multiple times with
    /// different configurations.
    fn is_unique(&self) -> bool {
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugins Trait (for add_plugins polymorphism)
// ─────────────────────────────────────────────────────────────────────────────

/// Trait for types that can be added to a server as plugins.
///
/// This trait enables `server.add_plugins()` to accept both:
/// - Single plugins implementing [`Plugin`]
/// - Plugin groups via [`PluginGroupBuilder`]
///
/// Users typically don't implement this trait directly.
pub trait Plugins {
    /// Adds these plugins to the server.
    fn add_to_server(self, server: &mut Server);
}

/// Single plugins implement `Plugins` directly.
impl<P: Plugin> Plugins for P {
    fn add_to_server(self, server: &mut Server) {
        // Capture PluginId while we still have the concrete type
        let id = PluginId::of::<P>();
        server.add_plugin_boxed(id, Box::new(self));
    }
}

/// `PluginGroupBuilder` implements `Plugins` to add all contained plugins.
impl Plugins for PluginGroupBuilder {
    fn add_to_server(self, server: &mut Server) {
        for boxed in self.plugins {
            server.add_plugin_boxed(boxed.id, boxed.plugin);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PluginGroup Trait
// ─────────────────────────────────────────────────────────────────────────────

/// A collection of plugins that can be added together.
///
/// Plugin groups provide a convenient way to bundle related plugins.
/// Users can customize the group before adding it to the server.
///
/// # Example
///
/// ```ignore
/// pub struct DefaultPlugins;
///
/// impl PluginGroup for DefaultPlugins {
///     fn build(self) -> PluginGroupBuilder {
///         PluginGroupBuilder::new()
///             .add(CorePlugin)
///             .add(TracingPlugin::default())
///             .add(IOPlugin)
///     }
/// }
///
/// // Use with customization
/// Server::new()
///     .add_plugins(
///         DefaultPlugins
///             .build()
///             .disable::<TracingPlugin>()
///             .add(CustomTracingPlugin::new())
///     )
///     .run();
/// ```
pub trait PluginGroup {
    /// Returns the plugins in this group.
    fn build(self) -> PluginGroupBuilder;
}

// ─────────────────────────────────────────────────────────────────────────────
// BoxedPlugin
// ─────────────────────────────────────────────────────────────────────────────

/// A boxed plugin with its captured [`PluginId`].
///
/// This struct preserves the plugin's type identity (via `PluginId`) after
/// boxing, enabling proper dependency resolution and duplicate detection.
pub(crate) struct BoxedPlugin {
    /// The plugin's unique identifier (captured before boxing).
    pub(crate) id: PluginId,
    /// The boxed plugin instance.
    pub(crate) plugin: Box<dyn Plugin>,
}

impl BoxedPlugin {
    /// Returns the plugin's name (delegating to the inner plugin).
    pub(crate) fn name(&self) -> &str {
        self.plugin.name()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PluginGroupBuilder
// ─────────────────────────────────────────────────────────────────────────────

/// Builder for customizing plugin groups.
///
/// Allows adding, removing, and reordering plugins within a group.
///
/// # Example
///
/// ```ignore
/// // Customize a plugin group
/// DefaultPlugins
///     .build()
///     .disable::<TracingPlugin>()
///     .add(CustomTracingPlugin::new())
///     .add_after::<MetricsPlugin, IOPlugin>(MetricsPlugin)
/// ```
#[derive(Default)]
pub struct PluginGroupBuilder {
    /// The plugins in this group, in order.
    pub(crate) plugins: Vec<BoxedPlugin>,
}

impl PluginGroupBuilder {
    /// Creates a new empty plugin group builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Adds a plugin to the end of the group.
    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "This is a builder method, not std::ops::Add"
    )]
    pub fn add<P: Plugin>(mut self, plugin: P) -> Self {
        let id = PluginId::of::<P>();
        self.plugins.push(BoxedPlugin {
            id,
            plugin: Box::new(plugin),
        });
        self
    }

    /// Adds a plugin before another plugin in the group.
    ///
    /// If `Target` is not found, the plugin is added at the beginning.
    ///
    /// # Type Parameters
    ///
    /// - `P`: The plugin to add
    /// - `Target`: The plugin to insert before
    #[must_use]
    pub fn add_before<P: Plugin, Target: Plugin>(mut self, plugin: P) -> Self {
        let target_name = core::any::type_name::<Target>();
        let position = self
            .plugins
            .iter()
            .position(|p| p.name() == target_name)
            .unwrap_or(0);
        let id = PluginId::of::<P>();
        self.plugins.insert(
            position,
            BoxedPlugin {
                id,
                plugin: Box::new(plugin),
            },
        );
        self
    }

    /// Adds a plugin after another plugin in the group.
    ///
    /// If `Target` is not found, the plugin is added at the end.
    ///
    /// # Type Parameters
    ///
    /// - `P`: The plugin to add
    /// - `Target`: The plugin to insert after
    #[must_use]
    pub fn add_after<P: Plugin, Target: Plugin>(mut self, plugin: P) -> Self {
        let target_name = core::any::type_name::<Target>();
        let position = self
            .plugins
            .iter()
            .position(|p| p.name() == target_name)
            .map(|i| i + 1)
            .unwrap_or(self.plugins.len());
        let id = PluginId::of::<P>();
        self.plugins.insert(
            position,
            BoxedPlugin {
                id,
                plugin: Box::new(plugin),
            },
        );
        self
    }

    /// Removes a plugin from the group by type.
    ///
    /// If the plugin is not found, this is a no-op.
    #[must_use]
    pub fn disable<P: Plugin>(mut self) -> Self {
        let target_name = core::any::type_name::<P>();
        self.plugins.retain(|p| p.name() != target_name);
        self
    }

    /// Returns the number of plugins in the group.
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns true if the group contains no plugins.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test plugins
    struct PluginA;
    impl Plugin for PluginA {
        fn build(&self, _server: &mut Server) {}
    }

    struct PluginB;
    impl Plugin for PluginB {
        fn build(&self, _server: &mut Server) {}
        fn dependencies(&self) -> Vec<PluginId> {
            vec![PluginId::of::<PluginA>()]
        }
    }

    struct PluginC;
    impl Plugin for PluginC {
        fn build(&self, _server: &mut Server) {}
    }

    #[test]
    fn plugin_id_equality() {
        let id1 = PluginId::of::<PluginA>();
        let id2 = PluginId::of::<PluginA>();
        let id3 = PluginId::of::<PluginB>();

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn plugin_id_type_name() {
        let id = PluginId::of::<PluginA>();
        assert!(id.type_name().contains("PluginA"));
    }

    #[test]
    fn plugin_default_name() {
        let plugin = PluginA;
        assert!(plugin.name().contains("PluginA"));
    }

    #[test]
    fn plugin_default_is_unique() {
        let plugin = PluginA;
        assert!(plugin.is_unique());
    }

    #[test]
    fn plugin_default_dependencies_empty() {
        let plugin = PluginA;
        assert!(plugin.dependencies().is_empty());
    }

    #[test]
    fn plugin_default_tick_schedules_empty() {
        let plugin = PluginA;
        assert!(plugin.tick_schedules().is_empty());
    }

    #[test]
    fn plugin_with_dependencies() {
        let plugin = PluginB;
        let deps = plugin.dependencies();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], PluginId::of::<PluginA>());
    }

    #[test]
    fn plugin_group_builder_add() {
        let builder = PluginGroupBuilder::new().add(PluginA).add(PluginB);

        assert_eq!(builder.len(), 2);
    }

    #[test]
    fn plugin_group_builder_disable() {
        let builder = PluginGroupBuilder::new()
            .add(PluginA)
            .add(PluginB)
            .disable::<PluginA>();

        assert_eq!(builder.len(), 1);
        assert!(builder.plugins[0].name().contains("PluginB"));
    }

    #[test]
    fn plugin_group_builder_add_before() {
        let builder = PluginGroupBuilder::new()
            .add(PluginA)
            .add(PluginB)
            .add_before::<_, PluginB>(PluginC);

        assert_eq!(builder.len(), 3);
        // Order: A, C, B
        assert!(builder.plugins[0].name().contains("PluginA"));
        assert!(builder.plugins[1].name().contains("PluginC"));
        assert!(builder.plugins[2].name().contains("PluginB"));
    }

    #[test]
    fn plugin_group_builder_add_after() {
        let builder = PluginGroupBuilder::new()
            .add(PluginA)
            .add(PluginB)
            .add_after::<_, PluginA>(PluginC);

        assert_eq!(builder.len(), 3);
        // Order: A, C, B
        assert!(builder.plugins[0].name().contains("PluginA"));
        assert!(builder.plugins[1].name().contains("PluginC"));
        assert!(builder.plugins[2].name().contains("PluginB"));
    }

    #[test]
    fn plugin_group_builder_add_before_not_found() {
        // When target not found, add at beginning
        let builder = PluginGroupBuilder::new()
            .add(PluginA)
            .add_before::<_, PluginB>(PluginC); // PluginB not in list

        assert_eq!(builder.len(), 2);
        // C added at beginning since B not found
        assert!(builder.plugins[0].name().contains("PluginC"));
        assert!(builder.plugins[1].name().contains("PluginA"));
    }

    #[test]
    fn plugin_group_builder_add_after_not_found() {
        // When target not found, add at end
        let builder = PluginGroupBuilder::new()
            .add(PluginA)
            .add_after::<_, PluginB>(PluginC); // PluginB not in list

        assert_eq!(builder.len(), 2);
        // C added at end since B not found
        assert!(builder.plugins[0].name().contains("PluginA"));
        assert!(builder.plugins[1].name().contains("PluginC"));
    }

    // Test PluginGroup trait
    struct TestPluginGroup;

    impl PluginGroup for TestPluginGroup {
        fn build(self) -> PluginGroupBuilder {
            PluginGroupBuilder::new().add(PluginA).add(PluginB)
        }
    }

    #[test]
    fn plugin_group_build() {
        let builder = TestPluginGroup.build();
        assert_eq!(builder.len(), 2);
    }

    #[test]
    fn plugin_id_type_id_method() {
        let id = PluginId::of::<PluginA>();
        assert_eq!(id.type_id(), TypeId::of::<PluginA>());

        let id_b = PluginId::of::<PluginB>();
        assert_eq!(id_b.type_id(), TypeId::of::<PluginB>());

        // Different plugin types should have different TypeIds
        assert_ne!(id.type_id(), id_b.type_id());
    }

    #[test]
    fn plugin_group_empty() {
        let builder = PluginGroupBuilder::new();

        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
    }

    #[test]
    fn plugin_group_disable_all() {
        let builder = PluginGroupBuilder::new()
            .add(PluginA)
            .add(PluginB)
            .disable::<PluginA>()
            .disable::<PluginB>();

        assert!(builder.is_empty());
    }

    #[test]
    fn plugin_group_disable_nonexistent_is_noop() {
        let builder = PluginGroupBuilder::new().add(PluginA).disable::<PluginC>(); // PluginC not in list

        // Should still have PluginA
        assert_eq!(builder.len(), 1);
        assert!(builder.plugins[0].name().contains("PluginA"));
    }

    // Test non-unique plugin behavior
    struct NonUniquePlugin;

    impl Plugin for NonUniquePlugin {
        fn build(&self, _server: &mut Server) {}

        fn is_unique(&self) -> bool {
            false // Can be added multiple times
        }
    }

    #[test]
    fn non_unique_plugin_can_be_added_multiple_times() {
        let mut server = Server::new();

        // Should not panic even when adding the same plugin type multiple times
        server.add_plugins(NonUniquePlugin);
        server.add_plugins(NonUniquePlugin);
        server.add_plugins(NonUniquePlugin);

        server.finish();

        // All three instances should have been built
        // (We can't directly verify this without side effects, but the key point
        // is that it doesn't panic like unique plugins would)
    }

    #[test]
    fn plugin_group_builder_len_and_is_empty() {
        let empty = PluginGroupBuilder::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let with_one = PluginGroupBuilder::new().add(PluginA);
        assert!(!with_one.is_empty());
        assert_eq!(with_one.len(), 1);

        let with_two = PluginGroupBuilder::new().add(PluginA).add(PluginB);
        assert!(!with_two.is_empty());
        assert_eq!(with_two.len(), 2);
    }
}
