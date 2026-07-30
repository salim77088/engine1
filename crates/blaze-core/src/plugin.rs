//! Plugin trait — every Blaze subsystem (renderer, physics, ui, script, …)
//! implements this so the user can compose engines from small pieces.

use crate::app::AppBuilder;

/// A plugin is a self-contained unit of engine functionality.
///
/// Implementors typically register resources, systems and event handlers
/// inside `build`.
pub trait Plugin: Send + Sync {
    /// Human-readable name used in logs.
    fn name(&self) -> &str { std::any::type_name::<Self>() }

    /// Configure the app. Called once at startup.
    fn build(&self, app: &mut AppBuilder);

    /// Optional post-build hook after every plugin has been built.
    fn finish(&self, _app: &mut AppBuilder) {}

    /// Whether this plugin should run for the current target.
    fn is_enabled(&self) -> bool { true }
}
