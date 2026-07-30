//! Blaze Engine — Core
//!
//! Provides the application lifecycle, plugin system, timekeeping and an
//! event bus that ties every other Blaze subsystem together.

pub mod app;
pub mod event;
pub mod plugin;
pub mod time;

pub use app::{App, AppBuilder, Resources, Runner};
pub use event::{Event, EventBus, SubscriberId};
pub use plugin::Plugin;
pub use time::Time;
