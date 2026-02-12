//! Server runtime for plugin orchestration.
//!
//! The [`Server`] is the central runtime that manages plugins and resources.
//! It is intentionally minimal — just a plugin orchestrator.
//!
//! # Philosophy
//!
//! **Everything is a plugin.** A bare server does nothing useful. All
//! functionality comes from plugins.
//!
//! ```ignore
//! // A minimal server has nothing
//! Server::new().run(); // Does nothing useful
//!
//! // Functionality comes from plugins
//! Server::new()
//!     .add_plugins(DefaultPlugins)
//!     .add_plugins(ToolsPlugin)
//!     .add_plugins(MyAgentPlugin)
//!     .run();
//! ```
//!
//! # Resource Scoping
//!
//! The server distinguishes between two types of resources:
//!
//! - **Global resources** - Server-lifetime, read-only, shared across all contexts
//! - **Local resources** - Per-context, mutable, created fresh for each execution
//!
//! ```ignore
//! // In a plugin:
//! fn build(&self, server: &mut Server) {
//!     // Global: shared across all contexts, read-only
//!     server.insert_global(Config::default());
//!
//!     // Local: fresh instance per context, mutable
//!     server.register_local(Memory::new);
//! }
//!
//! // Creating an execution context:
//! let ctx = server.create_context();
//! // ctx has access to global resources (read-only via Res<T>)
//! // ctx has fresh local resources (mutable via ResMut<T>)
//! ```
//!
//! # Lifecycle
//!
//! The server manages a strict plugin lifecycle:
//!
//! 1. **Dependency Resolution** - Validate and topologically sort plugins
//! 2. **Build Phase** - Call `plugin.build()` in dependency order
//! 3. **Ready Phase** - Call `plugin.ready()` in dependency order
//! 4. **Run Loop** - Execute systems and call `plugin.update()` (Layer 2)
//! 5. **Cleanup Phase** - Call `plugin.cleanup()` in reverse order

use crate::api::API;
use crate::param::SystemContext;
use crate::plugin::{Plugin, PluginId, Plugins, ScheduleId};
use crate::resource::{
    GlobalResource, LocalResource, Resource, ResourceRef, ResourceRefMut, Resources,
};
use core::any::TypeId;
use hashbrown::{HashMap, HashSet};

// ─────────────────────────────────────────────────────────────────────────────
// Server
// ─────────────────────────────────────────────────────────────────────────────

/// Type-erased resource for dynamic storage.
type BoxedResource = Box<dyn core::any::Any + Send + Sync>;

/// Factory function that creates a local resource instance.
type LocalFactory = Box<dyn Fn() -> BoxedResource + Send + Sync>;

/// Type-erased API for dynamic storage.
type BoxedAPI = Box<dyn core::any::Any + Send + Sync>;

/// Represents the build state of the server.
///
/// The server progresses through these states linearly:
/// `NotStarted` → `Building` → `Built`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BuildState {
    /// Server has not started building yet (initial state).
    #[default]
    NotStarted,
    /// Server is currently in the build phase (`finish()` is executing).
    Building,
    /// Server has completed building (`finish()` has returned).
    Built,
}

/// The runtime that orchestrates plugins and manages resources.
///
/// # Example
///
/// ```ignore
/// use polaris_system::server::Server;
///
/// Server::new()
///     .add_plugins(TracingPlugin::default())
///     .add_plugins(MyPlugin)
///     .run();
/// ```
pub struct Server {
    /// Global resources (server-lifetime, read-only, shared across all contexts).
    ///
    /// Registered via [`insert_global()`](Self::insert_global).
    /// Accessed via `Res<T>` (not `ResMut<T>`).
    global: Resources,

    /// Resources field for server-wide mutable storage.
    ///
    /// Resources inserted via [`insert_resource()`](Self::insert_resource) go here.
    /// We keep this separate from `global` for mutable access to resources not
    /// accessibble to systems via `Res<T>` and `ResMut<T>`. This is useful
    /// for plugins that need mutable server-wide state.
    /// Note: This is safe because Plugins' `update()` calls are not run concurrently.
    resources: Resources,

    /// Factories for creating per-context local resources.
    ///
    /// Registered via [`register_local()`](Self::register_local).
    /// Each call to [`create_context()`](Self::create_context) invokes these factories
    /// to create fresh resource instances.
    local_factories: HashMap<TypeId, LocalFactory>,

