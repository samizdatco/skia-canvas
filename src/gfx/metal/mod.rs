use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLCommandQueue, MTLDevice};
use skia_safe::gpu::{ mtl, direct_contexts, DirectContext };

pub mod offscreen;

#[cfg(feature = "window")]
pub mod window;

// create a Skia rendering context for use by either on- or offscreen renderers
fn make_direct_context(device:&ProtocolObject<dyn MTLDevice>) -> Option<(Retained<ProtocolObject<dyn MTLCommandQueue>>, DirectContext)>{
    let queue = device.newCommandQueue()?;
    let backend = unsafe {
        mtl::BackendContext::new(
            std::ptr::from_ref(device) as mtl::Handle,
            Retained::as_ptr(&queue) as mtl::Handle,
        )
    };
    direct_contexts::make_metal(&backend, None).map(|context| (queue, context))
}
