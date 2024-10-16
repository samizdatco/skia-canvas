#[macro_use]
extern crate log;

pub use rafx;
pub use skia_safe;
pub use skia_bindings;

pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

mod skia_support;
pub use skia_support::VkSkiaContext;
pub use skia_support::VkSkiaSurface;

mod renderer;
pub use renderer::RendererBuilder;
pub use renderer::Renderer;
pub use renderer::ValidationMode;

mod coordinates;
pub use coordinates::Size;
pub use coordinates::LogicalSize;
pub use coordinates::PhysicalSize;
pub use coordinates::CoordinateSystem;
pub use coordinates::CoordinateSystemHelper;
