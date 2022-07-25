use std::cell::RefCell;
use foreign_types::ForeignType;
use metal_rs::Device;
use skia_safe::gpu::{mtl, DirectContext, SurfaceOrigin};
use skia_safe::{Budgeted, ImageInfo, Surface};
thread_local!(static MTL_CONTEXT: RefCell<Option<Engine>> = RefCell::new(None));

pub struct Engine {
    context: DirectContext,
}

impl Engine {
    fn init() {
        MTL_CONTEXT.with(|cell| {
            let mut local_ctx = cell.borrow_mut();
            if local_ctx.is_none(){
                if let Some(ctx) = Engine::new() {
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
          Some(Engine{context})
      }else{
          None
      }

    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        Self::init();
        MTL_CONTEXT.with(|cell| {
            let local_ctx = cell.borrow();
            let mut context = local_ctx.as_ref().unwrap().context.clone();

            Surface::new_render_target(
                &mut context,
                Budgeted::Yes,
                image_info,
                Some(4),
                SurfaceOrigin::BottomLeft,
                None,
                true,
            )
        })
    }
}
