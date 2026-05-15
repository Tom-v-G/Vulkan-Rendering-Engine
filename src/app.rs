use anyhow::{anyhow, Result};
use bytemuck::bytes_of;
use egui::{FontId, RichText};
use std::mem::size_of;
use std::ptr::copy_nonoverlapping as memcpy;
use std::time::Instant;
// use winit::window::Window;
use egui_winit::winit::window::Window;
use rand::seq::{IndexedRandom, SliceRandom};

use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::ExtDebugUtilsExtension;
use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::vk::KhrSwapchainExtension;
use vulkanalia::window as vk_window;

use crate::app_data::AppData;
use crate::buffers::{
    create_index_buffer, create_uniform_buffers, create_vertex_buffer, UniformBufferObject,
};
use crate::camera::Camera;
use crate::chunk::{Chunk, ChunkCoord};
use crate::chunk_rendering::{create_chunk_pipeline, ChunkPushConstants, GpuChunk};
use crate::chunkmesher::MeshData;
use crate::commands::{create_command_buffers, create_command_pools, create_sync_objects};
use crate::descriptors::{create_descriptor_pool, create_descriptor_sets};
use crate::device::{create_logical_device, pick_physical_device};
use crate::gui::Gui;
use crate::images::{create_color_objects, create_depth_objects};
use crate::instance::create_instance;
use crate::load_models::load_model;
use crate::pipeline::{
    create_descriptor_set_layout, create_framebuffers, create_gui_framebuffers, create_pipeline,
    create_render_pass,
};
use crate::swapchain::{create_swapchain, create_swapchain_image_views};
use crate::textures::{create_texture_image, create_texture_image_view, create_texture_sampler};
use crate::utils::*;
use crate::voxel::Voxel;
use crate::{constants::*, RuntimeState};

use glam::{Mat4, Vec3};

pub struct RenderApp {
    pub entry: Entry,
    pub instance: Instance,
    pub data: AppData,
    pub device: Device,
    pub frame: usize,
    pub resized: bool,
    pub start: Instant,
    pub camera: Camera,
    pub models: usize,
    pub gui: Gui,
    pub shutdown_triggered: bool,
    pub menu_mode: bool,
}

impl RenderApp {
    /// Creates our Vulkan app.
    pub unsafe fn create(window: &Window) -> Result<Self> {
        let loader = LibloadingLoader::new(LIBRARY)?;
        let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
        let mut data = AppData::default();
        let camera = Camera::new();
        let instance = create_instance(window, &entry, &mut data)?;
        let frame = 0;
        let resized = false;
        let start = Instant::now();
        let models = 1;
        let shutdown_triggered = false;
        let menu_mode = false;

        println!("Creating device");
        data.surface = vk_window::create_surface(&instance, &window, &window)?;
        pick_physical_device(&instance, &mut data)?;
        let device = create_logical_device(&entry, &instance, &mut data)?;

        println!("Creating Pipeline components");
        create_swapchain(window, &instance, &device, &mut data)?;
        create_swapchain_image_views(&device, &mut data)?;
        create_render_pass(&instance, &device, &mut data)?;
        data.descriptor_set_layout = create_descriptor_set_layout(&device)?;
        create_pipeline(&instance, &device, &mut data)?;
        create_command_pools(&instance, &device, &mut data)?;

        println!("Creating Models");
        create_color_objects(&instance, &device, &mut data)?;
        create_depth_objects(&instance, &device, &mut data)?;
        create_framebuffers(&device, &mut data)?;
        create_texture_image(&instance, &device, &mut data)?;
        create_texture_image_view(&device, &mut data)?;
        create_texture_sampler(&device, &mut data)?;
        // load in model and create buffers
        load_model(&mut data)?;
        (data.vertex_buffer, data.vertex_buffer_memory) = create_vertex_buffer(
            &instance,
            &device,
            data.physical_device,
            &data.vertices,
            data.graphics_queue,
            data.command_pool,
        )?;
        (data.index_buffer, data.index_buffer_memory) = create_index_buffer(
            &instance,
            &device,
            data.physical_device,
            &data.indices,
            data.graphics_queue,
            data.command_pool,
        )?;
        create_uniform_buffers(&instance, &device, &mut data)?;
        create_descriptor_pool(&device, &mut data)?;
        create_descriptor_sets(&device, &mut data)?;
        create_command_buffers(&device, &mut data)?;
        create_sync_objects(&device, &mut data)?;

        println!("Creating GUI");
        let gui = Gui::create(
            &window,
            &instance,
            &device,
            &data,
            data.swapchain_images.len(),
        )?;
        println!("Creating GUI framebuffers");
        create_gui_framebuffers(&device, &mut data, &gui)?;

        println!("Creating Voxel Pipeline");
        create_chunk_pipeline(&device, &mut data)?;

        println!("App created");
        Ok(Self {
            entry,
            instance,
            data,
            device,
            frame,
            resized,
            start,
            camera,
            models,
            gui,
            shutdown_triggered,
            menu_mode,
        })
    }

