// all (e)GUI related code
use anyhow::{anyhow, Result};
use egui_winit::winit::window::Window;
use vulkanalia::prelude::v1_0::*;
// use winit::window::Window;

use std::mem::size_of;
use std::ptr::copy_nonoverlapping as memcpy;

use crate::app_data::AppData;
use crate::buffers::create_buffer;
use crate::commands::{begin_single_time_commands, end_single_time_commands};
use crate::images::{
    copy_buffer_to_image, create_image, create_image_view, transition_image_layout,
};
use crate::utils::create_shader_module;

// Matches the push constant block in the egui vertex shader:
// layout(push_constant) uniform PushConstants {
//     vec2 screen_size;
// } push_constants;
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct EguiPushConstants {
    screen_size: [f32; 2],
}

// Per-frame CPU-side buffers — rebuilt every frame from egui's mesh output
struct FrameBuffers {
    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
    vertex_count: usize,
    index_buffer: vk::Buffer,
    index_buffer_memory: vk::DeviceMemory,
    index_count: usize,
}

pub struct Gui {
    pub egui_ctx: egui::Context,
    pub egui_winit: egui_winit::State,

    // Render pass — separate from the scene pass, uses LOAD to composite on top
    pub render_pass: vk::RenderPass,

    // Pipeline
    descriptor_set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,

    // Font atlas texture (rebuilt when egui signals textures_delta)
    font_image: vk::Image,
    font_image_memory: vk::DeviceMemory,
    pub font_image_view: vk::ImageView,
    font_sampler: vk::Sampler,

    // One descriptor set per swapchain image
    descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,

    // Per-frame vertex + index buffers (one per swapchain image)
    frame_buffers: Vec<FrameBuffers>,
}

impl Gui {
    pub unsafe fn create(
        window: &Window,
        instance: &Instance,
        device: &Device,
        data: &AppData, // read-only: physical_device, swapchain_format, etc.
        swapchain_image_count: usize,
    ) -> Result<Self> {
        // ------------------------------------------------------------------ //
        // 1. egui context + winit state
        // ------------------------------------------------------------------ //
        let egui_ctx = egui::Context::default();
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            None, // native pixels per point — let egui detect it
            None,
            None, // max texture side — use egui default
        );

