//! System parameter injection.
//!
//! This module provides the dependency injection mechanism for systems.
//! System functions declare parameters that implement [`SystemParam`],
//! and these are automatically resolved from the [`SystemContext`] at runtime.
//!
//! # Core Types
//!
//! - [`Res<T>`] - Read-only access to a resource (walks hierarchy)
//! - [`ResMut<T>`] - Mutable access to a local resource (current scope only)
//! - [`Out<T>`] - Read previous system's output (ephemeral, per-execution)
//!
//! # Hierarchical Resource Model
//!
//! The [`SystemContext`] supports hierarchical scoping through parent-child
//! relationships. This enables multi-agent isolation:
//!
//! - `Res<T>` walks up the parent chain to find resources (read-only)
//! - `ResMut<T>` only accesses resources in the current scope (mutable)
//!
//! ```text
//! Server (global resources: Config, ToolRegistry)
//!    │
//!    └── Agent Context (local: AgentMemory)
//!           │
//!           └── Session Context (local: ConversationHistory)
//!                  │
//!                  └── Turn Context (local: Scratchpad)
//! ```
//!
//! # Access Descriptors
//!
//! Each parameter declares its access pattern via [`SystemParam::access()`].
//! This enables conflict detection between systems at scheduling time:
//!
//! - Read + Read: OK (multiple readers)
//! - Read + Write: CONFLICT
//! - Write + Write: CONFLICT
//!
//! # Example
//!
//! ```ignore
//! use polaris_system::param::{Res, ResMut, Out};
//!
//! // Res<Config> reads from global (via parent chain)
//! // ResMut<Memory> writes to local scope only
//! fn process(
//!     config: Res<Config>,           // Global, read-only (via parent chain)
//!     mut memory: ResMut<Memory>,    // Local, mutable (current scope)
//!     previous: Out<PreviousResult>, // Previous system's output
//! ) -> ProcessResult {
//!     memory.record(&previous);
//!     ProcessResult::new(&config, &previous)
//! }
//! ```

mod access;

use variadics_please::all_tuples;

pub use access::{Access, AccessMode, SystemAccess};

use crate::resource::{
    LocalResource, Output, OutputRef, Outputs, Resource, ResourceRef, ResourceRefMut, Resources,
};

/// A parameter that can be injected into a system function.
///
/// Types implementing this trait can appear as parameters in system functions.
/// The framework automatically fetches them from the [`SystemContext`] before execution.
///
/// # GAT Design
///
/// The `Item<'w>` GAT (Generic Associated Type) enables lifetime-parameterized
/// params like `Res<T>` to work with `IntoSystem`. The GAT produces the
/// correctly-lifetimed type when fetching from the context.
///
/// # Access Declaration
///
/// Each parameter declares its access pattern via [`access()`](Self::access),
/// enabling conflict detection between systems at scheduling time.
///
/// # Built-in Implementations
///
/// - [`Res<T>`] - Immutable resource access (long-lived state)
/// - [`ResMut<T>`] - Mutable resource access (long-lived state)
/// - [`Out<T>`] - Previous system output (ephemeral)
/// - `()` - Unit type (no parameter)
/// - Tuples of `SystemParam` types
pub trait SystemParam: Sized {
    /// The item type produced when fetching, parameterized by context lifetime.
    ///
    /// This GAT allows `IntoSystem` to use HRTB bounds like `for<'w> Fn(P::Item<'w>)`,
    /// enabling functions with `Res<T>` params to satisfy the trait bounds.
    type Item<'w>: SystemParam;

    /// Fetches this parameter from the system context.
    ///
    /// # Errors
    ///
    /// Returns [`ParamError`] if the parameter cannot be fetched
    /// (e.g., resource not found, borrow conflict).
    fn fetch<'w>(ctx: &'w SystemContext<'_>) -> Result<Self::Item<'w>, ParamError>;

    /// Declares the access pattern for this parameter.
    ///
    /// Used by the scheduler to detect conflicts between systems.
    /// The default implementation returns empty access (no conflicts).
    fn access() -> SystemAccess {
        SystemAccess::default()
    }
}

