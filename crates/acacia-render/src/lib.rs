//! wgpu renderer for [`acacia_world`] terrain: vanilla resource pack loading, per-block render data,
//! section meshing on worker threads, and drawing. Design: see README.md.

pub mod assets;
pub mod biome;
pub mod blocks;
pub mod camera;
mod gpu;
pub mod light;
pub mod mesh;
mod scene;
mod workers;

pub use camera::Camera;
pub use gpu::{FrameStats, Renderer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path}: {source}")]
    Json { path: String, source: serde_json::Error },
    #[error("{path}: {source}")]
    Image { path: String, source: image::ImageError },
    #[error("no compatible GPU adapter: {0}")]
    Adapter(String),
    #[error("GPU device: {0}")]
    Device(String),
    #[error("surface: {0}")]
    Surface(String),
}
