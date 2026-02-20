use examples::{PersistencePlugin, ReActPlugin};
use polaris::plugins::{PersistenceAPI, ResourceSerializer};
use polaris::{
    graph::hooks::{
        HooksAPI,
        api::BoxedHook,
        events::GraphEvent,
        schedule::{OnGraphComplete, OnGraphFailure},
    },
    system::{
        param::SystemContext,
        plugin::{Plugin, PluginId, ScheduleId, Version},
        resource::GlobalResource,
        server::Server,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{path::PathBuf, sync::Arc};

/// State store.
///
/// The outer `HashMap` key is the session ID.
/// The inner `HashMap` key is the composite resource key in format `"<plugin_id>/<storage_key>"`.
#[derive(Serialize, Deserialize, Default)]
struct Store {
    sessions: HashMap<String, HashMap<String, Entry>>,
}

/// A single persisted resource entry.
#[derive(Serialize, Deserialize)]
struct Entry {
    version: String,
    data: serde_json::Value,
}

/// Session configuration (global resource).
#[derive(Debug, Clone)]
struct SessionConfig {
    session_id: String,
    data_dir: PathBuf,
}

impl GlobalResource for SessionConfig {}

/// Plugin that saves and loads storable resources to a JSON file.
///
/// Requires [`PersistencePlugin`] before it, and any plugins that register
/// storable resources (e.g. `ReActPlugin`) between them.
pub struct SessionPlugin {
    config: SessionConfig,
}

impl SessionPlugin {
    pub fn new(session_id: impl Into<String>, data_dir: impl Into<PathBuf>) -> Self {
        Self {
            config: SessionConfig {
                session_id: session_id.into(),
                data_dir: data_dir.into(),
            },
        }
    }

    /// Loads persisted session data into the context.
    ///
    /// Call after `server.finish()` and `create_context()` to restore
    /// a prior session before execution.
    pub fn load(
        session_id: &str,
        data_dir: impl Into<PathBuf>,
        server: &Server,
        ctx: &mut SystemContext<'_>,
    ) {
        let Some(api) = server.api::<PersistenceAPI>() else {
            return;
        };
        let config = SessionConfig {
            session_id: session_id.to_string(),
            data_dir: data_dir.into(),
        };
        Self::load_resources(&config, &api.serializers(), ctx);
    }

    fn store_path(config: &SessionConfig) -> PathBuf {
        config.data_dir.join("persistence.json")
    }

    fn resource_key(v: &dyn ResourceSerializer) -> String {
        format!("{}/{}", v.plugin_id(), v.storage_key())
    }

    fn read_store(config: &SessionConfig) -> Store {
        std::fs::read_to_string(Self::store_path(config))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write_store(config: &SessionConfig, store: &Store) {
        let path = Self::store_path(config);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(store) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn load_resources(
        config: &SessionConfig,
        serializers: &[Arc<dyn ResourceSerializer>],
        ctx: &mut SystemContext<'_>,
    ) {
        let store = Self::read_store(config);
        let Some(session) = store.sessions.get(&config.session_id) else {
            return;
        };

        for serializer in serializers {
            let key = Self::resource_key(&**serializer);
            if let Some(entry) = session.get(&key) {
                match serializer.load(entry.data.clone(), ctx) {
                    Ok(()) => println!("[Session] Loaded {key}"),
                    Err(err) => eprintln!("[Session] Failed to load {key}: {err}"),
                }
            }
        }
    }

    fn save_resources(
        config: &SessionConfig,
        serializers: &[Arc<dyn ResourceSerializer>],
        ctx: &SystemContext<'_>,
    ) {
        let mut store = Self::read_store(config);
        let session = store.sessions.entry(config.session_id.clone()).or_default();

        for serializer in serializers {
            match serializer.save(ctx) {
                Ok(Some(data)) => {
                    session.insert(
                        Self::resource_key(&**serializer),
                        Entry {
                            version: serializer.schema_version().to_string(),
                            data,
                        },
                    );
                }
                Err(err) => eprintln!(
                    "[Session] Failed to save {}: {err}",
                    serializer.storage_key()
                ),
                _ => {}
            }
        }

        Self::write_store(config, &store);
        println!("[Session] Saved to {}", Self::store_path(config).display());
    }

    /// Registers a save hook on the given schedule.
    fn register_save_hook(
        hooks: &HooksAPI,
        schedule: ScheduleId,
        name: &'static str,
        config: SessionConfig,
        serializers: Arc<Vec<Arc<dyn ResourceSerializer>>>,
    ) {
        hooks
            .register_boxed(
                schedule,
                name,
                BoxedHook::new(
                    move |ctx: &mut SystemContext<'_>, _: &GraphEvent| {
                        Self::save_resources(&config, &serializers, ctx);
                    },
                    Vec::new(),
                ),
            )
            .expect("hook registration failed");
    }
}

impl Plugin for SessionPlugin {
    const ID: &'static str = "examples::sessions";
    const VERSION: Version = Version::new(0, 0, 1);

    fn build(&self, server: &mut Server) {
        server.insert_global(self.config.clone());

        if !server.contains_api::<HooksAPI>() {
            server.insert_api(HooksAPI::new());
        }
    }

    fn ready(&self, server: &mut Server) {
        let api = server
            .api::<PersistenceAPI>()
            .expect("PersistencePlugin must be added before SessionPlugin");
        let serializers = Arc::new(api.serializers());

        if serializers.is_empty() {
            return;
        }

        let hooks = server
            .api::<HooksAPI>()
            .expect("HooksAPI should be present");

        // Save on completion and failure
        Self::register_save_hook(
            hooks,
            ScheduleId::of::<OnGraphComplete>(),
            "session:save",
            self.config.clone(),
            Arc::clone(&serializers),
        );
        Self::register_save_hook(
            hooks,
            ScheduleId::of::<OnGraphFailure>(),
            "session:save_on_failure",
            self.config.clone(),
            serializers,
        );
    }

    fn dependencies(&self) -> Vec<PluginId> {
        vec![
            PluginId::of::<ReActPlugin>(),
            PluginId::of::<PersistencePlugin>(),
        ]
    }
}