/// Errors that can occur when fetching system parameters.
#[derive(Debug, thiserror::Error)]
pub enum ParamError {
    /// The requested resource was not found.
    #[error("resource not found: {0}")]
    ResourceNotFound(&'static str),

    /// A borrow conflict occurred (e.g., trying to mutably borrow
    /// a resource that is already borrowed).
    #[error("borrow conflict: {0}")]
    BorrowConflict(&'static str),

    /// The requested output was not found (no system has produced it yet).
    #[error("output not found: {0}")]
    OutputNotFound(&'static str),
}

/// The execution context passed to systems during execution.
///
/// `SystemContext` provides hierarchical access to resources and outputs:
/// - **Resources**: Owned by this scope, with read access to parent scopes
/// - **Global Resources**: Read-only access to server-level shared state
/// - **Outputs**: Owned, ephemeral system outputs for current execution
///
/// # Hierarchical Scoping
///
/// Contexts form a parent-child hierarchy for resource isolation, with global
/// resources accessible from all contexts:
///
/// ```text
/// Server (global resources: Config, ToolRegistry)
///    │
///    └── Agent Context (local: AgentMemory)
///           │
///           └── Session Context (local: ConversationHistory)
///                  │
///                  └── Turn Context (local: Scratchpad)
/// ```
///
/// Resource lookup order for `Res<T>`:
/// 1. Check local resources (current scope)
/// 2. Walk up parent chain (shadowing: closest wins)
/// 3. Check global resources (server-level)
///
/// `ResMut<T>` only accesses resources in the current scope (mutable).
///
/// # Ownership Model
///
/// ```text
/// SystemContext<'parent>
/// ├── parent: Option<&'parent SystemContext>  (read-only parent access)
/// ├── globals: Option<&'parent Resources>     (server's global resources)
/// ├── resources: Resources                     (owned, this scope's state)
/// └── outputs: Outputs                         (owned, per-execution)
/// ```
///
/// The `globals` reference is inherited by child contexts, so all contexts
/// in a hierarchy can access server-level resources.
pub struct SystemContext<'parent> {
    /// Parent context for hierarchical resource lookup.
    /// Read access walks up this chain; write access is current-scope only.
    parent: Option<&'parent SystemContext<'parent>>,
    /// Reference to server's global resources.
    /// Checked after parent chain is exhausted. Inherited by child contexts.
    globals: Option<&'parent Resources>,
    /// Resources owned by this scope.
    resources: Resources,
    /// Ephemeral system outputs for current execution (owned).
    outputs: Outputs,
}

impl Default for SystemContext<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'parent> SystemContext<'parent> {
    /// Creates a new root context with no parent or global resources.
    ///
    /// Resources and outputs are initialized empty. Use [`with_globals`](Self::with_globals)
    /// to create a context that can access server-level resources.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parent: None,
            globals: None,
            resources: Resources::new(),
            outputs: Outputs::new(),
        }
    }

    /// Creates a new context with access to global resources.
    ///
    /// This is typically called by [`Server::create_context()`] to create
    /// execution contexts that can access server-level resources via `Res<T>`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ctx = SystemContext::with_globals(server.global_resources());
    /// // ctx can now access global resources via Res<T>
    /// ```
    #[must_use]
    pub fn with_globals(globals: &'parent Resources) -> Self {
        Self {
            parent: None,
            globals: Some(globals),
            resources: Resources::new(),
            outputs: Outputs::new(),
        }
    }

    /// Builder pattern: inserts a resource and returns self.
    ///
    /// Useful for chaining insertions when creating a context.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ctx = SystemContext::new()
    ///     .with(Counter { value: 0 })
    ///     .with(Config { name: "test".into() });
    /// ```
    #[must_use]
    pub fn with<R: LocalResource>(mut self, resource: R) -> Self {
        self.insert(resource);
        self
    }