    /// APIs for plugin orchestration (build-time capability registries).
    ///
    /// Registered via [`insert_api()`](Self::insert_api).
    /// Accessed via [`api()`](Self::api) by plugins during build/ready phases.
    /// Unlike resources, APIs are not accessed by systems.
    apis: HashMap<TypeId, BoxedAPI>,

    /// Plugins pending build (not yet sorted).
    pending_plugins: Vec<PluginEntry>,

    /// Plugins that have been built, in sorted order.
    built_plugins: Vec<PluginEntry>,

    /// Set of plugin IDs that have been added (for duplicate detection).
    plugin_ids: HashSet<PluginId>,

    /// Maps schedule → plugin indices that registered for it.
    ///
    /// Indices are in dependency order (same as `built_plugins`).
    /// Built during `finish()` from plugin `tick_schedules()`.
    schedule_registry: HashMap<ScheduleId, Vec<usize>>,

    /// The current build state of the server.
    ///
    /// Progresses linearly: `NotStarted` → `Building` → `Built`.
    build_state: BuildState,
}

/// Internal entry for a registered plugin.
struct PluginEntry {
    /// The plugin's unique identifier.
    ///
    /// Used for dependency resolution and duplicate detection.
    #[expect(dead_code, reason = "Stored for future use in dependency graphs")]
    id: PluginId,

    /// The plugin instance.
    plugin: Box<dyn Plugin>,

