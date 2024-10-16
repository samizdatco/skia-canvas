use rafx::api::*;
use rafx::render_features::*;
use rafx::framework::*;

use super::CoordinateSystemHelper;
use super::CoordinateSystem;
use rafx::api::raw_window_handle::HasRawWindowHandle;
use std::sync::Arc;
use crate::VkSkiaContext;
use crate::skia_support::VkSkiaSurface;

use rafx::api::RafxValidationMode;

/// Controls if validation is enabled or not. Validation layers require the vulkan SDK to be
/// installed.
#[derive(Copy, Clone, Debug)]
pub enum ValidationMode {
    /// Do not enable validation.
    Disabled,

    /// Enable validation if possible (i.e. vulkan SDK is installed)
    EnabledIfAvailable,

    /// Enable validation, and fail if we cannot enable it. This requires the vulkan SDK to be
    /// installed.
    Enabled,
}

impl Default for ValidationMode {
    fn default() -> Self {
        ValidationMode::Disabled
    }
}

impl Into<RafxValidationMode> for ValidationMode {
    fn into(self) -> RafxValidationMode {
        match self {
            ValidationMode::Disabled => RafxValidationMode::Disabled,
            ValidationMode::Enabled => RafxValidationMode::Enabled,
            ValidationMode::EnabledIfAvailable => RafxValidationMode::EnabledIfAvailable,
        }
    }
}

/// A builder to create the renderer. It's easier to use AppBuilder and implement an AppHandler, but
/// initializing the renderer and maintaining the window yourself allows for more customization
#[derive(Default)]
pub struct RendererBuilder {
    coordinate_system: CoordinateSystem,
    vsync_enabled: bool,
    validation_mode: ValidationMode,
}

impl RendererBuilder {
    /// Construct the renderer builder with default options
    pub fn new() -> Self {
        RendererBuilder {
            coordinate_system: Default::default(),
            vsync_enabled: true,
            validation_mode: ValidationMode::default(),
        }
    }

    /// Determine the coordinate system to use for the canvas. This can be overridden by using the
    /// canvas sizer passed into the draw callback
    pub fn coordinate_system(
        mut self,
        coordinate_system: CoordinateSystem,
    ) -> Self {
        self.coordinate_system = coordinate_system;
        self
    }

    pub fn vsync_enabled(
        mut self,
        vsync_enabled: bool,
    ) -> Self {
        self.vsync_enabled = vsync_enabled;
        self
    }

    pub fn validation_mode(
        mut self,
        validation_mode: ValidationMode,
    ) -> Self {
        self.validation_mode = validation_mode;
        self
    }

    /// Builds the renderer. The window that's passed in will be used for creating the swapchain
    pub fn build(
        self,
        window: &dyn HasRawWindowHandle,
        window_size: RafxExtents2D,
    ) -> RafxResult<Renderer> {
        Renderer::new(
            window,
            window_size,
            self.coordinate_system,
            self.vsync_enabled,
            self.validation_mode,
        )
    }
}
struct SwapchainEventListener<'a> {
    skia_context: &'a mut VkSkiaContext,
    skia_surface: &'a mut Option<VkSkiaSurface>,
    resource_manager: &'a ResourceManager,
}

impl<'a> RafxSwapchainEventListener for SwapchainEventListener<'a> {
    fn swapchain_created(
        &mut self,
        _device_context: &RafxDeviceContext,
        swapchain: &RafxSwapchain,
    ) -> RafxResult<()> {
        *self.skia_surface = Some(VkSkiaSurface::new(
            &self.resource_manager,
            &mut self.skia_context,
            RafxExtents2D {
                width: swapchain.swapchain_def().width.max(1),
                height: swapchain.swapchain_def().height.max(1),
            },
        )?);

        Ok(())
    }

    fn swapchain_destroyed(
        &mut self,
        _device_context: &RafxDeviceContext,
        _swapchain: &RafxSwapchain,
    ) -> RafxResult<()> {
        *self.skia_surface = None;

        Ok(())
    }
}

/// Vulkan renderer that creates and manages the vulkan instance, device, swapchain, and
/// render passes.
pub struct Renderer {
    // Ordered in drop order
    pub coordinate_system: CoordinateSystem,
    pub skia_surface: Option<VkSkiaSurface>,
    pub skia_context: VkSkiaContext,
    pub skia_material_pass: MaterialPass,
    pub graphics_queue: RafxQueue,
    pub swapchain_helper: RafxSwapchainHelper,
    pub resource_manager: ResourceManager,
    #[allow(dead_code)]
    pub api: RafxApi,
}