    /// Creates a child context with this context as its parent.
    ///
    /// The child can read resources from this context (and its ancestors)
    /// but has its own local resources for writes. The child inherits the
    /// globals reference, so it can access server-level resources.
    #[must_use]
    pub fn child(&'parent self) -> SystemContext<'parent> {
        SystemContext {
            parent: Some(self),
            globals: self.globals,
            resources: Resources::new(),
            outputs: Outputs::new(),
        }
    }

    /// Inserts a local resource into this context's scope.
    ///
    /// This resource will shadow any resource of the same type in parent scopes
    /// for read access, and will be the target for mutable access.
    pub fn insert<R: LocalResource>(&mut self, resource: R) {
        self.resources.insert(resource);
    }

    /// Inserts any resource into this context's scope.
    ///
    /// This is primarily used for root contexts that hold global resources,
    /// or for testing. For normal usage, prefer [`insert`] which enforces
    /// the `LocalResource` bound.
    ///
    /// Note: Resources inserted this way can still only be mutated via
    /// `ResMut<T>` if they implement `LocalResource`.
    pub fn insert_resource<R: Resource>(&mut self, resource: R) {
        self.resources.insert(resource);
    }

    /// Inserts a type-erased resource into this context's scope.
    ///
    /// This is used internally by the server to instantiate local resources
    /// from factories. The `type_id` must match the correct type of the boxed
    /// resource.
    pub fn insert_boxed(
        &mut self,
        type_id: core::any::TypeId,
        resource: Box<dyn core::any::Any + Send + Sync>,
    ) {
        self.resources.insert_boxed(type_id, resource);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Resource methods (hierarchical lookup)
    // ─────────────────────────────────────────────────────────────────────

    /// Returns `true` if a resource of type `R` exists in this scope, any parent, or globals.
    #[must_use]
    pub fn contains_resource<R: Resource>(&self) -> bool {
        if self.resources.contains::<R>() {
            return true;
        }
        if let Some(parent) = self.parent {
            return parent.contains_resource::<R>();
        }
        if let Some(globals) = self.globals {
            return globals.contains::<R>();
        }
        false
    }

    /// Returns `true` if a resource of type `R` exists in this scope only.
    #[must_use]
    pub fn contains_local_resource<R: Resource>(&self) -> bool {
        self.resources.contains::<R>()
    }

    /// Gets an immutable reference to a resource, walking up the hierarchy.
    ///
    /// Lookup order:
    /// 1. Check this context's local resources
    /// 2. Walk up the parent chain (closest wins for shadowing)
    /// 3. Check global resources (server-level)
    ///
    /// # Errors
    ///
    /// Returns an error if the resource doesn't exist in any scope
    /// or is mutably borrowed.
    pub fn get_resource<R: Resource>(&self) -> Result<ResourceRef<R>, ParamError> {
        // Check local scope first
        match self.resources.get::<R>() {
            Ok(r) => return Ok(r),
            Err(crate::resource::ResourceError::BorrowConflict(name)) => {
                return Err(ParamError::BorrowConflict(name));
            }
            Err(crate::resource::ResourceError::NotFound(_)) => {
                // Not in local scope, try parent
            }
        }

        // Walk up to parent
        if let Some(parent) = self.parent {
            return parent.get_resource::<R>();
        }

        // Check global resources (server-level)
        if let Some(globals) = self.globals {
            match globals.get::<R>() {
                Ok(r) => return Ok(r),
                Err(crate::resource::ResourceError::BorrowConflict(name)) => {
                    return Err(ParamError::BorrowConflict(name));
                }
                Err(crate::resource::ResourceError::NotFound(_)) => {
                    // Not in globals either
                }
            }
        }

        Err(ParamError::ResourceNotFound(core::any::type_name::<R>()))
    }

    /// Gets a mutable reference to a resource in the current scope only.
    ///
    /// Unlike `get_resource`, this does NOT walk up the parent chain.
    /// Only resources in this context's local scope can be mutated.
    ///
    /// # Errors
    ///
    /// Returns an error if the resource doesn't exist in this scope
    /// or is already borrowed.
    pub fn get_resource_mut<R: Resource>(&self) -> Result<ResourceRefMut<R>, ParamError> {
        self.resources.get_mut::<R>().map_err(|err| match err {
            crate::resource::ResourceError::NotFound(name) => ParamError::ResourceNotFound(name),
            crate::resource::ResourceError::BorrowConflict(name) => {
                ParamError::BorrowConflict(name)
            }
        })
    }

    /// Returns a reference to this scope's local resources.
    #[must_use]
    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Returns a reference to the parent context, if any.
    #[must_use]
    pub fn parent(&self) -> Option<&SystemContext<'parent>> {
        self.parent
    }

    /// Returns a reference to the global resources, if any.
    #[must_use]
    pub fn globals(&self) -> Option<&Resources> {
        self.globals
    }

    /// Returns `true` if a resource with the given `TypeId` exists in this scope,
    /// any parent, or globals.
    ///
    /// This is useful for validation when the concrete type is not known
    /// at compile time (e.g., validating system access declarations).
    #[must_use]
    pub fn contains_resource_by_type_id(&self, type_id: core::any::TypeId) -> bool {
        if self.resources.contains_by_type_id(type_id) {
            return true;
        }
        if let Some(parent) = self.parent {
            return parent.contains_resource_by_type_id(type_id);
        }
        if let Some(globals) = self.globals {
            return globals.contains_by_type_id(type_id);
        }
        false
    }

    /// Returns `true` if a resource with the given `TypeId` exists in this scope only.
    ///
    /// This is useful for validating mutable access (`ResMut`) which only operates
    /// on the current scope.
    #[must_use]
    pub fn contains_local_resource_by_type_id(&self, type_id: core::any::TypeId) -> bool {
        self.resources.contains_by_type_id(type_id)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Output methods (owned, ephemeral system return values)
    // ─────────────────────────────────────────────────────────────────────

    /// Inserts a system output.
    ///
    /// Called by the executor after a system returns a value.
    /// If an output of this type already exists, it is replaced.
    pub fn insert_output<O: Output>(&mut self, output: O) {
        self.outputs.insert(output);
    }

    /// Inserts a type-erased system output.
    ///
    /// Called by the executor when the concrete output type is not known
    /// at compile time. The `type_id` must match the correct type of the value.
    pub fn insert_output_boxed(
        &mut self,
        type_id: core::any::TypeId,
        output: Box<dyn core::any::Any + Send + Sync>,
    ) {
        self.outputs.insert_boxed(type_id, output);
    }

    /// Returns `true` if an output of type `O` exists.
    #[must_use]
    pub fn contains_output<O: Output>(&self) -> bool {
        self.outputs.contains::<O>()
    }

    /// Returns `true` if an output with the given `TypeId` exists.
    ///
    /// This is useful for validation when the concrete type is not known
    /// at compile time (e.g., validating system access declarations).
    #[must_use]
    pub fn contains_output_by_type_id(&self, type_id: core::any::TypeId) -> bool {
        self.outputs.contains_by_type_id(type_id)
    }

    /// Gets an immutable reference to an output.
    ///
    /// # Errors
    ///
    /// Returns an error if the output doesn't exist.
    pub fn get_output<O: Output>(&self) -> Result<OutputRef<O>, ParamError> {
        self.outputs.get::<O>().map_err(|err| match err {
            crate::resource::OutputError::NotFound(name) => ParamError::OutputNotFound(name),
            crate::resource::OutputError::Busy(name) => ParamError::BorrowConflict(name),
        })
    }

    /// Clears all outputs.
    ///
    /// Called by the executor between agent runs to reset ephemeral state.
    pub fn clear_outputs(&mut self) {
        self.outputs.clear();
    }

    /// Returns a reference to the underlying outputs.
    #[must_use]
    pub fn outputs(&self) -> &Outputs {
        &self.outputs
    }

    /// Returns a mutable reference to the underlying outputs.
    #[must_use]
    pub fn outputs_mut(&mut self) -> &mut Outputs {
        &mut self.outputs
    }

    /// Takes ownership of this context's outputs, replacing them with an empty container.
    ///
    /// This is used to extract outputs from child contexts (e.g., after parallel
    /// branch execution) before dropping them, so outputs can be merged into the
    /// parent context without borrow conflicts.
    #[must_use]
    pub fn take_outputs(&mut self) -> Outputs {
        core::mem::take(&mut self.outputs)
    }
}

/// Immutable access to a resource.
///
/// `Res<T>` provides read-only access to a resource stored in the [`SystemContext`].
/// Multiple systems can hold `Res<T>` to the same resource simultaneously.
///
/// # Example
///
/// ```ignore
/// fn read_config(config: Res<Config>) {
///     println!("Debug mode: {}", config.debug);
/// }
/// ```
pub struct Res<'w, T: Resource> {
    inner: ResourceRef<'w, T>,
}

impl<'w, T: Resource> core::ops::Deref for Res<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// SystemParam impl for Res with ANY lifetime 'a
// The GAT produces Res<'w, T> with the context's lifetime
impl<'a, T: Resource> SystemParam for Res<'a, T> {
    type Item<'w> = Res<'w, T>;