    /// The plugin's name (cached for error messages).
    name: String,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    /// Creates a new empty server.
    ///
    /// The server starts with no plugins and no resources.
    #[must_use]
    pub fn new() -> Self {
        Self {
            global: Resources::new(),
            resources: Resources::new(),
            local_factories: HashMap::new(),
            apis: HashMap::new(),
            pending_plugins: Vec::new(),
            built_plugins: Vec::new(),
            plugin_ids: HashSet::new(),
            schedule_registry: HashMap::new(),
            build_state: BuildState::NotStarted,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Plugin Management
    // ─────────────────────────────────────────────────────────────────────────

    /// Adds one or more plugins to the server.
    ///
    /// Accepts either:
    /// - A single plugin implementing [`Plugin`]
    /// - A [`PluginGroupBuilder`](crate::plugin::PluginGroupBuilder) containing multiple plugins
    ///
    /// # Panics
    ///
    /// Panics if a unique plugin is added twice.
    ///
    /// # Example
    ///
    /// ```ignore
    /// server
    ///     .add_plugins(TracingPlugin::default())
    ///     .add_plugins(DefaultPlugins)
    ///     .add_plugins(MyPlugin);
    /// ```
    pub fn add_plugins<P: Plugins>(&mut self, plugins: P) -> &mut Self {
        plugins.add_to_server(self);
        self
    }

    /// Internal method to add a boxed plugin with its captured ID.
    ///
    /// Called by [`Plugins::add_to_server`] implementations.
    ///
    /// # Arguments
    ///
    /// * `id` - The plugin's unique identifier (captured before boxing)
    /// * `plugin` - The boxed plugin instance
    pub(crate) fn add_plugin_boxed(&mut self, id: PluginId, plugin: Box<dyn Plugin>) {
        let name = plugin.name().to_string();

        // For unique plugins, check if already added
        if plugin.is_unique() && self.plugin_ids.contains(&id) {
            panic!(
                "Plugin '{}' is unique and was already added.\n\
                 If you intended to add this plugin multiple times, \
                 set `is_unique()` to return `false`.",
                name
            );
        }

        // Track this plugin ID
        self.plugin_ids.insert(id);

        let entry = PluginEntry { id, plugin, name };

        // If we're in the build phase, the plugin is built immediately
        if self.build_state == BuildState::Building {
            // Build immediately and add to built list
            entry.plugin.build(self);
            self.built_plugins.push(entry);
        } else {
            // Queue for later
            self.pending_plugins.push(entry);
        }
    }

    /// Returns true if a plugin of the given type has been added.
    #[must_use]
    pub fn has_plugin<P: Plugin>(&self) -> bool {
        let name = core::any::type_name::<P>();
        self.pending_plugins
            .iter()
            .chain(self.built_plugins.iter())
            .any(|p| p.name == name)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Resource Access
    // ─────────────────────────────────────────────────────────────────────────

    /// Inserts a resource into the server.
    ///
    /// If a resource of this type already exists, it is replaced and the
    /// old value is returned.
    ///
    /// # Example
    ///
    /// ```ignore
    /// server.insert_resource(MyConfig { value: 42 });
    /// ```
    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> Option<R> {
        self.resources.insert(resource)
    }

    /// Returns true if a resource of type `R` exists.
    #[must_use]
    pub fn contains_resource<R: Resource>(&self) -> bool {
        self.resources.contains::<R>()
    }

    /// Gets an immutable reference to a resource.
    ///
    /// Returns `None` if the resource doesn't exist or is mutably borrowed.
    #[must_use]
    pub fn get_resource<R: Resource>(&self) -> Option<ResourceRef<R>> {
        self.resources.get::<R>().ok()
    }

    /// Gets a mutable reference to a resource.
    ///
    /// Returns `None` if the resource doesn't exist or is already borrowed.
    #[must_use]
    pub fn get_resource_mut<R: Resource>(&self) -> Option<ResourceRefMut<R>> {
        self.resources.get_mut::<R>().ok()
    }

    /// Removes a resource from the server and returns it.
    ///
    /// Returns `None` if the resource doesn't exist.
    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.resources.remove::<R>()
    }

    /// Returns a reference to the underlying resources container.
    #[must_use]
    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Returns a mutable reference to the underlying resources container.
    #[must_use]
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Scoped Resources (Global / Local)
    // ─────────────────────────────────────────────────────────────────────────

    /// Inserts a global resource into the server.
    ///
    /// Global resources are:
    /// - Server-lifetime (live as long as the server)
    /// - Read-only (accessible via `Res<T>`, not `ResMut<T>`)
    /// - Shared across all execution contexts
    ///
    /// Use this for configuration, tool registries, and other shared state
    /// that should not be modified during execution.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[derive(Resource)]
    /// #[global]
    /// struct Config { name: String }
    ///
    /// server.insert_global(Config { name: "my-agent".into() });
    ///
    /// // Later, in a system:
    /// fn my_system(config: Res<Config>) {
    ///     // Read-only access to shared config
    /// }
    /// ```
    pub fn insert_global<R: GlobalResource>(&mut self, resource: R) -> Option<R> {
        self.global.insert(resource)
    }

    /// Returns true if a global resource of type `R` exists.
    #[must_use]
    pub fn contains_global<R: GlobalResource>(&self) -> bool {
        self.global.contains::<R>()
    }

    /// Gets an immutable reference to a global resource.
    ///
    /// Returns `None` if the resource doesn't exist.
    #[must_use]
    pub fn get_global<R: GlobalResource>(&self) -> Option<ResourceRef<R>> {
        self.global.get::<R>().ok()
    }

    /// Registers a factory for creating per-context local resources.
    ///
    /// Local resources are:
    /// - Per-context (fresh instance for each [`create_context()`](Self::create_context) call)
    /// - Mutable (accessible via `ResMut<T>`)
    /// - Isolated between contexts (Agent A's state ≠ Agent B's state)
    ///
    /// Use this for agent state, memory, scratchpads, and other state
    /// that should be isolated per agent execution.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[derive(Resource)]
    /// struct Memory { messages: Vec<String> }
    ///
    /// impl Memory {
    ///     fn new() -> Self {
    ///         Self { messages: Vec::new() }
    ///     }
    /// }
    ///
    /// server.register_local(Memory::new);
    ///
    /// // Later, in a system:
    /// fn my_system(mut memory: ResMut<Memory>) {
    ///     memory.messages.push("Hello".into());
    /// }
    /// ```
    pub fn register_local<R: LocalResource>(
        &mut self,
        factory: impl Fn() -> R + Send + Sync + 'static,
    ) {
        self.local_factories
            .insert(TypeId::of::<R>(), Box::new(move || Box::new(factory())));
    }

    /// Returns true if a local resource factory for type `R` is registered.
    #[must_use]
    pub fn has_local<R: LocalResource>(&self) -> bool {
        self.local_factories.contains_key(&TypeId::of::<R>())
    }

    /// Creates an execution context with global resources and fresh local resources.
    ///
    /// The returned context:
    /// - Has read-only access to all global resources via `Res<T>`
    /// - Has mutable access to fresh local resource instances via `ResMut<T>`
    /// - Can create child contexts via [`SystemContext::child()`]
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Register resources
    /// server.insert_global(Config::default());
    /// server.register_local(Memory::new);
    ///
    /// // Create execution context
    /// let ctx = server.create_context();
    ///
    /// // Access resources
    /// let config = ctx.get_resource::<Config>().unwrap();  // From global
    /// let mut memory = ctx.get_resource_mut::<Memory>().unwrap();  // Fresh instance
    /// ```
    #[must_use]
    pub fn create_context(&self) -> SystemContext<'_> {
        // Create context with access to server's global resources
        let mut ctx = SystemContext::with_globals(&self.global);

        // Instantiate local resources from factories
        for (type_id, factory) in &self.local_factories {
            let boxed = factory();
            ctx.insert_boxed(*type_id, boxed);
        }

        ctx
    }

    /// Returns a reference to the global resources container.
    #[must_use]
    pub fn global_resources(&self) -> &Resources {
        &self.global
    }

    /// Returns whether the server has been built (i.e., `finish()` has been called).
    #[must_use]
    pub fn is_built(&self) -> bool {
        self.build_state == BuildState::Built
    }

    // ─────────────────────────────────────────────────────────────────────────
    // API Access
    // ─────────────────────────────────────────────────────────────────────────

    /// Inserts an API into the server.
    ///
    /// APIs are build-time capability registries that plugins use for orchestration.
    /// Unlike resources (accessed by systems), APIs are accessed by plugins during
    /// the build/ready phases.
    ///
    /// If an API of this type already exists, it is replaced and the old value
    /// is returned.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use polaris_system::api::API;
    ///
    /// pub struct AgentAPI { /* ... */ }
    /// impl API for AgentAPI {}
    ///
    /// // In a plugin's build():
    /// fn build(&self, server: &mut Server) {
    ///     server.insert_api(AgentAPI::new());
    /// }
    /// ```
    pub fn insert_api<A: API>(&mut self, api: A) -> Option<A> {
        let type_id = TypeId::of::<A>();
        let boxed: BoxedAPI = Box::new(api);
        self.apis
            .insert(type_id, boxed)
            .and_then(|old| old.downcast::<A>().ok())
            .map(|b| *b)
    }

    /// Gets a reference to an API.
    ///
    /// Returns `None` if the API doesn't exist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In a plugin's ready():
    /// fn ready(&self, server: &mut Server) {
    ///     let api = server.api::<AgentAPI>()
    ///         .expect("AgentAPI required");
    ///     api.register("my-agent", MyAgent::new());
    /// }
    /// ```
    #[must_use]
    pub fn api<A: API>(&self) -> Option<&A> {
        self.apis
            .get(&TypeId::of::<A>())
            .and_then(|boxed| boxed.downcast_ref::<A>())
    }

    /// Returns true if an API of type `A` exists.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if server.contains_api::<AgentAPI>() {
    ///     // AgentAPI is available
    /// }
    /// ```
    #[must_use]
    pub fn contains_api<A: API>(&self) -> bool {
        self.apis.contains_key(&TypeId::of::<A>())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Tick Methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Triggers a tick for the given schedule type.
    ///
    /// Only plugins that declared interest in this schedule via
    /// [`Plugin::tick_schedules()`] will have their [`Plugin::update()`] called.
    /// Plugins are ticked in dependency order (same as build/ready).
    ///
    /// # Safety Guarantees
    ///
    /// The `built_plugins` array cannot be modified during tick:
    /// - `built_plugins` is a private field, inaccessible to plugins
    /// - Calling `add_plugins()` during update queues to `pending_plugins`, not `built_plugins`
    /// - Calling `finish()` during update panics (already built)
    ///
    /// # When to Call
    ///
    /// Layer 2 (`polaris_agent`) decides when to call this method based on
    /// agent execution events. Common schedules might include:
    /// - `PostAgentRun` - After an agent completes execution
    /// - `PreTurn` / `PostTurn` - Before/after each turn in a conversation
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Layer 2 defines schedule marker types:
    /// pub struct PostAgentRun;
    ///
    /// // Layer 2 executor triggers the tick:
    /// server.tick::<PostAgentRun>();
    /// ```
    pub fn tick<S: 'static>(&mut self) {
        self.tick_schedule(ScheduleId::of::<S>());
    }

    /// Triggers a tick for the given schedule ID.
    ///
    /// Plugins are ticked in dependency order.
    /// This is the non-generic version of [`tick()`](Self::tick).
    pub fn tick_schedule(&mut self, schedule: ScheduleId) {
        let Some(plugin_indices) = self.schedule_registry.get(&schedule) else {
            return;
        };

        // Clone indices to avoid borrow conflict with &mut self passed to update()
        let indices: Vec<usize> = plugin_indices.clone();

        for idx in indices {
            let plugin_ptr = &self.built_plugins[idx].plugin as *const Box<dyn Plugin>;
            // SAFETY: built_plugins cannot be modified during this loop:
            // - It's a private field, inaccessible to plugin code
            // - add_plugins() during update goes to pending_plugins (build_state is Built)
            // - finish() during update panics (build_state is not NotStarted)
            // The pointer remains valid throughout the loop.
            unsafe {
                (*plugin_ptr).update(self, schedule);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Lifecycle Methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Builds all plugins and prepares the server for execution.
    ///
    /// This method:
    /// 1. Validates all plugin dependencies exist
    /// 2. Topologically sorts plugins by dependencies
    /// 3. Calls `build()` on each plugin in order
    /// 4. Calls `ready()` on each plugin in order
    ///
    /// # Panics
    ///
    /// - If a plugin's dependency is not satisfied
    /// - If there is a circular dependency between plugins
    /// - If called more than once
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut server = Server::new();
    /// server.add_plugins(MyPlugin);
    /// server.finish(); // Build and ready all plugins
    /// ```
    pub fn finish(&mut self) {
        if self.build_state != BuildState::NotStarted {
            panic!("Server::finish() was already called. Cannot build twice.");
        }

        // Phase 1: Sort plugins by dependencies
        let sorted_plugins = self.sort_plugins_by_dependencies();

        // Phase 2: Build all plugins in sorted order
        self.build_state = BuildState::Building;
        for entry in sorted_plugins {
            entry.plugin.build(self);
            self.built_plugins.push(entry);
        }

        // Phase 3: Ready all plugins in sorted order
        // We need to iterate by index since ready() takes &mut Server
        for i in 0..self.built_plugins.len() {
            // SAFETY: We're using index-based access to avoid borrow conflicts
            // The plugin is borrowed immutably, and we pass &mut self to ready()
            let plugin_ptr = &self.built_plugins[i].plugin as *const Box<dyn Plugin>;
            // SAFETY: We don't modify built_plugins during this loop, and the
            // pointer remains valid. The plugin's ready() may add resources but
            // shouldn't modify built_plugins.
            unsafe {
                (*plugin_ptr).ready(self);
            }
        }

        // Phase 4: Build schedule registry from plugin tick_schedules()
        self.build_schedule_registry();

        self.build_state = BuildState::Built;
    }

    /// Builds the schedule registry from plugin `tick_schedules()` declarations.
    ///
    /// Called at the end of `finish()`. Maps each schedule to the indices of
    /// plugins that registered for it, preserving dependency order.
    fn build_schedule_registry(&mut self) {
        self.schedule_registry.clear();

        // Iterate in dependency order (built_plugins is already sorted)
        for (idx, entry) in self.built_plugins.iter().enumerate() {
            for schedule in entry.plugin.tick_schedules() {
                self.schedule_registry
                    .entry(schedule)
                    .or_default()
                    .push(idx);
            }
        }
    }

    /// Runs the server lifecycle.
    ///
    /// This is a convenience method that calls `finish()` and then returns.
    /// The full run loop with `update()` calls will be added in Layer 2.
    ///
    /// # Panics
    ///
    /// Same as [`finish()`](Self::finish).
    pub fn run(&mut self) {
        self.finish();
        // Run loop will be added in Layer 2
    }

    /// Runs build and ready phases, then returns.
    ///
    /// This is an alias for `finish()`, intended for testing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[test]
    /// fn test_plugin() {
    ///     let mut server = Server::new();
    ///     server.add_plugins(MyPlugin);
    ///     server.run_once();
    ///
    ///     assert!(server.contains_resource::<MyResource>());
    /// }
    /// ```
    pub fn run_once(&mut self) {
        self.finish();
    }

    /// Cleans up all plugins in reverse dependency order.
    ///
    /// Call this when shutting down the server to allow plugins to
    /// gracefully release resources.
    ///
    /// # Example
    ///
    /// ```ignore
    /// server.run();
    /// // ... server is running ...
    /// server.cleanup(); // Graceful shutdown
    /// ```
    pub fn cleanup(&mut self) {
        // Cleanup in reverse order (dependents before dependencies)
        for i in (0..self.built_plugins.len()).rev() {
            let plugin_ptr = &self.built_plugins[i].plugin as *const Box<dyn Plugin>;
            // SAFETY: Same as ready() - we don't modify built_plugins during cleanup
            unsafe {
                (*plugin_ptr).cleanup(self);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Internal: Dependency Resolution
    // ─────────────────────────────────────────────────────────────────────────

    /// Sorts pending plugins by dependencies using topological sort.
    ///
    /// Returns the sorted list of plugins.
    ///
    /// # Panics
    ///
    /// - If a plugin's dependency is not found
    /// - If there is a circular dependency
    fn sort_plugins_by_dependencies(&mut self) -> Vec<PluginEntry> {
        if self.pending_plugins.is_empty() {
            return Vec::new();
        }

        // Build a map of plugin name -> index for dependency lookup
        let mut name_to_index: HashMap<String, usize> = HashMap::new();
        for (i, entry) in self.pending_plugins.iter().enumerate() {
            name_to_index.insert(entry.name.clone(), i);
        }

        // Build adjacency list and compute in-degrees
        let n = self.pending_plugins.len();
        let mut in_degree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, entry) in self.pending_plugins.iter().enumerate() {
            for dep_id in entry.plugin.dependencies() {
                let dep_name = dep_id.type_name();

                // Find the dependency in pending plugins
                if let Some(&dep_idx) = name_to_index.get(dep_name) {
                    // dep_idx must be built before i
                    dependents[dep_idx].push(i);
                    in_degree[i] += 1;
                } else {
                    // Check if already built
                    if !self.built_plugins.iter().any(|p| p.name == dep_name) {
                        panic!(
                            "Plugin '{}' requires '{}' which was not added.\n\
                             Add {} before {}, or use a plugin group that includes it.",
                            entry.name, dep_name, dep_name, entry.name
                        );
                    }
                    // Dependency already built, no need to track
                }
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue: Vec<usize> = Vec::new();
        for (i, &deg) in in_degree.iter().enumerate() {
            if deg == 0 {
                queue.push(i);
            }
        }

        let mut sorted_indices: Vec<usize> = Vec::with_capacity(n);

        while let Some(idx) = queue.pop() {
            sorted_indices.push(idx);

            for &dependent_idx in &dependents[idx] {
                in_degree[dependent_idx] -= 1;
                if in_degree[dependent_idx] == 0 {
                    queue.push(dependent_idx);
                }
            }
        }

        // Check for cycle
        if sorted_indices.len() != n {
            // Find plugins involved in cycle
            let in_cycle: Vec<&str> = in_degree
                .iter()
                .enumerate()
                .filter(|(_, deg)| **deg > 0)
                .map(|(i, _)| self.pending_plugins[i].name.as_str())
                .collect();

            panic!(
                "Circular dependency detected among plugins: {:?}\n\
                 Break the cycle by extracting shared functionality into a separate plugin.",
                in_cycle
            );
        }

        // Extract plugins in sorted order
        // We need to drain pending_plugins while preserving order
        let mut pending = core::mem::take(&mut self.pending_plugins);

        // Create a mapping from old index to new position
        let mut old_to_new: Vec<Option<usize>> = vec![None; n];
        for (new_pos, &old_idx) in sorted_indices.iter().enumerate() {
            old_to_new[old_idx] = Some(new_pos);
        }

        // Sort pending by the new order
        // We'll collect into a vec of Options, then unwrap
        let mut result: Vec<Option<PluginEntry>> = (0..n).map(|_| None).collect();
        for (old_idx, entry) in pending.drain(..).enumerate() {
            let new_pos = old_to_new[old_idx].expect("all indices should be mapped");
            result[new_pos] = Some(entry);
        }

        result.into_iter().flatten().collect()
    }
}
