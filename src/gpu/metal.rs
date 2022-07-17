use std::cell::RefCell;
use skia_safe::gpu::DirectContext;
use foreign_types_shared::ForeignType;
use metal_rs::Device;
use skia_safe::gpu::mtl;

thread_local!(static MTL_CONTEXT: RefCell<Option<Metal>> = RefCell::new(None));

pub struct Metal {
    context: DirectContext,
}

impl Metal {
    fn init() {
        MTL_CONTEXT.with(|cell| {
            let mut local_ctx = cell.borrow_mut();
            if local_ctx.is_none(){
                if let Some(ctx) = Metal::new() {
                    local_ctx.replace(ctx);
                }
            }
        })
    }

    pub fn supported() -> bool {
        Self::init();
        MTL_CONTEXT.with(|cell| cell.borrow().is_some() )
    }

    pub fn new() -> Option<Self> {
      let device = Device::system_default()?;
      let command_queue = device.new_command_queue();
      let backend_context = unsafe {
        mtl::BackendContext::new(
            device.as_ptr() as mtl::Handle,
            command_queue.as_ptr() as mtl::Handle,
            std::ptr::null(),
        )
      };
      if let Some(context) = DirectContext::new_metal(&backend_context, None){
        Some(Metal{context})
      }else{
        None
      }

    }

    pub fn direct_context() -> Option<DirectContext> {
        Self::init();
        MTL_CONTEXT.with(|cell| {
          let local_ctx = cell.borrow();
          Some(local_ctx.as_ref().unwrap().context.clone())
        })
      }

}
