# Blaze Engine

A lightweight, modern 2D/3D game engine built in Rust on top of 2026-era
libraries. Designed to be small enough to read in a weekend, fast enough
for real games, and free of licensing surprises.

> **Status:** alpha — API still evolving. The renderer, ECS, physics, input,
> scripting and editor scaffolding all work today; mesh/material pipelines,
> asset loading, and the full scene editor are growing on `main`.

---

## Why another engine?

Most open-source engines either come with a heavy IDE and a custom language
you have to learn (Godot, Unreal), or they're tiny frameworks that leave
too much to the user (Fyrox, Macroquad). **Blaze** sits in the middle:

| Goal | How |
|------|-----|
| Memory-safe, zero-overhead core | Rust + `wgpu` + `hecs` + `rapier` |
| Cross-platform from a single codebase | `winit` for windowing, `wgpu` for graphics |
| Approachable scripting | `rhai` for hot-reloadable game logic |
| Built-in editor | `egui` panels wired into the default runner |
| Tiny dependency footprint | No vendored binaries, no C++ toolchain required |
| Free & permissive | MIT **or** Apache-2.0 (your choice) |

## Architecture

```
blaze-engine/
├── crates/
│   ├── blaze-core      # App builder, plugin system, time, event bus
│   ├── blaze-math      # glam re-export + Transform / Transform2D / Color
│   ├── blaze-ecs       # hecs wrapper + stage-based scheduler
│   ├── blaze-input     # keyboard / mouse / gamepad state
│   ├── blaze-render    # wgpu device + triangle pipeline (3D API)
│   ├── blaze-physics   # rapier2d wrapper (rapier3d optional)
│   ├── blaze-ui        # egui editor panels
│   ├── blaze-script    # rhai runtime for hot-reloadable scripts
│   └── blaze-app       # default winit runner that ties it all together
├── editor/             # blaze-editor binary (the IDE)
└── examples/
    ├── hello-triangle/ # smallest possible Blaze app
    └── hello-2d/       # ECS + physics demo
```

## Quick start

```bash
# Build & run the editor
cargo run --release --bin blaze-editor

# Run an example
cargo run --release --bin hello-triangle
```

Press **Escape** to close the window.

## Adding a system

```rust
use blaze_core::{App, Stage};
use blaze_ecs::{EcsPlugin, World, AppBuilderEcsExt};
use blaze_app::run;

fn main() {
    let mut b = App::builder();
    b.add_plugin(EcsPlugin);
    b.add_system(Stage::Update, |world: &mut World| {
        for (_id, _pos) in world.query::<&blaze_math::Vec3>().iter() {
            // do something every frame
        }
    });
    run(b).unwrap();
}
```

## Scripting with rhai

```rhai
// scripts/hello.rhai
log("Hello from rhai!");

fn on_update() {
    log("tick");
}
```

```rust
use blaze_script::ScriptRuntime;
// at startup:
script_runtime.lock().load("hello", include_str!("../scripts/hello.rhai")).unwrap();
// each frame:
script_runtime.lock().call_event("on_update");
```

## CI / Releases

Every push runs `cargo fmt`/`clippy`/`test` on Ubuntu. Every push to `main`
**and** every tag `v*` triggers release builds on **Windows (x86_64)**,
**Linux (x86_64)** and **macOS (aarch64 + x86_64)**, producing ready-to-run
editor binaries that are attached to a GitHub Release.

See [`.github/workflows/ci.yml`](.github/workflows/ci.yml) for the full pipeline.

## License

Dual-licensed under **MIT** or **Apache-2.0**, at your option. Contributions
intentionally retain this dual license so the engine can be embedded in any
project, commercial or otherwise.