    fn fetch<'w>(ctx: &'w SystemContext<'_>) -> Result<Self::Item<'w>, ParamError> {
        let inner = ctx.get_resource::<T>()?;
        Ok(Res { inner })
    }

    fn access() -> SystemAccess {
        SystemAccess::new().with_read::<T>()
    }
}

/// Mutable access to a resource.
///
/// `ResMut<T>` provides read-write access to a resource stored in the [`SystemContext`].
/// Only one system can hold `ResMut<T>` to a resource at a time.
///
/// # `LocalResource` Requirement
///
/// `ResMut<T>` requires `T: LocalResource`. This enforces at compile time that
/// only per-context resources can be mutated. Global resources (marked with
/// `GlobalResource`) are read-only and can only be accessed via `Res<T>`.
///
/// ```ignore
/// // This works - Memory is LocalResource
/// fn update_memory(mut memory: ResMut<Memory>) {
///     memory.messages.push(new_message);
/// }
///
/// // This fails to compile - Config is GlobalResource
/// fn bad_update(mut config: ResMut<Config>) {  // Error!
///     config.name = "new".into();
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// struct Counter { value: i32 }
/// impl LocalResource for Counter {}
///
/// fn increment_counter(mut counter: ResMut<Counter>) {
///     counter.value += 1;
/// }
/// ```
pub struct ResMut<'w, T: LocalResource> {
    inner: ResourceRefMut<'w, T>,
}

impl<'w, T: LocalResource> core::ops::Deref for ResMut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'w, T: LocalResource> core::ops::DerefMut for ResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// SystemParam impl for ResMut with ANY lifetime 'a
// Requires T: LocalResource for compile-time safety
impl<'a, T: LocalResource> SystemParam for ResMut<'a, T> {
    type Item<'w> = ResMut<'w, T>;

    fn fetch<'w>(ctx: &'w SystemContext<'_>) -> Result<Self::Item<'w>, ParamError> {
        let inner = ctx.get_resource_mut::<T>()?;
        Ok(ResMut { inner })
    }

    fn access() -> SystemAccess {
        SystemAccess::new().with_write::<T>()
    }
}

