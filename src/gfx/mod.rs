// reject `window` without a backend also being selected
#[cfg(all(feature = "window", not(any(feature = "metal", feature = "vulkan"))))]
compile_error!("the `window` feature requires enabling `metal` or `vulkan`");

#[cfg(feature = "metal")]
mod metal;
#[cfg(feature = "vulkan")]
mod vulkan;
pub mod page;

// budgeted storage for readback surfaces, export snapshots, and embedded-canvas rasters
pub(crate) mod cache;

// a window's last on-screen render, kept as a backdrop for the next one (owned not cached)
#[cfg(feature = "window")]
pub(crate) mod framebuffer;

// rendering engine, CPU/GPU dispatch, and the gpu render thread
pub(crate) mod engine;

// backend-selected windowed renderer (only when a gpu backend + window are enabled)
#[cfg(all(feature = "metal", feature = "window"))]
pub use crate::gfx::metal::window::MetalRenderer as Renderer;
#[cfg(all(feature = "vulkan", feature = "window"))]
pub use crate::gfx::vulkan::window::VulkanRenderer as Renderer;

// public rendering API — paths preserved for consumers (crate::gfx::RenderingEngine, etc.)
pub use engine::{RenderingEngine, render_soon};
#[cfg(feature = "window")]
pub use engine::RenderOutcome;
