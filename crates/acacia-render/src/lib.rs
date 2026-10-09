//! wgpu renderer for [`acacia_world`] terrain: vanilla resource pack loading, per-block render data,
//! section meshing on worker threads, and drawing. Design: see README.md.

pub mod assets;
pub mod biome;
pub mod block_models;
pub mod blocks;
pub mod camera;
pub mod clouds;
mod cull;
pub mod entity;
pub mod fluid_view;
pub mod glint;
mod gpu;
pub mod item;
pub mod light;
pub mod lightning;
pub mod look;
pub mod lookpack;
pub mod mesh;
pub mod particles;
pub mod shadows;
mod scene;
pub mod sign_text;
pub mod sky;
pub mod weather;
mod workers;

pub use camera::Camera;
pub use gpu::{FrameStats, Outline, Renderer, load_crack_stages};
pub use look::Look;
pub use lookpack::LookPack;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path}: {source}")]
    Json { path: String, source: serde_json::Error },
    #[error("{path}: {source}")]
    Image { path: String, source: image::ImageError },
    #[error("{path}: {reason}")]
    LookPack { path: String, reason: String },
    #[error("no compatible GPU adapter: {0}")]
    Adapter(String),
    #[error("GPU device: {0}")]
    Device(String),
    #[error("surface: {0}")]
    Surface(String),
}
