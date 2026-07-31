// reject `window` without a backend also being selected
#[cfg(all(feature = "window", not(any(feature = "metal", feature = "vulkan"))))]
compile_error!("the `window` feature requires enabling `metal` or `vulkan`");

#[cfg(feature = "metal")]
mod metal;
#[cfg(feature = "vulkan")]
mod vulkan;
pub mod page;

// caches for the render thread's gpu-backed page resources: live recording surfaces
// (see gfx::page::RecordingSurface) and persistent snapshot bitmaps
pub(crate) mod cache;

// rendering engine, CPU/GPU dispatch, and the gpu render thread
mod engine;

// backend-selected windowed renderer (only when a gpu backend + window are enabled)
#[cfg(all(feature = "metal", feature = "window"))]
pub use crate::gfx::metal::window::MetalRenderer as Renderer;
#[cfg(all(feature = "vulkan", feature = "window"))]
pub use crate::gfx::vulkan::window::VulkanRenderer as Renderer;

// public rendering API — paths preserved for consumers (crate::gfx::RenderingEngine, etc.)
pub use engine::{RenderingEngine, render_soon};
#[cfg(feature = "window")]
pub use engine::RenderOutcome;
