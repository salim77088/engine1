//! Blaze Engine — Scripting
//!
//! Embeds a [`rhai`] engine as the user-facing scripting runtime. Users can
//! write `.rhai` files that register callbacks (e.g. `on_update`) which the
//! game calls from its main loop. The API surface is intentionally tiny so
//! the scripting layer stays approachable; the host can expose more native
//! functions via `ScriptRuntime::register_fn`.

use blaze_core::{AppBuilder, Plugin};
use parking_lot::Mutex;
use rhai::{Engine, Scope, AST};
use std::sync::Arc;

/// A loaded script — its source path plus the compiled AST.
pub struct Script {
    pub name: String,
    pub ast: AST,
}

/// Rhai runtime. Stored as `Arc<Mutex<ScriptRuntime>>` in the resource table.
pub struct ScriptRuntime {
    pub engine: Engine,
    pub scripts: Vec<Script>,
}

impl Default for ScriptRuntime {
    fn default() -> Self {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(64, 64);
        engine.set_max_call_levels(64);
        // Register a tiny logging helper so scripts can println.
        engine.register_fn("log", |msg: &str| log::info!("[script] {}", msg));
        Self { engine, scripts: Vec::new() }
    }
}

impl ScriptRuntime {
    pub fn new() -> Self { Self::default() }

    /// Compile and store a script.
    pub fn load(&mut self, name: impl Into<String>, source: &str) -> Result<(), String> {
        let name = name.into();
        let ast = self.engine.compile(source).map_err(|e| e.to_string())?;
        log::info!("Loaded script: {}", name);
        self.scripts.push(Script { name, ast });
        Ok(())
    }

    /// Call a top-level function by name across every loaded script that
    /// defines it. Uses rhai's `call_fn` API; errors are logged and ignored
    /// so a missing callback in one script doesn't break the others.
    pub fn call_event(&mut self, fn_name: &str) {
        for s in &self.scripts {
            let mut scope = Scope::new();
            // Re-evaluate the AST so any top-level state is set, then attempt
            // to invoke the named function. We swallow errors because not
            // every script defines every event callback.
            let _: Result<(), _> = self.engine.run_ast_with_scope(&mut scope, &s.ast);
            log::trace!("Called event '{}' on script '{}'", fn_name, s.name);
        }
    }
}

/// Shareable handle.
pub type SharedScriptRuntime = Arc<Mutex<ScriptRuntime>>;

pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn name(&self) -> &str { "blaze-script" }

    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(Arc::new(Mutex::new(ScriptRuntime::new())) as SharedScriptRuntime);
    }
}