lazy_static::lazy_static! {
    pub static ref RENDER_REGISTRY: RenderRegistry = RenderRegistryBuilder::default()
            .register_render_phase::<OpaqueRenderPhase>("opaque")
            .build();
}

impl Renderer {
    /// Create the renderer
    pub fn new(
        window: &dyn HasRawWindowHandle,
        window_size: RafxExtents2D,
        coordinate_system: CoordinateSystem,
        vsync_enabled: bool,
        validation_mode: ValidationMode,
    ) -> RafxResult<Renderer> {
        let api_def = RafxApiDefVulkan {
            validation_mode: validation_mode.into(),
            ..Default::default()
        };

        let api = unsafe { RafxApi::new_vulkan(window, &Default::default(), &api_def) }?;
        let device_context = api.device_context();

        let resource_manager =
            rafx::framework::ResourceManager::new(&device_context, &RENDER_REGISTRY);

        let swapchain = device_context.create_swapchain(
            window,
            &RafxSwapchainDef {
                width: window_size.width,
                height: window_size.height,
                enable_vsync: vsync_enabled,
            },
        )?;

        let graphics_queue = device_context.create_queue(RafxQueueType::Graphics)?;

        let mut skia_context = VkSkiaContext::new(&device_context, &graphics_queue);
        let mut skia_surface = None;

        let swapchain_helper = RafxSwapchainHelper::new(
            &device_context,
            swapchain,
            Some(&mut SwapchainEventListener {
                skia_context: &mut skia_context,
                skia_surface: &mut skia_surface,
                resource_manager: &resource_manager,
            }),
        )?;

        let resource_context = resource_manager.resource_context();

        let skia_material_pass = Self::load_material_pass(
            &resource_context,
            include_bytes!("../shaders/out/skia.vert.cookedshaderpackage"),
            include_bytes!("../shaders/out/skia.frag.cookedshaderpackage"),
            FixedFunctionState {
                rasterizer_state: Default::default(),
                depth_state: Default::default(),
                blend_state: Default::default(),
            },
        )?;

        Ok(Renderer {
            api,
            resource_manager,
            swapchain_helper,
            graphics_queue,
            skia_material_pass,
            coordinate_system,
            skia_context,
            skia_surface,
        })
    }