/// Read-only access to a previous system's output.
///
/// `Out<T>` provides access to ephemeral data produced by a previous system
/// in the current execution. Unlike [`Res<T>`] which accesses long-lived
/// shared state, `Out<T>` reads from the outputs container which is cleared
/// between agent runs.
///
/// # When to Use
///
/// - **`Out<T>`**: Reading a previous system's return value (ephemeral)
/// - **`Res<T>`**: Reading long-lived shared state (Memory, Config, etc.)
///
/// # Example
///
/// ```ignore
/// // System A returns a value (stored as output by executor)
/// fn reason(llm: Res<LLM>) -> ReasoningResult {
///     ReasoningResult { action: "search".into() }
/// }
///
/// // System B reads System A's output
/// fn execute(reasoning: Out<ReasoningResult>, tools: Res<Tools>) -> ToolResult {
///     tools.execute(&reasoning.action)
/// }
/// ```
pub struct Out<'w, T: Output> {
    inner: OutputRef<'w, T>,
}

impl<'w, T: Output> core::ops::Deref for Out<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// SystemParam impl for Out with ANY lifetime 'a
impl<'a, T: Output> SystemParam for Out<'a, T> {
    type Item<'w> = Out<'w, T>;

    fn fetch<'w>(ctx: &'w SystemContext<'_>) -> Result<Self::Item<'w>, ParamError> {
        let inner = ctx.get_output::<T>()?;
        Ok(Out { inner })
    }

    fn access() -> SystemAccess {
        // Out<T> reads from outputs (previous system's return value)
        // We track this as output read access
        let mut access = SystemAccess::new();
        access.outputs.push(Access::read::<T>());
        access
    }
}

/// Optional output access.
///
/// Returns `None` if the output doesn't exist instead of erroring.
impl<'a, T: Output> SystemParam for Option<Out<'a, T>> {
    type Item<'w> = Option<Out<'w, T>>;

    fn fetch<'w>(ctx: &'w SystemContext<'_>) -> Result<Self::Item<'w>, ParamError> {
        match <Out<'a, T> as SystemParam>::fetch(ctx) {
            Ok(out) => Ok(Some(out)),
            Err(ParamError::OutputNotFound(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn access() -> SystemAccess {
        <Out<'a, T> as SystemParam>::access()
    }
}

// Unit type implementation
impl SystemParam for () {
    type Item<'w> = ();

    fn fetch<'w>(_ctx: &'w SystemContext<'_>) -> Result<Self::Item<'w>, ParamError> {
        Ok(())
    }
}

// Tuple implementations for multiple parameters
macro_rules! impl_system_param_tuple {
    ($($param:ident),*) => {
        impl<$($param: SystemParam),*> SystemParam for ($($param,)*) {
            type Item<'w> = ($($param::Item<'w>,)*);

            fn fetch<'w>(ctx: &'w SystemContext<'_>) -> Result<Self::Item<'w>, ParamError> {
                Ok(($($param::fetch(ctx)?,)*))
            }

            fn access() -> SystemAccess {
                let mut access = SystemAccess::new();
                $(access.merge(&$param::access());)*
                access
            }
        }
    };
}

// Generate impls for tuples of size 1 to 8
all_tuples!(impl_system_param_tuple, 1, 8, P);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::LocalResource;

    #[derive(Debug, PartialEq)]
    struct Counter {
        value: i32,
    }

    // Counter is a LocalResource - can be mutated via ResMut<Counter>
    impl LocalResource for Counter {}

    #[derive(Debug, PartialEq)]
    struct Config {
        name: String,
    }

    // Config is also LocalResource for these tests
    // (In real usage, Config would likely be GlobalResource)
    impl LocalResource for Config {}

    #[test]
    fn context_get_resource() {
        let ctx = SystemContext::new().with(Counter { value: 42 });
        let counter = ctx.get_resource::<Counter>().unwrap();
        assert_eq!(counter.value, 42);
    }

    #[test]
    fn res_fetch() {
        let ctx = SystemContext::new().with(Counter { value: 10 });
        let res = Res::<Counter>::fetch(&ctx).unwrap();
        assert_eq!(res.value, 10);
    }

    #[test]
    fn res_mut_fetch_and_modify() {
        let ctx = SystemContext::new().with(Counter { value: 0 });
        {
            let mut res = ResMut::<Counter>::fetch(&ctx).unwrap();
            res.value += 5;
        }

        let res = Res::<Counter>::fetch(&ctx).unwrap();
        assert_eq!(res.value, 5);
    }

    #[test]
    fn multiple_res_allowed() {
        let ctx = SystemContext::new().with(Counter { value: 42 });
        let res1 = Res::<Counter>::fetch(&ctx).unwrap();
        let res2 = Res::<Counter>::fetch(&ctx).unwrap();

        assert_eq!(res1.value, res2.value);
    }

