//! Blaze Engine — Scene serialization
//!
//! Save/load the ECS world to a RON file. Each entity is serialized as a
//! list of (type-name, ron-string) pairs. We hand-roll the component
//! registry because hecs doesn't have built-in reflection.

use anyhow::{Context, Result};
use blaze_components::{
    Camera, DirectionalLight, Material, Mesh, Name, PointLight, Sprite, Tag,
};
use blaze_ecs::World;
use blaze_math::{Transform, Transform2D};
use serde::{Deserialize, Serialize};

/// One component on an entity. `ty` discriminates the payload; `data` is
/// a RON-encoded string of the component value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentEntry {
    pub ty: String,
    pub data: String,
}

/// A serialized entity — a list of components keyed by type name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntity {
    pub id: u64,
    pub components: Vec<ComponentEntry>,
}

/// A full scene — version + list of entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub version: u32,
    pub entities: Vec<SceneEntity>,
}

impl Scene {
    pub fn empty() -> Self {
        Self { version: 1, entities: Vec::new() }
    }

    /// Snapshot the world into a Scene.
    pub fn from_world(world: &World) -> Self {
        let mut entities = Vec::new();
        let ids: Vec<blaze_ecs::Entity> = world.iter().map(|r| r.entity()).collect();
        for entity in ids {
            let mut entries = Vec::new();
            // For each component type we know about, try to serialize it.
            macro_rules! try_comp {
                ($ty:ty, $name:expr) => {
                    if let Ok(entity_ref) = world.entity(entity) {
                        if let Some(g) = entity_ref.get::<&$ty>() {
                            if let Ok(s) = ron::to_string(&*g) {
                                entries.push(ComponentEntry { ty: $name.into(), data: s });
                            }
                        }
                    }
                };
            }
            try_comp!(Name, "Name");
            try_comp!(Tag, "Tag");
            try_comp!(Transform, "Transform");
            try_comp!(Transform2D, "Transform2D");
            try_comp!(Mesh, "Mesh");
            try_comp!(Material, "Material");
            try_comp!(Sprite, "Sprite");
            try_comp!(Camera, "Camera");
            try_comp!(DirectionalLight, "DirectionalLight");
            try_comp!(PointLight, "PointLight");

            entities.push(SceneEntity {
                id: entity.id() as u64,
                components: entries,
            });
        }
        Self { version: 1, entities }
    }

    /// Spawn every entity in the scene into the (assumed-empty) world.
    pub fn spawn_into(&self, world: &mut World) -> Result<()> {
        use hecs::EntityBuilder;
        for ent in &self.entities {
            let mut builder = EntityBuilder::new();
            for comp in &ent.components {
                match comp.ty.as_str() {
                    "Name" => {
                        let v: Name = ron::from_str(&comp.data)
                            .context("deserializing Name")?;
                        builder.add(v);
                    }
                    "Tag" => {
                        let v: Tag = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "Transform" => {
                        let v: Transform = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "Transform2D" => {
                        let v: Transform2D = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "Mesh" => {
                        let v: Mesh = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "Material" => {
                        let v: Material = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "Sprite" => {
                        let v: Sprite = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "Camera" => {
                        let v: Camera = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "DirectionalLight" => {
                        let v: DirectionalLight = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    "PointLight" => {
                        let v: PointLight = ron::from_str(&comp.data)?;
                        builder.add(v);
                    }
                    other => log::warn!("Unknown component type in scene: {other}"),
                }
            }
            world.spawn(builder.build());
        }
        Ok(())
    }

    /// Serialize to a RON string.
    pub fn to_ron(&self) -> Result<String> {
        let pretty = ron::ser::PrettyConfig::default();
        Ok(ron::ser::to_string_pretty(self, pretty)?)
    }

    /// Deserialize from a RON string.
    pub fn from_ron(s: &str) -> Result<Self> {
        Ok(ron::from_str(s)?)
    }

    /// Save to a file path.
    pub fn save_to_path(&self, path: &std::path::Path) -> Result<()> {
        let s = self.to_ron()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, s).with_context(|| format!("writing scene {}", path.display()))?;
        Ok(())
    }

    /// Load from a file path.
    pub fn load_from_path(path: &std::path::Path) -> Result<Self> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("reading scene {}", path.display()))?;
        Self::from_ron(&s)
    }
}
