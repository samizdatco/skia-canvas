use metal::{ foreign_types::ForeignTypeRef, CommandQueue, DeviceRef };
use skia_safe::gpu::{ mtl, direct_contexts, DirectContext };

pub mod offscreen;

#[cfg(feature = "window")]
pub mod window;

// create a Skia rendering context for use by either on- or offscreen renderers
fn make_direct_context(device:&DeviceRef) -> Option<(CommandQueue, DirectContext)>{
    let queue = device.new_command_queue();
    let backend = unsafe {
        mtl::BackendContext::new(
            device.as_ptr() as mtl::Handle,
            queue.as_ptr() as mtl::Handle,
        )
    };
    direct_contexts::make_metal(&backend, None).map(|context| (queue, context))
}