    #[test]
    fn res_mut_blocks_res() {
        let ctx = SystemContext::new().with(Counter { value: 42 });
        let _res_mut = ResMut::<Counter>::fetch(&ctx).unwrap();
        let result = Res::<Counter>::fetch(&ctx);

        assert!(matches!(result, Err(ParamError::BorrowConflict(_))));
    }

    #[test]
    fn res_blocks_res_mut() {
        let ctx = SystemContext::new().with(Counter { value: 42 });
        let _res = Res::<Counter>::fetch(&ctx).unwrap();
        let result = ResMut::<Counter>::fetch(&ctx);

        assert!(matches!(result, Err(ParamError::BorrowConflict(_))));
    }

    #[test]
    fn missing_resource_error() {
        let ctx = SystemContext::new();

        let result = Res::<Counter>::fetch(&ctx);
        assert!(matches!(result, Err(ParamError::ResourceNotFound(_))));
    }

    #[test]
    fn tuple_param_fetch() {
        let ctx = SystemContext::new()
            .with(Counter { value: 1 })
            .with(Config {
                name: "test".into(),
            });
        let (counter, config) = <(Res<Counter>, Res<Config>)>::fetch(&ctx).unwrap();
        assert_eq!(counter.value, 1);
        assert_eq!(config.name, "test");
    }

    #[test]
    fn unit_param_fetch() {
        let ctx = SystemContext::new();
        let result = <()>::fetch(&ctx);
        assert!(result.is_ok());
    }

    // ─────────────────────────────────────────────────────────────────────
    // Hierarchical context tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn child_sees_parent_resources() {
        let parent = SystemContext::new().with(Counter { value: 42 });
        let child = parent.child();