    pub unsafe fn recreate_swapchain(&mut self, window: &Window) -> Result<()> {
        self.device.device_wait_idle()?;
        self.destroy_swapchain();
        create_swapchain(window, &self.instance, &self.device, &mut self.data)?;
        create_swapchain_image_views(&self.device, &mut self.data)?;
        create_render_pass(&self.instance, &self.device, &mut self.data)?;
        create_pipeline(&self.instance, &self.device, &mut self.data)?;
        create_chunk_pipeline(&self.device, &mut self.data)?;
        create_color_objects(&self.instance, &self.device, &mut self.data)?;
        create_depth_objects(&self.instance, &self.device, &mut self.data)?;
        create_framebuffers(&self.device, &mut self.data)?;

        // Destroy old GUI framebuffers
        self.data
            .gui_framebuffers
            .iter()
            .for_each(|f| self.device.destroy_framebuffer(*f, None));

        // Recreate for new swapchain size
        create_gui_framebuffers(&self.device, &mut self.data, &self.gui)?;

        create_uniform_buffers(&self.instance, &self.device, &mut self.data)?;
        create_descriptor_pool(&self.device, &mut self.data)?;
        create_descriptor_sets(&self.device, &mut self.data)?;
        create_command_buffers(&self.device, &mut self.data)?;

        self.data
            .images_in_flight
            .resize(self.data.swapchain_images.len(), vk::Fence::null());
        Ok(())
    }

    /// Renders a frame for our Vulkan app.
    pub unsafe fn render(&mut self, window: &Window, runtime: &RuntimeState) -> Result<()> {
        self.device.wait_for_fences(
            &[self.data.frames[self.frame].in_flight_fence],
            true,
            u64::MAX,
        )?;

        // Destroy items on the deletion queue
        self.data.frames[self.frame]
            .deletion_queue
            .flush(&self.device);

        let result = self.device.acquire_next_image_khr(
            self.data.swapchain,
            u64::MAX,
            self.data.frames[self.frame].image_available_semaphore,
            vk::Fence::null(),
        );

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => return self.recreate_swapchain(window),
            Err(e) => return Err(anyhow!(e)),
        };

        if !self.data.images_in_flight[image_index as usize].is_null() {
            self.device.wait_for_fences(
                &[self.data.images_in_flight[image_index as usize]],
                true,
                u64::MAX,
            )?;
        }

        self.data.images_in_flight[image_index as usize] =
            self.data.frames[self.frame].in_flight_fence;

        // ---- GUI ----
        // probably a cleaner way to do this
        let mut wireframe = false;
        let mut show_normals = false;
        let mut render_distance = 3;

        let ctx = self.gui.begin_frame(window);
        egui::SidePanel::right("Sidepanel")
            .default_width(300.0)
            .width_range(250.0..=std::f32::INFINITY)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading(RichText::new("Engine Test").font(FontId::proportional(40.0)));