        // ------------------------------------------------------------------ //
        // 2. Render pass
        //    Single color attachment. LOAD_OP::LOAD so the scene underneath
        //    is preserved. Final layout is PRESENT_SRC_KHR since egui renders
        //    last before presentation.
        // ------------------------------------------------------------------ //
        let color_attachment = vk::AttachmentDescription::builder()
            .format(data.swapchain_format)
            .samples(vk::SampleCountFlags::_1) // egui renders to resolved image, not MSAA
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::PRESENT_SRC_KHR) // written by scene pass
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let color_attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let color_attachments = &[color_attachment_ref];
        let subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(color_attachments);

        // Wait for the scene pass color output before writing
        let dependency = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

        let attachments = &[color_attachment];
        let subpasses = &[subpass];
        let dependencies = &[dependency];
        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(attachments)
            .subpasses(subpasses)
            .dependencies(dependencies);

        let render_pass = device.create_render_pass(&render_pass_info, None)?;

        // ------------------------------------------------------------------ //
        // 3. Descriptor set layout
        //    Single binding: combined image sampler for the egui font atlas.
        // ------------------------------------------------------------------ //
        let sampler_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);

        let bindings = &[sampler_binding];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(bindings);

        let descriptor_set_layout = device.create_descriptor_set_layout(&layout_info, None)?;

        // ------------------------------------------------------------------ //
        // 4. Pipeline layout
        //    Push constant carries the screen size in pixels so the vertex
        //    shader can convert egui's logical coords to NDC.
        // ------------------------------------------------------------------ //
        let push_constant_range = vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(size_of::<EguiPushConstants>() as u32); // 8 bytes: vec2

        let set_layouts = &[descriptor_set_layout];
        let push_constant_ranges = &[push_constant_range];
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(set_layouts)
            .push_constant_ranges(push_constant_ranges);

        let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None)?;

        // ------------------------------------------------------------------ //
        // 5. Graphics pipeline
        //    Key differences from your scene pipeline:
        //      - No depth test or write (GUI draws on top unconditionally)
        //      - No MSAA (egui targets the resolved 1-sample swapchain image)
        //      - No back-face culling (egui meshes are flat quads)
        //      - Pre-multiplied alpha blending (egui convention)
        //      - Dynamic viewport/scissor (clip rects change every frame)
        // ------------------------------------------------------------------ //

        // egui ships SPIR-V shaders via the egui_wgpu crate, but for vulkanalia
        // you need to compile your own. Place these in shaders/egui_vert.spv
        // and shaders/egui_frag.spv — see the note after this function for the
        // GLSL source.
        let vert = include_bytes!("../shaders/egui/egui_vert.spv");
        let frag = include_bytes!("../shaders/egui/egui_frag.spv");

        let vert_shader_module = create_shader_module(device, vert)?;
        let frag_shader_module = create_shader_module(device, frag)?;

        let vert_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_shader_module)
            .name(b"main\0");

        let frag_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_shader_module)
            .name(b"main\0");

        // egui vertex layout: pos (vec2 f32), uv (vec2 f32), color (u32 rgba)
        // Total: 20 bytes per vertex
        let binding_description = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(20) // 4+4+4+4+4 = 20 bytes
            .input_rate(vk::VertexInputRate::VERTEX)
            .build();

        let pos_attr = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT) // vec2
            .offset(0)
            .build();

        let uv_attr = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT) // vec2
            .offset(8)
            .build();

        let color_attr = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(vk::Format::R8G8B8A8_UNORM) // packed u32 as 4× unorm bytes
            .offset(16)
            .build();

        let binding_descriptions = &[binding_description];
        let attribute_descriptions = &[pos_attr, uv_attr, color_attr];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(binding_descriptions)
            .vertex_attribute_descriptions(attribute_descriptions);

        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        // Dynamic viewport and scissor — updated per draw call for clip rects
        let dynamic_states = &[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(dynamic_states);

        // Viewport and scissor count must still be declared even when dynamic
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            .scissor_count(1);

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE) // egui quads are not consistently wound
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::_1); // no MSAA for GUI

        // No depth test — egui draws on top of everything
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(false)
            .depth_write_enable(false)
            .depth_compare_op(vk::CompareOp::ALWAYS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        // Pre-multiplied alpha: egui outputs colors with alpha pre-multiplied
        let attachment = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::all())
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE) // already multiplied
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .build();

        let attachments = &[attachment];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(attachments);

        let stages = &[vert_stage, frag_stage];
        let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(stages)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0)
            .base_pipeline_handle(vk::Pipeline::null())
            .base_pipeline_index(-1);

        let pipeline = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)?
            .0[0];

        device.destroy_shader_module(vert_shader_module, None);
        device.destroy_shader_module(frag_shader_module, None);

        // ------------------------------------------------------------------ //
        // 6. Font atlas texture
        //    Start with a 1×1 placeholder. The real atlas is uploaded on the
        //    first call to end_frame_and_render via textures_delta.
        // ------------------------------------------------------------------ //
        let (font_image, font_image_memory) = create_image(
            instance,
            device,
            data.physical_device,
            1,
            1, // placeholder size
            1, // mip levels
            vk::SampleCountFlags::_1,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let font_image_view = create_image_view(
            device,
            font_image,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageAspectFlags::COLOR,
            1,
        )?;

        println!("Transitioning Image layout");
        // Transition placeholder to SHADER_READ_ONLY so it is valid to bind
        // before the first real atlas upload
        transition_image_layout(
            device,
            data.graphics_queue,
            data.command_pool,
            font_image,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            1,
        )?;
        println!("Done");

        // Linear filter for smooth font rendering
        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .anisotropy_enable(false)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR);

        let font_sampler = device.create_sampler(&sampler_info, None)?;

        // ------------------------------------------------------------------ //
        // 7. Descriptor pool + sets (one per swapchain image)
        // ------------------------------------------------------------------ //
        let sampler_pool_size = vk::DescriptorPoolSize::builder()
            .type_(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(swapchain_image_count as u32);

        let pool_sizes = &[sampler_pool_size];
        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(pool_sizes)
            .max_sets(swapchain_image_count as u32);

        let descriptor_pool = device.create_descriptor_pool(&pool_info, None)?;

        let layouts = vec![descriptor_set_layout; swapchain_image_count];
        let alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&layouts);

        let descriptor_sets = device.allocate_descriptor_sets(&alloc_info)?;

        // Point every descriptor set at the font atlas. They all share the
        // same sampler and image view — updated together in upload_font_texture.
        for &set in &descriptor_sets {
            let image_info = vk::DescriptorImageInfo::builder()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(font_image_view)
                .sampler(font_sampler)
                .build();

            let image_infos = &[image_info];
            let write = vk::WriteDescriptorSet::builder()
                .dst_set(set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(image_infos);

            device.update_descriptor_sets(&[write], &[] as &[vk::CopyDescriptorSet]);
        }

        // ------------------------------------------------------------------ //
        // 8. Per-frame vertex + index buffers
        //    Start empty (zero-size buffers are invalid in Vulkan, so we
        //    allocate a small minimum). Resized lazily in end_frame_and_render.
        // ------------------------------------------------------------------ //
        let min_size = 1024u64; // 1 KB initial allocation
        let mut frame_buffers = Vec::with_capacity(swapchain_image_count);

        for _ in 0..swapchain_image_count {
            let (vertex_buffer, vertex_buffer_memory) = create_buffer(
                instance,
                device,
                data.physical_device,
                min_size,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;

            let (index_buffer, index_buffer_memory) = create_buffer(
                instance,
                device,
                data.physical_device,
                min_size,
                vk::BufferUsageFlags::INDEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;

            frame_buffers.push(FrameBuffers {
                vertex_buffer,
                vertex_buffer_memory,
                vertex_count: 0,
                index_buffer,
                index_buffer_memory,
                index_count: 0,
            });
        }

        Ok(Self {
            egui_ctx,
            egui_winit,
            render_pass,
            descriptor_set_layout,
            pipeline_layout,
            pipeline,
            font_image,
            font_image_memory,
            font_image_view,
            font_sampler,
            descriptor_pool,
            descriptor_sets,
            frame_buffers,
        })
    }

    // ------------------------------------------------------------------ //
    // begin_frame
    // Call this at the start of your render loop, before building UI.
    // Returns the egui Context so the caller can construct widgets.
    // ------------------------------------------------------------------ //
    pub fn begin_frame(&mut self, window: &Window) -> &egui::Context {
        let raw_input = self.egui_winit.take_egui_input(window);
        self.egui_ctx.begin_pass(raw_input);
        &self.egui_ctx
    }

    // ------------------------------------------------------------------ //
    // end_frame_and_render
    // Call this after all UI has been built, passing the command buffer
    // that is currently being recorded for this frame.
    // ------------------------------------------------------------------ //
    pub unsafe fn end_frame_and_render(
        &mut self,
        window: &Window,
        instance: &Instance,
        device: &Device,
        data: &AppData,
        command_buffer: vk::CommandBuffer,
        framebuffer: vk::Framebuffer, // the egui framebuffer for this image index
        image_index: usize,
        swapchain_extent: vk::Extent2D,
    ) -> Result<()> {
        // ---- 1. End the egui frame, get paint jobs and texture updates ---- //

        let full_output = self.egui_ctx.end_pass();

        // Forward platform output (clipboard, cursor changes, etc.) to winit
        self.egui_winit
            .handle_platform_output(window, full_output.platform_output);

        // Tessellate shapes into textured meshes
        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        // ---- 2. Upload font atlas if egui signals a texture update ---- //

        // textures_delta.set contains (id, ImageDelta) pairs for new or
        // updated textures. For the default font atlas there is only ever
        // one texture (TextureId::default()), but egui supports user textures
        // too so we iterate all of them.
        for (id, delta) in &full_output.textures_delta.set {
            // We only handle the managed font atlas for now.
            // User textures (id != TextureId::default()) would need a
            // separate HashMap<TextureId, GpuTexture> — add that when needed.
            if *id != egui::TextureId::default() {
                continue;
            }
            self.upload_font_texture(instance, device, data, delta)?;
        }

        // textures_delta.free lists textures egui no longer needs.
        // For the default atlas this never fires, but handle it for correctness.
        for id in &full_output.textures_delta.free {
            if *id == egui::TextureId::default() {
                // Would destroy and recreate font_image here if needed.
                // In practice egui only grows the atlas, never frees it mid-run.
            }
        }

        // ---- 3. Upload vertex + index data for this frame ---- //

        // Collect all vertices and indices from every paint job into two
        // flat buffers. Track per-job offsets for the draw loop below.
        struct JobOffsets {
            vertex_offset: i32, // signed — Vulkan cmd_draw_indexed takes i32
            index_offset: u32,
            index_count: u32,
            clip_rect: egui::Rect,
        }

        let mut all_vertices: Vec<egui::epaint::Vertex> = Vec::new();
        let mut all_indices: Vec<u32> = Vec::new();
        let mut job_offsets: Vec<JobOffsets> = Vec::new();

        for job in &paint_jobs {
            let egui::epaint::Primitive::Mesh(mesh) = &job.primitive else {
                continue;
                // skip egui::epaint::Primitive::Callback (not used here)
                // Use this if I want to add 3D viewports later.
            };

            job_offsets.push(JobOffsets {
                vertex_offset: all_vertices.len() as i32,
                index_offset: all_indices.len() as u32,
                index_count: mesh.indices.len() as u32,
                clip_rect: job.clip_rect,
            });

            all_vertices.extend_from_slice(&mesh.vertices);
            all_indices.extend_from_slice(&mesh.indices);
        }

        // Nothing to draw this frame — still need to run the render pass
        // so the swapchain image transitions correctly, but skip buffer uploads.
        let has_geometry = !all_vertices.is_empty();

        if has_geometry {
            let vertex_size = (size_of::<egui::epaint::Vertex>() * all_vertices.len()) as u64;
            let index_size = (size_of::<u32>() * all_indices.len()) as u64;

            let fb = &mut self.frame_buffers[image_index];

            // Reallocate vertex buffer if the current one is too small.
            // We never shrink — only grow — to avoid thrashing the allocator.
            if vertex_size > Self::buffer_size(device, fb.vertex_buffer) {
                device.destroy_buffer(fb.vertex_buffer, None);
                device.free_memory(fb.vertex_buffer_memory, None);

                let (buf, mem) = create_buffer(
                    instance,
                    device,
                    data.physical_device,
                    vertex_size,
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                )?;
                fb.vertex_buffer = buf;
                fb.vertex_buffer_memory = mem;
            }

            // Same for the index buffer.
            if index_size > Self::buffer_size(device, fb.index_buffer) {
                device.destroy_buffer(fb.index_buffer, None);
                device.free_memory(fb.index_buffer_memory, None);

                let (buf, mem) = create_buffer(
                    instance,
                    device,
                    data.physical_device,
                    index_size,
                    vk::BufferUsageFlags::INDEX_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                )?;
                fb.index_buffer = buf;
                fb.index_buffer_memory = mem;
            }

            // Write vertices
            let vertex_dst = device.map_memory(
                fb.vertex_buffer_memory,
                0,
                vertex_size,
                vk::MemoryMapFlags::empty(),
            )?;
            memcpy(all_vertices.as_ptr(), vertex_dst.cast(), all_vertices.len());
            device.unmap_memory(fb.vertex_buffer_memory);

            // Write indices
            let index_dst = device.map_memory(
                fb.index_buffer_memory,
                0,
                index_size,
                vk::MemoryMapFlags::empty(),
            )?;
            memcpy(all_indices.as_ptr(), index_dst.cast(), all_indices.len());
            device.unmap_memory(fb.index_buffer_memory);

            fb.vertex_count = all_vertices.len();
            fb.index_count = all_indices.len();
        }

        // ---- 4. Record render pass ---- //

        let render_area = vk::Rect2D::builder()
            .offset(vk::Offset2D::default())
            .extent(swapchain_extent);

        // No clear values — we use LOAD_OP::LOAD to preserve the scene
        let render_pass_info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.render_pass)
            .framebuffer(framebuffer)
            .render_area(render_area)
            .clear_values(&[]);

        device.cmd_begin_render_pass(
            command_buffer,
            &render_pass_info,
            vk::SubpassContents::INLINE, // egui commands go directly, no secondary buffers
        );

        if has_geometry {
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            // Set dynamic viewport — full swapchain extent
            let viewport = vk::Viewport::builder()
                .x(0.0)
                .y(0.0)
                .width(swapchain_extent.width as f32)
                .height(swapchain_extent.height as f32)
                .min_depth(0.0)
                .max_depth(1.0);
            device.cmd_set_viewport(command_buffer, 0, &[viewport]);

            // Bind combined vertex + index buffer for this frame
            let fb = &self.frame_buffers[image_index];
            device.cmd_bind_vertex_buffers(command_buffer, 0, &[fb.vertex_buffer], &[0]);
            device.cmd_bind_index_buffer(command_buffer, fb.index_buffer, 0, vk::IndexType::UINT32);

            // Push screen size in logical pixels
            let screen_size = EguiPushConstants {
                screen_size: [
                    swapchain_extent.width as f32 / full_output.pixels_per_point,
                    swapchain_extent.height as f32 / full_output.pixels_per_point,
                ],
            };
            let screen_size_bytes = std::slice::from_raw_parts(
                &screen_size as *const EguiPushConstants as *const u8,
                size_of::<EguiPushConstants>(),
            );
            device.cmd_push_constants(
                command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                screen_size_bytes,
            );

            // Bind the font atlas descriptor set (same for all draw calls)
            device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.descriptor_sets[image_index]],
                &[],
            );

            // One draw call per paint job, with a scissor rect for clipping
            let pixels_per_point = full_output.pixels_per_point;
            for offsets in &job_offsets {
                // Convert egui logical clip rect to physical pixels, clamped
                // to the swapchain extent so the scissor rect is always valid
                let min_x = (offsets.clip_rect.min.x * pixels_per_point).round() as i32;
                let min_y = (offsets.clip_rect.min.y * pixels_per_point).round() as i32;
                let max_x = (offsets.clip_rect.max.x * pixels_per_point).round() as i32;
                let max_y = (offsets.clip_rect.max.y * pixels_per_point).round() as i32;

                // Clamp to [0, swapchain extent]
                let scissor_x = min_x.max(0) as u32;
                let scissor_y = min_y.max(0) as u32;
                let scissor_w = (max_x.min(swapchain_extent.width as i32) - min_x).max(0) as u32;
                let scissor_h = (max_y.min(swapchain_extent.height as i32) - min_y).max(0) as u32;

                // Skip draw calls with a zero-area scissor rect
                if scissor_w == 0 || scissor_h == 0 {
                    continue;
                }

                let scissor = vk::Rect2D::builder()
                    .offset(vk::Offset2D {
                        x: scissor_x as i32,
                        y: scissor_y as i32,
                    })
                    .extent(vk::Extent2D {
                        width: scissor_w,
                        height: scissor_h,
                    });
                device.cmd_set_scissor(command_buffer, 0, &[scissor]);

                device.cmd_draw_indexed(
                    command_buffer,
                    offsets.index_count,
                    1,                     // instance count
                    offsets.index_offset,  // first index
                    offsets.vertex_offset, // vertex offset
                    0,                     // first instance
                );
            }
        }

        device.cmd_end_render_pass(command_buffer);

        Ok(())
    }

    // ------------------------------------------------------------------ //
    // destroy
    // Call this in App::destroy, after device_wait_idle.
    // ------------------------------------------------------------------ //
    pub unsafe fn destroy(&mut self, device: &Device) {
        // Per-frame buffers
        for fb in &self.frame_buffers {
            device.destroy_buffer(fb.vertex_buffer, None);
            device.free_memory(fb.vertex_buffer_memory, None);
            device.destroy_buffer(fb.index_buffer, None);
            device.free_memory(fb.index_buffer_memory, None);
        }
        self.frame_buffers.clear();

        // Descriptor pool implicitly frees all descriptor sets
        device.destroy_descriptor_pool(self.descriptor_pool, None);

        // Font texture
        device.destroy_sampler(self.font_sampler, None);
        device.destroy_image_view(self.font_image_view, None);
        device.destroy_image(self.font_image, None);
        device.free_memory(self.font_image_memory, None);

        // Pipeline resources
        device.destroy_pipeline(self.pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
        device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);

        // Render pass
        device.destroy_render_pass(self.render_pass, None);
    }

    // ------------------------------------------------------------------ //
    // Private helpers
    // ------------------------------------------------------------------ //

    // Query the allocated size of an existing buffer via memory requirements.
    // Used to decide whether a buffer needs to be reallocated this frame.
    unsafe fn buffer_size(device: &Device, buffer: vk::Buffer) -> u64 {
        device.get_buffer_memory_requirements(buffer).size
    }

    // Upload a new or updated egui texture delta to the font atlas.
    // Called from end_frame_and_render when textures_delta.set is non-empty.
    unsafe fn upload_font_texture(
        &mut self,
        instance: &Instance,
        device: &Device,
        data: &AppData,
        delta: &egui::epaint::ImageDelta,
    ) -> Result<()> {
        // Extract raw RGBA bytes from the delta image
        let pixels: Vec<u8> = match &delta.image {
            egui::ImageData::Color(image) => {
                // Color image: each pixel is already Color32 (4 bytes RGBA)
                image.pixels.iter().flat_map(|c| c.to_array()).collect()
            } //
              //
              // Seems that ImageData no longer has an Alpha or Font attribute
              // egui::ImageData::Alpha(image) => {
              //     // Use egui's built-in srgba_pixels() rather than manual conversion.
              //     // It handles gamma correctly and outputs premultiplied sRGBA bytes,
              //     // which is exactly what the font texture upload expects.

              //     image
              //         .srgba_pixels(None)
              //         .flat_map(|c| c.to_array())
              //         .collect()
              // }
        };

        let width = delta.image.width() as u32;
        let height = delta.image.height() as u32;
        let size = pixels.len() as u64;

        // If the atlas grew (egui reallocated it), we need to recreate the
        // GPU image at the new size before uploading.
        let current_requirements = device.get_image_memory_requirements(self.font_image);

        let needed_size = (width * height * 4) as u64;

        if needed_size > current_requirements.size {
            // Destroy old resources
            device.destroy_image_view(self.font_image_view, None);
            device.destroy_image(self.font_image, None);
            device.free_memory(self.font_image_memory, None);

            // Recreate at new size
            let (new_image, new_memory) = create_image(
                instance,
                device,
                data.physical_device,
                width,
                height,
                1,
                vk::SampleCountFlags::_1,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;

            let new_view = create_image_view(
                device,
                new_image,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageAspectFlags::COLOR,
                1,
            )?;

            self.font_image = new_image;
            self.font_image_memory = new_memory;
            self.font_image_view = new_view;

            // Update all descriptor sets to point at the new image view
            for &set in &self.descriptor_sets {
                let image_info = vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(self.font_image_view)
                    .sampler(self.font_sampler)
                    .build();

                let image_infos = &[image_info];
                let write = vk::WriteDescriptorSet::builder()
                    .dst_set(set)
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(image_infos);

                device.update_descriptor_sets(&[write], &[] as &[vk::CopyDescriptorSet]);
            }
        }

        // Upload pixel data via staging buffer — same pattern as create_texture_image
        let (staging_buffer, staging_memory) = create_buffer(
            instance,
            device,
            data.physical_device,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let dst = device.map_memory(staging_memory, 0, size, vk::MemoryMapFlags::empty())?;
        memcpy(pixels.as_ptr(), dst.cast(), pixels.len());
        device.unmap_memory(staging_memory);

        // Transition → transfer destination → shader read
        transition_image_layout(
            device,
            data.graphics_queue,
            data.command_pool,
            self.font_image,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            1,
        )?;

        copy_buffer_to_image(
            device,
            data.graphics_queue,
            data.command_pool,
            staging_buffer,
            self.font_image,
            width,
            height,
        )?;

        transition_image_layout(
            device,
            data.graphics_queue,
            data.command_pool,
            self.font_image,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            1,
        )?;

        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_memory, None);

        Ok(())
    }
}