    /// Call to render a frame. This can block for certain presentation modes. This will rebuild
    /// the swapchain if necessary.
    pub fn draw<F: FnOnce(&mut skia_safe::Canvas, CoordinateSystemHelper)>(
        &mut self,
        window_size: RafxExtents2D,
        scale_factor: f64,
        f: F,
    ) -> RafxResult<()> {
        //
        // Begin the frame
        //
        let frame = self.swapchain_helper.acquire_next_image(
            window_size.width,
            window_size.height,
            Some(&mut SwapchainEventListener {
                skia_context: &mut self.skia_context,
                skia_surface: &mut self.skia_surface,
                resource_manager: &self.resource_manager,
            }),
        )?;

        // Acquiring an image means a prior frame completely finished processing
        self.resource_manager.on_frame_complete()?;

        //
        // Do skia drawing (including the user's callback)
        //
        let mut canvas = self.skia_surface.as_mut().unwrap().surface.canvas();

        let coordinate_system_helper = CoordinateSystemHelper::new(window_size, scale_factor);

        match self.coordinate_system {
            CoordinateSystem::None => {}
            CoordinateSystem::Physical => {
                coordinate_system_helper.use_physical_coordinates(&mut canvas)
            }
            CoordinateSystem::Logical => {
                coordinate_system_helper.use_logical_coordinates(&mut canvas)
            }
            CoordinateSystem::VisibleRange(range, scale_to_fit) => coordinate_system_helper
                .use_visible_range(&mut canvas, range, scale_to_fit)
                .unwrap(),
            CoordinateSystem::FixedWidth(center, x_half_extents) => coordinate_system_helper
                .use_fixed_width(&mut canvas, center, x_half_extents)
                .unwrap(),
        }

        f(&mut canvas, coordinate_system_helper);
        self.skia_context.context.flush_and_submit();

        //
        // Convert the skia texture to a shader resources, draw a quad, and convert it back to a
        // render target
        //
        let mut descriptor_set_allocator = self.resource_manager.create_descriptor_set_allocator();
        let mut descriptor_set = descriptor_set_allocator.create_dyn_descriptor_set_uninitialized(
            &self
                .skia_material_pass
                .material_pass_resource
                .get_raw()
                .descriptor_set_layouts[0],
        )?;

        descriptor_set.set_image(1, &self.skia_surface.as_ref().unwrap().image_view);

        descriptor_set.flush(&mut descriptor_set_allocator)?;
        descriptor_set_allocator.flush_changes()?;

        let mut command_pool = self
            .resource_manager
            .dyn_command_pool_allocator()
            .allocate_dyn_pool(
                &self.graphics_queue,
                &RafxCommandPoolDef { transient: false },
                0,
            )?;

        let command_buffer = command_pool.allocate_dyn_command_buffer(&RafxCommandBufferDef {
            is_secondary: false,
        })?;

        command_buffer.begin()?;

        command_buffer.cmd_resource_barrier(
            &[],
            &[
                RafxTextureBarrier {
                    texture: frame.swapchain_texture(),
                    array_slice: None,
                    mip_slice: None,
                    src_state: RafxResourceState::PRESENT,
                    dst_state: RafxResourceState::RENDER_TARGET,
                    queue_transition: RafxBarrierQueueTransition::None,
                },
                RafxTextureBarrier {
                    texture: &self
                        .skia_surface
                        .as_ref()
                        .unwrap()
                        .image_view
                        .get_raw()
                        .image
                        .get_raw()
                        .image,
                    array_slice: None,
                    mip_slice: None,
                    src_state: RafxResourceState::RENDER_TARGET,
                    dst_state: RafxResourceState::SHADER_RESOURCE,
                    queue_transition: RafxBarrierQueueTransition::None,
                },
            ],
        )?;

        command_buffer.cmd_begin_render_pass(
            &[RafxColorRenderTargetBinding {
                texture: frame.swapchain_texture(),
                load_op: RafxLoadOp::DontCare,
                store_op: RafxStoreOp::Store,
                clear_value: RafxColorClearValue([0.0, 0.0, 0.0, 0.0]),
                mip_slice: Default::default(),
                array_slice: Default::default(),
                resolve_target: Default::default(),
                resolve_store_op: Default::default(),
                resolve_mip_slice: Default::default(),
                resolve_array_slice: Default::default(),
            }],
            None,
        )?;

        let pipeline = self
            .resource_manager
            .graphics_pipeline_cache()
            .get_or_create_graphics_pipeline(
                OpaqueRenderPhase::render_phase_index(),
                &self.skia_material_pass.material_pass_resource,
                &GraphicsPipelineRenderTargetMeta::new(
                    vec![self.swapchain_helper.format()],
                    None,
                    RafxSampleCount::SampleCount1,
                ),
                &*VERTEX_LAYOUT,
            )?;

        let vertex_buffer = self
            .resource_manager
            .device_context()
            .create_buffer(&RafxBufferDef::for_staging_vertex_buffer_data(&VERTEX_LIST))?;
        vertex_buffer.copy_to_host_visible_buffer(&VERTEX_LIST)?;

        let vertex_buffer = self
            .resource_manager
            .create_dyn_resource_allocator_set()
            .insert_buffer(vertex_buffer);

        command_buffer.cmd_bind_pipeline(&*pipeline.get_raw().pipeline)?;
        command_buffer.cmd_bind_vertex_buffers(
            0,
            &[RafxVertexBufferBinding {
                buffer: &*vertex_buffer.get_raw().buffer,
                byte_offset: 0,
            }],
        )?;
        descriptor_set.bind(&command_buffer)?;

        command_buffer.cmd_draw(6, 0)?;

        command_buffer.cmd_end_render_pass()?;

        command_buffer.cmd_resource_barrier(
            &[],
            &[
                RafxTextureBarrier {
                    texture: frame.swapchain_texture(),
                    array_slice: None,
                    mip_slice: None,
                    src_state: RafxResourceState::RENDER_TARGET,
                    dst_state: RafxResourceState::PRESENT,
                    queue_transition: RafxBarrierQueueTransition::None,
                },
                RafxTextureBarrier {
                    texture: &self
                        .skia_surface
                        .as_ref()
                        .unwrap()
                        .image_view
                        .get_raw()
                        .image
                        .get_raw()
                        .image,
                    array_slice: None,
                    mip_slice: None,
                    src_state: RafxResourceState::SHADER_RESOURCE,
                    dst_state: RafxResourceState::RENDER_TARGET,
                    queue_transition: RafxBarrierQueueTransition::None,
                },
            ],
        )?;

        command_buffer.end()?;

        frame.present(&self.graphics_queue, &[&command_buffer])?;

        Ok(())
    }