                egui::CollapsingHeader::new(
                    RichText::new("Performance").font(FontId::proportional(25.0)),
                )
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("perf_grid").show(ui, |ui| {
                        ui.label("FPS");
                        ui.label(format!("{:.1}", runtime.metrics.frametimer.fps()));
                        ui.end_row();
                        ui.label("CPU");
                        ui.label(format!("{:.2} %", runtime.metrics.cpu_usage));
                        ui.end_row();
                        ui.label("RAM");
                        ui.label(format!("{} mb", runtime.metrics.memory_mb));
                        ui.end_row();

                        ui.label("Camera Position");
                        ui.end_row();
                        let (camera_x, camera_y, camera_z) =
                            <(f32, f32, f32)>::from(self.camera.eye);
                        ui.label(format!("{:.2}, {:.2}, {:.2}", camera_x, camera_y, camera_z));
                        ui.end_row();
                        ui.label("Camera Direction");
                        ui.end_row();
                        let (cam_dir_x, cam_dir_y, cam_dir_z) =
                            <(f32, f32, f32)>::from(self.camera.direction());
                        ui.label(format!(
                            "{:.2}, {:.2}, {:.2}",
                            cam_dir_x, cam_dir_y, cam_dir_z
                        ));
                        ui.end_row();
                        ui.label("Chunks");
                        ui.label(format!("1"));
                        ui.end_row();
                    });
                });

                egui::CollapsingHeader::new(
                    RichText::new("Rendering").font(FontId::proportional(25.0)),
                )
                .default_open(true)
                .show(ui, |ui| {
                    ui.checkbox(&mut wireframe, "Wireframe");
                    ui.checkbox(&mut show_normals, "Normals");

                    ui.add(egui::Slider::new(&mut render_distance, 2..=32).text("Render Distance"));
                });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if self.menu_mode {
                        ui.label(RichText::new("Menu Mode").font(FontId::proportional(25.0)));
                    }
                })
            });

        egui::Window::new("Debug").show(ctx, |ui| {
            ui.label(RichText::new("Large Text").font(FontId::proportional(40.0)));
            ui.label(
                RichText::new(format!("Models: {}", self.models))
                    .font(FontId::proportional(30.0))
                    .color(egui::Color32::RED),
            );
            // ui.label(
            //     RichText::new(format!("Frame time: {} s", self.start.elapsed().as_secs()))
            //         .font(FontId::proportional(30.0)),
            // );
        });

        self.update_command_buffer(image_index, window, runtime)?; // <- GUI render also happens in here
        self.update_uniform_buffer(image_index, &self.camera)?;

        let wait_semaphores = &[self.data.frames[self.frame].image_available_semaphore];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

        let command_buffers = &[self.data.command_buffers[image_index as usize]];
        let signal_semaphores = &[self.data.frames[self.frame].render_finished_semaphore];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        self.device
            .reset_fences(&[self.data.frames[self.frame].in_flight_fence])?;

        self.device.queue_submit(
            self.data.graphics_queue,
            &[submit_info],
            self.data.frames[self.frame].in_flight_fence,
        )?;

        let swapchains = &[self.data.swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);

        let result = self
            .device
            .queue_present_khr(self.data.present_queue, &present_info);

        let changed = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR)
            || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);

        if self.resized || changed {
            self.resized = false;
            self.recreate_swapchain(window)?;
        } else if let Err(e) = result {
            return Err(anyhow!(e));
        }

        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;

        Ok(())
    }

    /// Destroys our Vulkan app.
    pub unsafe fn destroy(&mut self, runtime: &mut RuntimeState) {
        // Destroy Runtime Gpu Components
        // (uploaded chunks)
        runtime.chunk_manager.destroy(&self.instance, &self.device);

        self.destroy_swapchain();
        self.data
            .command_pools
            .iter()
            .for_each(|p| self.device.destroy_command_pool(*p, None));

        self.device.destroy_sampler(self.data.texture_sampler, None);
        self.device
            .destroy_image_view(self.data.texture_image_view, None);
        self.device.destroy_image(self.data.texture_image, None);
        self.device
            .free_memory(self.data.texture_image_memory, None);
        self.device
            .destroy_descriptor_set_layout(self.data.descriptor_set_layout, None);
        self.device.destroy_buffer(self.data.index_buffer, None);
        self.device.free_memory(self.data.index_buffer_memory, None);
        self.device.destroy_buffer(self.data.vertex_buffer, None);
        self.device
            .free_memory(self.data.vertex_buffer_memory, None);

        self.data
            .frames
            .iter_mut()
            .for_each(|f| f.destroy(&self.device));

        self.device
            .destroy_command_pool(self.data.command_pool, None);

        self.gui.destroy(&self.device); // ADD

        // Also destroy gui framebuffers
        self.data
            .gui_framebuffers
            .iter()
            .for_each(|f| self.device.destroy_framebuffer(*f, None));

        self.device.destroy_device(None);
        self.instance.destroy_surface_khr(self.data.surface, None);

        if VALIDATION_ENABLED {
            self.instance
                .destroy_debug_utils_messenger_ext(self.data.messenger, None);
        }

        self.instance.destroy_instance(None);
    }

    pub unsafe fn destroy_swapchain(&mut self) {
        // MSAA sampler
        self.device
            .destroy_image_view(self.data.color_image_view, None);
        self.device.free_memory(self.data.color_image_memory, None);
        self.device.destroy_image(self.data.color_image, None);
        // depth buffer
        self.device
            .destroy_image_view(self.data.depth_image_view, None);
        self.device.free_memory(self.data.depth_image_memory, None);
        self.device.destroy_image(self.data.depth_image, None);
        // descriptor pool
        self.device
            .destroy_descriptor_pool(self.data.descriptor_pool, None);
        self.data
            .uniform_buffers
            .iter()
            .for_each(|b| self.device.destroy_buffer(*b, None));
        self.data
            .uniform_buffers_memory
            .iter()
            .for_each(|m| self.device.free_memory(*m, None));
        self.data
            .framebuffers
            .iter()
            .for_each(|f| self.device.destroy_framebuffer(*f, None));
        // uncomment when switching to single command pool
        // self.device
        //     .free_command_buffers(self.data.command_pool, &self.data.command_buffers);
        self.device.destroy_pipeline(self.data.pipeline, None);
        self.device
            .destroy_pipeline_layout(self.data.pipeline_layout, None);
        self.device.destroy_pipeline(self.data.voxel_pipeline, None);
        self.device
            .destroy_pipeline_layout(self.data.voxel_pipeline_layout, None);
        self.device.destroy_render_pass(self.data.render_pass, None);
        self.data
            .swapchain_image_views
            .iter()
            .for_each(|v| self.device.destroy_image_view(*v, None));
        self.device.destroy_swapchain_khr(self.data.swapchain, None);
    }

    pub unsafe fn update_uniform_buffer(&self, image_index: usize, camera: &Camera) -> Result<()> {
        // let view = Mat4::look_at_rh(
        //     point3(6.0, 0.0, 2.0),
        //     point3(0.0, 0.0, 0.0),
        //     vec3(0.0, 0.0, 1.0),
        // );

        let view = camera.view_matrix();

        // Update this to rows if incorrect
        let correction = Mat4::from_cols_array(&[
            1.0,
            0.0,
            0.0,
            0.0,
            // We're also flipping the Y-axis with this line's `-1.0`.
            0.0,
            -1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0 / 2.0,
            0.0,
            0.0,
            0.0,
            1.0 / 2.0,
            1.0,
        ]);

        let fov_y = 45.0_f32.to_radians(); // 45 degrees in radians
        let aspect_ratio =
            self.data.swapchain_extent.width as f32 / self.data.swapchain_extent.height as f32;
        let near = 0.1;
        let far = 9999.0;

        let proj = correction * Mat4::perspective_rh(fov_y, aspect_ratio, near, far);
        let ubo = UniformBufferObject { view, proj };

        let memory = self.device.map_memory(
            self.data.uniform_buffers_memory[image_index],
            0,
            size_of::<UniformBufferObject>() as u64,
            vk::MemoryMapFlags::empty(),
        )?;

        memcpy(&ubo, memory.cast(), 1);

        self.device
            .unmap_memory(self.data.uniform_buffers_memory[image_index]);

        Ok(())
    }

    pub unsafe fn update_command_buffer(
        &mut self,
        image_index: usize,
        window: &Window,
        runtime: &RuntimeState,
    ) -> Result<()> {
        //Approach 3: resetting command pools

        // Pick right command pool
        let command_pool = self.data.command_pools[image_index];
        self.device
            .reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty())?;

        let command_buffer = self.data.command_buffers[image_index];

        // Approach 1: Reset command buffer

        // let command_buffer = self.data.command_buffers[image_index];
        // self.device.reset_command_buffer(
        //     command_buffer,
        //     vk::CommandBufferResetFlags::empty(),
        // )?;

        // Approach 2: Create new command buffer

        // // clear previous buffer
        // let previous = self.data.command_buffers[image_index];
        // self.device.free_command_buffers(self.data.command_pool, &[previous]);
        // // allocate new buffer in memory
        // let allocate_info = vk::CommandBufferAllocateInfo::builder()
        //     .command_pool(self.data.command_pool)
        //     .level(vk::CommandBufferLevel::PRIMARY)
        //     .command_buffer_count(1);

        // let command_buffer = self.device.allocate_command_buffers(&allocate_info)?[0];
        // self.data.command_buffers[image_index] = command_buffer;

        // Fill buffer with new data

        let info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        self.device.begin_command_buffer(command_buffer, &info)?;

        let render_area = vk::Rect2D::builder()
            .offset(vk::Offset2D::default())
            .extent(self.data.swapchain_extent);

        let color_clear_value = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        };

        let depth_clear_value = vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        };

        let clear_values = &[color_clear_value, depth_clear_value];
        let info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.data.render_pass)
            .framebuffer(self.data.framebuffers[image_index])
            .render_area(render_area)
            .clear_values(clear_values);

        self.device.cmd_begin_render_pass(
            command_buffer,
            &info,
            // vk::SubpassContents::SECONDARY_COMMAND_BUFFERS, //only works if all rendering is done in SCBs
            vk::SubpassContents::INLINE,
        );

        // // Models are passed here
        // let secondary_command_buffers = (0..self.models)
        //     .map(|i| self.update_secondary_command_buffer(image_index, i))
        //     .collect::<Result<Vec<_>, _>>()?;

        // if !secondary_command_buffers.is_empty() {
        //     self.device
        //         .cmd_execute_commands(command_buffer, &secondary_command_buffers[..]);
        // }

        // Draw chunks on the primary buffer (for now)

        self.draw_chunks(
            command_buffer,
            runtime.chunk_manager.visible_chunks(),
            image_index,
        )?;

        self.device.cmd_end_render_pass(command_buffer);

        // GUI call appended here (can I do this in a secondary command buffer?)
        self.gui.end_frame_and_render(
            // window is not available here, so we need to pass it in — see note below
            &window, // see note
            &self.instance,
            &self.device,
            &self.data,
            command_buffer,
            self.data.gui_framebuffers[image_index],
            image_index,
            self.data.swapchain_extent,
        )?;

        self.device.end_command_buffer(command_buffer)?;

        // queue gpu chunks for deletion
        // self.data.frames[self.frame]
        //     .deletion_queue
        //     .push(gpu_chunk.vertex_buffer, gpu_chunk.vertex_buffer_memory);
        // self.data.frames[self.frame]
        //     .deletion_queue
        //     .push(gpu_chunk.index_buffer, gpu_chunk.index_buffer_memory);

        // TODO: check if a deletion queue is necesary for the frames

        Ok(())
    }

    pub unsafe fn update_secondary_command_buffer(
        &mut self,
        image_index: usize,
        model_index: usize,
    ) -> Result<vk::CommandBuffer> {
        self.data
            .secondary_command_buffers
            .resize_with(image_index + 1, Vec::new);
        let command_buffers = &mut self.data.secondary_command_buffers[image_index];

        // Update command buffer vec to right length
        while model_index >= command_buffers.len() {
            let allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(self.data.command_pools[image_index])
                .level(vk::CommandBufferLevel::SECONDARY)
                .command_buffer_count(1);

            let command_buffer = self.device.allocate_command_buffers(&allocate_info)?[0];

            command_buffers.push(command_buffer);
        }

        // Command buffer update
        let command_buffer = command_buffers[model_index];

        // Tutorial Model
        let y = (((model_index % 2) as f32) * 2.5) - 1.25;
        let z = (((model_index / 2) as f32) * -2.0) + 1.0;

        // UBO model matrix to push constant

        // let time = self.start.elapsed().as_secs_f32();
        let time: f32 = 4.5;

        let model = Mat4::from_translation(Vec3::new(0.0, y, z))
            * Mat4::from_axis_angle(Vec3::new(0.0, 0., 1.0), 90.0_f32.to_radians() * time);

        let model_bytes =
            std::slice::from_raw_parts(&model as *const Mat4 as *const u8, size_of::<Mat4>());

        let opacity = (model_index + 1) as f32 * 0.25;
        let opacity_bytes = &opacity.to_ne_bytes()[..];

        // Commands

        let inheritance_info = vk::CommandBufferInheritanceInfo::builder()
            .render_pass(self.data.render_pass)
            .subpass(0)
            .framebuffer(self.data.framebuffers[image_index]);

        let info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
            .inheritance_info(&inheritance_info);

        self.device.begin_command_buffer(command_buffer, &info)?;

        self.device.cmd_bind_pipeline(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.data.pipeline,
        );
        self.device
            .cmd_bind_vertex_buffers(command_buffer, 0, &[self.data.vertex_buffer], &[0]);
        self.device.cmd_bind_index_buffer(
            command_buffer,
            self.data.index_buffer,
            0,
            vk::IndexType::UINT32,
        );
        self.device.cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.data.pipeline_layout,
            0,
            &[self.data.descriptor_sets[image_index]],
            &[],
        );
        self.device.cmd_push_constants(
            command_buffer,
            self.data.pipeline_layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            model_bytes,
        );
        self.device.cmd_push_constants(
            command_buffer,
            self.data.pipeline_layout,
            vk::ShaderStageFlags::FRAGMENT,
            64,
            opacity_bytes,
        );
        self.device
            .cmd_draw_indexed(command_buffer, self.data.indices.len() as u32, 1, 0, 0, 0);

        self.device.end_command_buffer(command_buffer)?;

        Ok(command_buffer)
    }

    pub unsafe fn draw_chunks<'a, I>(
        &self,
        command_buffer: vk::CommandBuffer,
        visible_chunks: I,
        image_index: usize,
    ) -> Result<()>
    where
        I: IntoIterator<Item = &'a GpuChunk>,
    {
        // Bind pipeline once — all chunks share the same pipeline
        self.device.cmd_bind_pipeline(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.data.voxel_pipeline,
        );

        // Bind global descriptor set once (camera UBO)
        self.device.cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.data.voxel_pipeline_layout,
            0,
            &[self.data.descriptor_sets[image_index]],
            &[],
        );

        for chunk in visible_chunks {
            debug!("Rendering {:?}", chunk.world_pos);
            // Per-chunk world position as a push constant
            let push = ChunkPushConstants {
                world_pos: [
                    chunk.world_pos.0 as f32 * CHUNK_SIZE as f32,
                    chunk.world_pos.1 as f32 * CHUNK_SIZE as f32,
                    chunk.world_pos.2 as f32 * CHUNK_SIZE as f32,
                ],
                _padding: 0.,
            };
            self.device.cmd_push_constants(
                command_buffer,
                self.data.voxel_pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytes_of(&push),
            );

            self.device
                .cmd_bind_vertex_buffers(command_buffer, 0, &[chunk.vertex_buffer], &[0]);
            self.device.cmd_bind_index_buffer(
                command_buffer,
                chunk.index_buffer,
                0,
                vk::IndexType::UINT32,
            );
            self.device
                .cmd_draw_indexed(command_buffer, chunk.index_count, 1, 0, 0, 0);
        }

        return Ok(());
    }
}
