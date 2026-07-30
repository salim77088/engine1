//! Application lifecycle: builder + runner.

use crate::event::EventBus;
use crate::plugin::Plugin;
use crate::time::Time;
use anyhow::Result;
use parking_lot::Mutex;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// A runner owns the built [`App`] and executes the main loop.
pub trait Runner: Send + 'static {
    fn run(self: Box<Self>, app: App) -> Result<()>;
}

/// Type-erased resource storage. Resources are stored as `Arc<dyn Any + Send +
/// Sync>` so any `T: Any + Send + Sync` can be inserted; mutable state should
/// be wrapped in an `Arc<Mutex<T>>` or `Arc<RwLock<T>>` before insertion.
#[derive(Default)]
pub struct Resources {
    inner: parking_lot::RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl Resources {
    pub fn insert<R: Any + Send + Sync + 'static>(&self, r: R) {
        self.inner.write().insert(TypeId::of::<R>(), Arc::new(r));
    }

    /// Returns whether a resource of type `R` exists.
    pub fn contains<R: Any + Send + Sync + 'static>(&self) -> bool {
        self.inner.read().contains_key(&{ TypeId::of::<R>() })
    }

    /// Run a closure with shared access to the resource, if present.
    pub fn with<R, F, Out>(&self, f: F) -> Option<Out>
    where
        R: Any + Send + Sync + 'static,
        F: FnOnce(&R) -> Out,
    {
        let guard = self.inner.read();
        let tid = TypeId::of::<R>();
        let arc = guard.get(&tid)?.clone();
        drop(guard);
        let r = arc.downcast_ref::<R>()?;
        Some(f(r))
    }

    /// Run a closure with exclusive access to the resource, if present.
    /// Only works when `R` is `Send + Sync` interior-mutable (e.g. an
    /// `Arc<Mutex<T>>` or `Arc<RwLock<T>>`).
    pub fn with_clone<R, F, Out>(&self, f: F) -> Option<Out>
    where
        R: Any + Send + Sync + Clone + 'static,
        F: FnOnce(R) -> Out,
    {
        // For Clone types, downcast, clone, then call f.
        let guard = self.inner.read();
        let tid = TypeId::of::<R>();
        let arc = guard.get(&tid)?.clone();
        drop(guard);
        let r = arc.downcast_ref::<R>()?;
        Some(f(r.clone()))
    }
}

/// Builder returned by [`App::new`]. Plugins are added with [`Self::add_plugin`]
/// and resources with [`Self::insert_resource`].
pub struct AppBuilder {
    pub time: Time,
    pub event_bus: Arc<EventBus>,
    pub resources: Resources,
    plugins: Vec<Box<dyn Plugin>>,
    runner: Option<Box<dyn Runner>>,
    should_exit: Arc<Mutex<bool>>,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self {
            time: Time::new(),
            event_bus: Arc::new(EventBus::new()),
            resources: Resources::default(),
            plugins: Vec::new(),
            runner: None,
            should_exit: Arc::new(Mutex::new(false)),
        }
    }

    pub fn add_plugin<P: Plugin + 'static>(&mut self, plugin: P) -> &mut Self {
        log::info!("Adding plugin: {}", plugin.name());
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn insert_resource<R: Any + Send + Sync + 'static>(&mut self, r: R) -> &mut Self {
        self.resources.insert(r);
        self
    }

    pub fn set_runner<R: Runner>(&mut self, runner: R) -> &mut Self {
        self.runner = Some(Box::new(runner));
        self
    }

    pub fn event_bus(&self) -> Arc<EventBus> { self.event_bus.clone() }

    pub fn should_exit(&self) -> bool { *self.should_exit.lock() }

    pub fn request_exit(&self) { *self.should_exit.lock() = true; }

    pub fn build(mut self) -> Result<App> {
        let plugins = std::mem::take(&mut self.plugins);
        for p in &plugins {
            if p.is_enabled() {
                p.build(&mut self);
            }
        }
        for p in &plugins {
            if p.is_enabled() {
                p.finish(&mut self);
            }
        }

        let runner = self.runner.take();
        Ok(App {
            time: self.time,
            event_bus: self.event_bus.clone(),
            resources: self.resources,
            runner,
            should_exit: self.should_exit.clone(),
        })
    }
}

/// A built, ready-to-run engine instance.
pub struct App {
    pub time: Time,
    pub event_bus: Arc<EventBus>,
    pub resources: Resources,
    runner: Option<Box<dyn Runner>>,
    should_exit: Arc<Mutex<bool>>,
}

impl App {
    pub fn builder() -> AppBuilder { AppBuilder::new() }

    pub fn run(mut self) -> Result<()> {
        if let Some(runner) = self.runner.take() {
            runner.run(self)
        } else {
            log::warn!("No runner was set; the app exits immediately.");
            Ok(())
        }
    }

    pub fn should_exit(&self) -> bool { *self.should_exit.lock() }
    pub fn request_exit(&self) { *self.should_exit.lock() = true; }
}
