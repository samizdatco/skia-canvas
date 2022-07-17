use std::cell::RefCell;
use skia_safe::gpu::DirectContext;

thread_local!(static MTL_CONTEXT: RefCell<Option<Metal>> = RefCell::new(None));

#[cfg(target_os = "macos")]
pub struct Metal {
    context: DirectContext,
}

#[cfg(target_os = "macos")]
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
      use foreign_types_shared::ForeignType;
      use metal_rs::Device;
      use skia_safe::gpu::mtl;

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

// #[cfg(target_os = "macos")]
// impl Drop for Metal {
//     fn drop(&mut self) {
//         self.device.destroy_context(&mut self.context).unwrap();
//     }
// }

//
// a dummy struct for linux & windows to ignore
//

#[cfg(not(target_os = "macos"))]
pub struct Metal {}

#[cfg(not(target_os = "macos"))]
impl Metal {
    pub fn new() -> Option<Self> { None }
    pub fn supported() -> bool { false }
    pub fn direct_context() -> Option<DirectContext> { None }
}