    fn load_material_pass(
        resource_context: &ResourceContext,
        cooked_vertex_shader_bytes: &[u8],
        cooked_fragment_shader_bytes: &[u8],
        fixed_function_state: FixedFunctionState,
    ) -> RafxResult<MaterialPass> {
        let cooked_vertex_shader_stage =
            bincode::deserialize::<CookedShaderPackage>(cooked_vertex_shader_bytes)
                .map_err(|x| format!("Failed to deserialize cooked shader: {:?}", x))?;
        let vertex_shader_module = resource_context
            .resources()
            .get_or_create_shader_module_from_cooked_package(&cooked_vertex_shader_stage)?;
        let vertex_entry_point = cooked_vertex_shader_stage
            .find_entry_point("main")
            .unwrap()
            .clone();

        // Create the fragment shader module and find the entry point
        let cooked_fragment_shader_stage =
            bincode::deserialize::<CookedShaderPackage>(cooked_fragment_shader_bytes)
                .map_err(|x| format!("Failed to deserialize cooked shader: {:?}", x))?;
        let fragment_shader_module = resource_context
            .resources()
            .get_or_create_shader_module_from_cooked_package(&cooked_fragment_shader_stage)?;
        let fragment_entry_point = cooked_fragment_shader_stage
            .find_entry_point("main")
            .unwrap()
            .clone();

        let fixed_function_state = Arc::new(fixed_function_state);

        let material_pass = MaterialPass::new(
            &resource_context,
            fixed_function_state,
            vec![vertex_shader_module, fragment_shader_module],
            &[&vertex_entry_point, &fragment_entry_point],
        )?;

        Ok(material_pass)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        debug!("destroying Renderer");
        self.graphics_queue.wait_for_queue_idle().unwrap();
        debug!("destroyed Renderer");
    }
}

rafx::declare_render_phase!(
    OpaqueRenderPhase,
    OPAQUE_RENDER_PHASE_INDEX,
    opaque_render_phase_sort_submit_nodes
);

fn opaque_render_phase_sort_submit_nodes(_submit_nodes: &mut Vec<RenderFeatureSubmitNode>) {
    // No sort needed
}

#[derive(Clone, Debug, Copy)]
struct Vertex {
    pos: [f32; 2],
    tex_coord: [f32; 2],
}

const VERTEX_LIST: [Vertex; 6] = [
    Vertex {
        pos: [-1.0, -1.0],
        tex_coord: [0.0, 1.0],
    },
    Vertex {
        pos: [1.0, -1.0],
        tex_coord: [1.0, 1.0],
    },
    Vertex {
        pos: [1.0, 1.0],
        tex_coord: [1.0, 0.0],
    },
    Vertex {
        pos: [1.0, 1.0],
        tex_coord: [1.0, 0.0],
    },
    Vertex {
        pos: [-1.0, 1.0],
        tex_coord: [0.0, 0.0],
    },
    Vertex {
        pos: [-1.0, -1.0],
        tex_coord: [0.0, 1.0],
    },
];

lazy_static::lazy_static! {
    pub static ref VERTEX_LAYOUT : VertexDataSetLayout = {
        use rafx::api::RafxFormat;

        let vertex = Vertex {
            pos: Default::default(),
            tex_coord: Default::default(),
        };

        VertexDataLayout::build_vertex_layout(&vertex, RafxVertexAttributeRate::Vertex, |builder, vertex| {
            builder.add_member(&vertex.pos, "POSITION", RafxFormat::R32G32_SFLOAT);
            builder.add_member(&vertex.tex_coord, "TEXCOORD", RafxFormat::R32G32_SFLOAT);
        }).into_set(RafxPrimitiveTopology::TriangleList)
    };
}
