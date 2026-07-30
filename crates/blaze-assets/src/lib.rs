//! Blaze Engine — Assets
//!
//! The asset system loads files from disk (textures, meshes, scenes)
//! and hands out opaque handles. The actual GPU upload of textures
//! happens in `blaze-render` (which has the wgpu device); this crate
//! only owns the CPU-side decoded data and the file-watching.

use anyhow::{Context, Result};
use blaze_core::{AppBuilder, Plugin};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Decoded image data (CPU-side). The renderer uploads it to a wgpu
/// texture lazily on first use.
pub struct TextureAsset {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA8
}

impl TextureAsset {
    pub fn from_path(path: &Path) -> Result<Self> {
        let img = image::open(path).with_context(|| format!("loading texture {}", path.display()))?;
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        Ok(Self {
            width: w,
            height: h,
            pixels: rgba.into_raw(),
        })
    }

    /// Generate a 1x1 white texture (useful as a placeholder).
    pub fn white_1x1() -> Self {
        Self {
            width: 1,
            height: 1,
            pixels: vec![255, 255, 255, 255],
        }
    }

    /// Generate a checkerboard texture (useful for debugging / default).
    pub fn checkerboard(size: u32, cells: u32) -> Self {
        let cell = size / cells;
        let mut pixels = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                let on = ((x / cell) + (y / cell)) % 2 == 0;
                let c = if on { 200 } else { 80 };
                pixels.extend_from_slice(&[c, c, c, 255]);
            }
        }
        Self { width: size, height: size, pixels }
    }
}

/// A handle to a loaded asset. Cheap to clone (it's an Arc).
pub type TextureHandle = Arc<TextureAsset>;

/// Central asset registry. Stored as `Arc<RwLock<AssetRegistry>>` in
/// the engine's resource table.
#[derive(Default)]
pub struct AssetRegistry {
    textures: HashMap<String, TextureHandle>,
    pub asset_root: PathBuf,
}

impl AssetRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { asset_root: root.into(), textures: HashMap::new() }
    }

    /// Get a texture if already loaded.
    pub fn get_texture(&self, key: &str) -> Option<TextureHandle> {
        self.textures.get(key).cloned()
    }

    /// Load a texture from `<asset_root>/<key>`. Cached by key.
    pub fn load_texture(&mut self, key: impl Into<String>) -> Result<TextureHandle> {
        let key = key.into();
        if let Some(h) = self.textures.get(&key) {
            return Ok(h.clone());
        }
        let path = self.asset_root.join(&key);
        let tex = TextureAsset::from_path(&path)?;
        let h = Arc::new(tex);
        self.textures.insert(key.clone(), h.clone());
        log::info!("Loaded texture: {}", key);
        Ok(h)
    }

    /// Insert a runtime-constructed texture (e.g. the editor's checkerboard).
    pub fn insert_texture(&mut self, key: impl Into<String>, tex: TextureAsset) -> TextureHandle {
        let key = key.into();
        let h = Arc::new(tex);
        self.textures.insert(key, h.clone());
        h
    }

    /// Enumerate every loaded texture key (for the asset browser).
    pub fn texture_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.textures.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Enumerate files under `asset_root` for the asset browser. Returns
    /// paths relative to the root.
    pub fn list_files(&self) -> Vec<String> {
        if !self.asset_root.exists() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let _ = walk_dir(&self.asset_root, &self.asset_root, &mut out);
        out.sort();
        out
    }
}

fn walk_dir(root: &Path, dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Shareable handle.
pub type SharedAssetRegistry = Arc<RwLock<AssetRegistry>>;

pub struct AssetPlugin {
    pub asset_root: PathBuf,
}

impl Default for AssetPlugin {
    fn default() -> Self {
        Self { asset_root: PathBuf::from("assets") }
    }
}

impl Plugin for AssetPlugin {
    fn name(&self) -> &str { "blaze-assets" }

    fn build(&self, app: &mut AppBuilder) {
        let registry = AssetRegistry::with_root(&self.asset_root);
        app.insert_resource(Arc::new(RwLock::new(registry)) as SharedAssetRegistry);
    }
}