        let counter = child.get_resource::<Counter>().unwrap();
        assert_eq!(counter.value, 42);
    }

    #[test]
    fn child_can_shadow_parent() {
        let parent = SystemContext::new().with(Counter { value: 1 });
        let child = parent.child().with(Counter { value: 2 });

        // Child sees its own value
        assert_eq!(child.get_resource::<Counter>().unwrap().value, 2);
        // Parent still has original
        assert_eq!(parent.get_resource::<Counter>().unwrap().value, 1);
    }

    #[test]
    fn mutation_only_in_current_scope() {
        let parent = SystemContext::new().with(Counter { value: 1 });
        let child = parent.child();

        // Can read from parent
        assert!(child.get_resource::<Counter>().is_ok());

        // Cannot mutate parent's resource (not in child's local scope)
        assert!(child.get_resource_mut::<Counter>().is_err());
    }

    #[test]
    fn child_can_mutate_own_resources() {
        let parent = SystemContext::new().with(Counter { value: 1 });
        let child = parent.child().with(Counter { value: 10 });

        // Child can mutate its own shadowed resource
        {
            let mut counter = child.get_resource_mut::<Counter>().unwrap();
            counter.value += 5;
        }

        assert_eq!(child.get_resource::<Counter>().unwrap().value, 15);
        // Parent unchanged
        assert_eq!(parent.get_resource::<Counter>().unwrap().value, 1);
    }

    #[test]
    fn deep_hierarchy() {
        let root = SystemContext::new().with(Counter { value: 1 });
        let level1 = root.child().with(Config {
            name: "level1".into(),
        });
        let level2 = level1.child();

        // level2 can see both Counter (from root) and Config (from level1)
        assert_eq!(level2.get_resource::<Counter>().unwrap().value, 1);
        assert_eq!(level2.get_resource::<Config>().unwrap().name, "level1");
    }

    // ─────────────────────────────────────────────────────────────────────
    // Output tests
    // ─────────────────────────────────────────────────────────────────────

    #[derive(Debug, PartialEq)]
    struct ReasoningResult {
        action: String,
    }

    #[test]
    fn context_insert_and_get_output() {
        let mut ctx = SystemContext::new();
        ctx.insert_output(ReasoningResult {
            action: "search".into(),
        });

        let output = ctx.get_output::<ReasoningResult>().unwrap();
        assert_eq!(output.action, "search");
    }

    #[test]
    fn out_fetch() {
        let mut ctx = SystemContext::new();
        ctx.insert_output(ReasoningResult {
            action: "calculate".into(),
        });

        let out = Out::<ReasoningResult>::fetch(&ctx).unwrap();
        assert_eq!(out.action, "calculate");
    }

    #[test]
    fn out_not_found_error() {
        let ctx = SystemContext::new();

        let result = Out::<ReasoningResult>::fetch(&ctx);
        assert!(matches!(result, Err(ParamError::OutputNotFound(_))));
    }

    #[test]
    fn optional_out_returns_none() {
        let ctx = SystemContext::new();

        let result = Option::<Out<ReasoningResult>>::fetch(&ctx).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn optional_out_returns_some() {
        let mut ctx = SystemContext::new();
        ctx.insert_output(ReasoningResult {
            action: "test".into(),
        });

        let result = Option::<Out<ReasoningResult>>::fetch(&ctx).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().action, "test");
    }

    #[test]
    fn clear_outputs() {
        let mut ctx = SystemContext::new();
        ctx.insert_output(ReasoningResult {
            action: "test".into(),
        });

        assert!(ctx.contains_output::<ReasoningResult>());

        ctx.clear_outputs();

        assert!(!ctx.contains_output::<ReasoningResult>());
        assert!(Out::<ReasoningResult>::fetch(&ctx).is_err());
    }

    #[test]
    fn outputs_and_resources_are_separate() {
        let mut ctx = SystemContext::new().with(Counter { value: 42 });
        ctx.insert_output(Counter { value: 100 });

        // They should be separate
        {
            let res = Res::<Counter>::fetch(&ctx).unwrap();
            let out = Out::<Counter>::fetch(&ctx).unwrap();

            assert_eq!(res.value, 42);
            assert_eq!(out.value, 100);
        } // Drop borrows before clearing

        // Clearing outputs doesn't affect resources
        ctx.clear_outputs();

        assert!(Res::<Counter>::fetch(&ctx).is_ok());
        assert!(Out::<Counter>::fetch(&ctx).is_err());
    }

    #[test]
    fn multiple_out_reads_allowed() {
        let mut ctx = SystemContext::new();
        ctx.insert_output(ReasoningResult {
            action: "test".into(),
        });

        let out1 = Out::<ReasoningResult>::fetch(&ctx).unwrap();
        let out2 = Out::<ReasoningResult>::fetch(&ctx).unwrap();

        assert_eq!(out1.action, out2.action);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Type-erased insert tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn context_insert_boxed_resource() {
        use core::any::{Any, TypeId};

        let mut ctx = SystemContext::new();

        let type_id = TypeId::of::<Counter>();
        let boxed: Box<dyn Any + Send + Sync> = Box::new(Counter { value: 77 });
        ctx.insert_boxed(type_id, boxed);

        // Should be retrievable via normal get
        let counter = ctx.get_resource::<Counter>().unwrap();
        assert_eq!(counter.value, 77);
    }

    #[test]
    fn context_insert_output_boxed() {
        use core::any::{Any, TypeId};

        let mut ctx = SystemContext::new();

        let type_id = TypeId::of::<ReasoningResult>();
        let boxed: Box<dyn Any + Send + Sync> = Box::new(ReasoningResult {
            action: "boxed_action".into(),
        });
        ctx.insert_output_boxed(type_id, boxed);

        // Should be retrievable via normal get
        let result = ctx.get_output::<ReasoningResult>().unwrap();
        assert_eq!(result.action, "boxed_action");
    }

    #[test]
    fn contains_resource_by_type_id() {
        use core::any::TypeId;

        let ctx = SystemContext::new().with(Counter { value: 1 });

        let counter_id = TypeId::of::<Counter>();
        let config_id = TypeId::of::<Config>();

        assert!(ctx.contains_resource_by_type_id(counter_id));
        assert!(!ctx.contains_resource_by_type_id(config_id));
    }

    #[test]
    fn contains_local_resource_by_type_id() {
        use core::any::TypeId;

        let parent = SystemContext::new().with(Counter { value: 1 });
        let child = parent.child().with(Config {
            name: "child".into(),
        });

        let counter_id = TypeId::of::<Counter>();
        let config_id = TypeId::of::<Config>();

        // Child can see Counter in hierarchy but not locally
        assert!(child.contains_resource_by_type_id(counter_id));
        assert!(!child.contains_local_resource_by_type_id(counter_id));

        // Child has Config locally
        assert!(child.contains_resource_by_type_id(config_id));
        assert!(child.contains_local_resource_by_type_id(config_id));
    }

    #[test]
    fn contains_output_by_type_id() {
        use core::any::TypeId;

        let mut ctx = SystemContext::new();
        ctx.insert_output(ReasoningResult {
            action: "test".into(),
        });

        let reasoning_id = TypeId::of::<ReasoningResult>();
        let counter_id = TypeId::of::<Counter>();

        assert!(ctx.contains_output_by_type_id(reasoning_id));
        assert!(!ctx.contains_output_by_type_id(counter_id));
    }

    // ─────────────────────────────────────────────────────────────────────
    // Deep hierarchy tests
    // ─────────────────────────────────────────────────────────────────────

    #[derive(Debug, PartialEq)]
    struct Level1Resource {
        name: String,
    }
    impl LocalResource for Level1Resource {}

    #[derive(Debug, PartialEq)]
    struct Level2Resource {
        value: i32,
    }
    impl LocalResource for Level2Resource {}

    #[derive(Debug, PartialEq)]
    struct Level3Resource {
        data: Vec<u8>,
    }
    impl LocalResource for Level3Resource {}

    #[test]
    fn three_level_hierarchy() {
        let root = SystemContext::new().with(Counter { value: 0 });
        let level1 = root.child().with(Level1Resource { name: "L1".into() });
        let level2 = level1.child().with(Level2Resource { value: 42 });
        let level3 = level2.child().with(Level3Resource {
            data: vec![1, 2, 3],
        });

        // Level 3 can see all resources up the chain
        assert_eq!(level3.get_resource::<Counter>().unwrap().value, 0);
        assert_eq!(level3.get_resource::<Level1Resource>().unwrap().name, "L1");
        assert_eq!(level3.get_resource::<Level2Resource>().unwrap().value, 42);
        assert_eq!(
            level3.get_resource::<Level3Resource>().unwrap().data,
            vec![1, 2, 3]
        );
    }

    #[test]
    fn four_level_hierarchy_shadowing() {
        let root = SystemContext::new().with(Counter { value: 1 });
        let level1 = root.child().with(Counter { value: 10 });
        let level2 = level1.child().with(Counter { value: 100 });
        let level3 = level2.child().with(Counter { value: 1000 });

        // Each level sees its own shadowed Counter
        assert_eq!(root.get_resource::<Counter>().unwrap().value, 1);
        assert_eq!(level1.get_resource::<Counter>().unwrap().value, 10);
        assert_eq!(level2.get_resource::<Counter>().unwrap().value, 100);
        assert_eq!(level3.get_resource::<Counter>().unwrap().value, 1000);

        // Each can only mutate its own
        {
            let mut counter = level3.get_resource_mut::<Counter>().unwrap();
            counter.value += 1;
        }
        assert_eq!(level3.get_resource::<Counter>().unwrap().value, 1001);
        // Others unchanged
        assert_eq!(level2.get_resource::<Counter>().unwrap().value, 100);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Tuple parameter tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn tuple_param_three_elements() {
        let mut ctx = SystemContext::new()
            .with(Counter { value: 1 })
            .with(Config {
                name: "test".into(),
            });
        ctx.insert_output(ReasoningResult {
            action: "go".into(),
        });

        let (counter, config, out) =
            <(Res<Counter>, Res<Config>, Out<ReasoningResult>)>::fetch(&ctx).unwrap();

        assert_eq!(counter.value, 1);
        assert_eq!(config.name, "test");
        assert_eq!(out.action, "go");
    }

    #[test]
    fn tuple_param_with_mutable() {
        let ctx = SystemContext::new()
            .with(Counter { value: 1 })
            .with(Config {
                name: "test".into(),
            });

        let (counter, mut config) = <(Res<Counter>, ResMut<Config>)>::fetch(&ctx).unwrap();

        assert_eq!(counter.value, 1);
        config.name = "modified".into();
        drop(config);
        drop(counter);

        let config = Res::<Config>::fetch(&ctx).unwrap();
        assert_eq!(config.name, "modified");
    }

    // ─────────────────────────────────────────────────────────────────────
    // SystemParam access declaration tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn res_declares_read_access() {
        let access = <Res<Counter>>::access();
        assert_eq!(access.resources.len(), 1);
        assert_eq!(access.resources[0].mode, AccessMode::Read);
        assert!(access.resources[0].type_name.contains("Counter"));
    }

    #[test]
    fn res_mut_declares_write_access() {
        let access = <ResMut<Counter>>::access();
        assert_eq!(access.resources.len(), 1);
        assert_eq!(access.resources[0].mode, AccessMode::Write);
        assert!(access.resources[0].type_name.contains("Counter"));
    }

    #[test]
    fn out_declares_output_access() {
        let access = <Out<ReasoningResult>>::access();
        assert_eq!(access.outputs.len(), 1);
        assert_eq!(access.outputs[0].mode, AccessMode::Read);
        assert!(access.outputs[0].type_name.contains("ReasoningResult"));
    }

    #[test]
    fn tuple_access_merges_all() {
        let access = <(Res<Counter>, ResMut<Config>, Out<ReasoningResult>)>::access();

        assert_eq!(access.resources.len(), 2);
        assert_eq!(access.outputs.len(), 1);
    }

    #[test]
    fn unit_declares_empty_access() {
        let access = <()>::access();
        assert!(access.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────
    // take_outputs + merge pattern tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn take_outputs_and_merge_into_parent() {
        let mut parent = SystemContext::new();
        parent.insert_output(ReasoningResult {
            action: "parent".into(),
        });

        // Simulate parallel branch: create child, produce output, extract, drop, merge
        let child_outputs = {
            let mut child = parent.child();
            child.insert_output(ReasoningResult {
                action: "child".into(),
            });
            child.take_outputs()
        };
        // child is dropped here, releasing borrow on parent

        parent.outputs_mut().merge_from(child_outputs);

        let output = parent.get_output::<ReasoningResult>().unwrap();
        assert_eq!(output.action, "child");
    }
}
