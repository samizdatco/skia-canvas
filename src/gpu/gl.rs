use surfman::{Device, Context, Connection, ContextAttributeFlags, ContextAttributes, GLVersion};
use std::cell::RefCell;
use skia_safe::gpu::DirectContext;

thread_local!(static GL_CONTEXT: RefCell<Option<GLContext>> = RefCell::new(None));

fn gl_init() -> bool {
    GL_CONTEXT.with(|cell| {
        let mut local_ctx = cell.borrow_mut();
        if local_ctx.is_none(){
            if let Some(ctx) = GLContext::new() {
                local_ctx.replace(ctx);
                true
            } else {
                false
            }
        } else {
            true
        }
    })
}

pub fn gl_supported() -> bool {
    gl_init()
}

pub fn get_gl_context() -> DirectContext {
    gl_init();
    DirectContext::new_gl(None, None).expect("Failed to create GL context")
}

struct GLContext {
    device:Device,
    context:Context
}

impl GLContext {
    pub fn new() -> Option<Self> {
        let connection = Connection::new().ok()?;
        let adapter = connection.create_hardware_adapter().ok()?;
        let mut device = connection.create_device(&adapter).ok()?;
        let context_attributes = ContextAttributes {
            version: GLVersion::new(3, 3),
            flags: ContextAttributeFlags::empty(),
        };
        let context_descriptor = device
            .create_context_descriptor(&context_attributes)
            .ok()?;
        let context = device.create_context(&context_descriptor, None).ok()?;
        device.make_context_current(&context).ok()?;
        gl::load_with(|symbol_name| device.get_proc_address(&context, symbol_name));

        Some(GLContext{device, context})
    }
}

impl Drop for GLContext {
    fn drop(&mut self) {
        self.device.destroy_context(&mut self.context).unwrap();
    }
}
