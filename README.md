# Blaze Engine

A lightweight, modern 2D/3D game engine built in Rust on top of 2026-era
libraries — designed to feel like a real engine (Unity/Godot-style editor
with a live viewport, hierarchy, inspector, console, asset browser) while
staying small enough to read in a weekend.

> **v0.3.0** — Real engine architecture with PBR mesh pipeline, sprite
> pipeline, ECS-integrated physics, scene save/load, and a full editor UI
> showing the live game viewport inside egui.

---

## What's in the box

### Architecture (modular Cargo workspace)

| Crate | Wraps | Purpose |
|-------|-------|---------|
| `blaze-core` | (native) | App builder, plugin system, time, type-erased event bus, resources |
| `blaze-math` | `glam` | Vec/Mat/Quat + `Transform` / `Transform2D` / `Color` (serde-enabled) |
| `blaze-ecs` | `hecs` | Stage-based scheduler (PreUpdate / Update / FixedUpdate / PostUpdate) |
| `blaze-components` | (native) | `Name`, `Tag`, `Mesh`, `Material`, `Sprite`, `Camera`, `DirectionalLight`, `PointLight` |
| `blaze-input` | (native) | Keyboard / mouse state with just-pressed / just-released edges |
| `blaze-render` | `wgpu 22` | Camera, PBR mesh pipeline, 2D sprite pipeline, offscreen render target, egui overlay |
| `blaze-physics` | `rapier3d 0.22` | `RigidBody` + `Collider` ECS components, `PhysicsSync` system, CCD, joints |
| `blaze-assets` | `image 0.25` | Asset registry, texture loading from disk, file enumeration |
| `blaze-scene` | `ron 0.8` | Serialize/deserialize the ECS world to RON, scene save/load |
| `blaze-ui` | `egui 0.29` | Full editor: menu bar, hierarchy, viewport, inspector, console, asset browser |
| `blaze-script` | `rhai 1.x (sync)` | Hot-reloadable scripting runtime |
| `blaze-app` | `winit 0.30` | Default runner wiring everything together |

### Editor features

- **Top menu bar**: File (New/Open/Save Scene, Exit), Edit, View (toggle panels), Help
- **Hierarchy panel**: live entity list with add/delete, component icons
- **Viewport**: displays the live game scene rendered to an offscreen texture, with drag-to-orbit and scroll-to-zoom camera controls
- **Inspector**: per-entity Transform editing (position, scale, rotation drag-values), add/remove Mesh/Material/Sprite/Camera/Light components
- **Console**: log feed (warnings, info, sentinel commands)
- **Asset Browser**: floating window listing files under `assets/`
- **About modal**

### Rendering pipeline

- **Camera component**: perspective or orthographic, with configurable FOV, near/far, clear color
- **Mesh pipeline**: PBR-ish lighting with 1 directional light + up to 8 point lights, Blinn-Phong specular, supports Cube/Quad/Sphere/Plane primitives
- **Sprite pipeline**: 2D colored quads with alpha blending
- **Render-to-texture**: the game scene is rendered to an offscreen `RenderTarget` (color + depth), then displayed inside the egui central panel via `egui_wgpu::Renderer::register_native_texture`

### Physics

- `RigidBody` component: dynamic, fixed, kinematic-position, kinematic-velocity
- `Collider` component: box, sphere, capsule
- `PhysicsLink` internal component bridges ECS entity <-> rapier handle
- `physics_sync_system` runs at `FixedUpdate`: creates bodies, syncs Transform <-> rapier, steps simulation

### Scene serialization

- `Scene::from_world()` snapshots the entire ECS world
- `Scene::spawn_into()` rebuilds the world from a saved scene
- RON format, human-readable and editable
- Save/Load menu items in the editor

## Quick start

```bash
# Build & run the editor (opens a window with a starter 3D scene)
cargo run --release --bin blaze-editor

# Run an example
cargo run --release --bin hello-cube
cargo run --release --bin hello-sprite
```

The editor starts with a starter scene containing:
- An editor camera (orbit around the origin with drag + scroll)
- A directional light (the "sun")
- A floor plane
- A demo cube (red, PBR material)
- A demo sphere (blue, metallic)
- A point light (warm color)

Press **Escape** to close the window.

## Adding your own entities

Use the **+Add Entity** button in the Hierarchy, then in the Inspector:
1. Click **+ Transform** (added automatically)
2. Click **+ Mesh** — adds a Cube + default Material
3. Click **+ Camera** / **+ Directional Light** / **+ Point Light** as needed
4. Edit Position / Scale / Rotation via the drag-values

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

Dual-licensed under **MIT** or **Apache-2.0**, at your option.
