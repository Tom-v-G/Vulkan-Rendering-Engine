// Everything to do with rendering chunks to screen. Contains binding- and attribute descriptions of the Voxel Vertices
// pipeline construction, push constants and GpuChunk structs.

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use vulkanalia::prelude::v1_0::*;

use crate::{
    app_data::AppData,
    buffers::{create_index_buffer, create_vertex_buffer},
    chunk::ChunkCoord,
    chunkmesher::{MeshData, VoxelVertex},
    utils::create_shader_module,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ChunkPushConstants {
    pub world_pos: [f32; 3],
}

pub struct GpuChunk {
    pub vertex_buffer: vk::Buffer,
    pub vertex_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
    pub index_count: u32,
    pub world_pos: (i32, i32, i32),
}

impl GpuChunk {
    pub unsafe fn new(
        instance: &Instance,
        device: &Device,
        physical_device: vk::PhysicalDevice,
        graphics_queue: vk::Queue,
        command_pool: vk::CommandPool,
        meshdata: &MeshData,
        world_pos: ChunkCoord,
    ) -> Result<Self> {
        let (vertex_buffer, vertex_buffer_memory) = create_vertex_buffer(
            instance,
            device,
            physical_device,
            &meshdata.vertices,
            graphics_queue,
            command_pool,
        )?;

        let (index_buffer, index_buffer_memory) = create_index_buffer(
            instance,
            device,
            physical_device,
            &meshdata.indices,
            graphics_queue,
            command_pool,
        )?;

        let index_count = meshdata.indices.len() as u32;
        return Ok(Self {
            vertex_buffer,
            vertex_buffer_memory,
            index_buffer,
            index_buffer_memory,
            index_count,
            world_pos,
        });
    }

    pub unsafe fn destroy(&mut self, instance: &Instance, device: &Device) {
        device.destroy_buffer(self.vertex_buffer, None);
        device.free_memory(self.vertex_buffer_memory, None);
        device.destroy_buffer(self.index_buffer, None);
        device.free_memory(self.index_buffer_memory, None);
    }
}

impl VoxelVertex {
    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(size_of::<VoxelVertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }

    pub fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
        let pos = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT) //Vec2 f32 format: for some reason denoted with RG(not B) values
            .offset(0)
            .build();
        let normal = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R8_SINT)
            .offset(12)
            .build();
        let color = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(vk::Format::R8G8B8A8_UINT) // alpha is ignored atm
            .offset(13)
            .build();
        [pos, normal, color]
    }
}

pub unsafe fn create_chunk_pipeline(device: &Device, data: &mut AppData) -> Result<()> {
    let vert = include_bytes!("../shaders/voxel/vert.spv");
    let frag = include_bytes!("../shaders/voxel/frag.spv");

    let vert_shader_module = create_shader_module(device, &vert[..])?;
    let frag_shader_module = create_shader_module(device, &frag[..])?;

    let vert_stage = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vert_shader_module)
        .name(b"main\0");

    let frag_stage = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(frag_shader_module)
        .name(b"main\0");

    let binding_descriptions = &[VoxelVertex::binding_description()];
    let attribute_descriptions = VoxelVertex::attribute_descriptions();
    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(binding_descriptions)
        .vertex_attribute_descriptions(&attribute_descriptions);

    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);

    let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS)
        .depth_bounds_test_enable(false)
        .min_depth_bounds(0.0) // Optional.
        .max_depth_bounds(1.0); // Optional.
                                // .stencil_test_enable(false) // if you want to enable stencil tests
                                // .front(/* vk::StencilOpState */) // Optional.
                                // .back(/* vk::StencilOpState */); // Optional.

    let viewport = vk::Viewport::builder()
        .x(0.0)
        .y(0.0)
        .width(data.swapchain_extent.width as f32)
        .height(data.swapchain_extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0);

    let scissor = vk::Rect2D::builder()
        .offset(vk::Offset2D { x: 0, y: 0 })
        .extent(data.swapchain_extent);

    let viewports = &[viewport];
    let scissors = &[scissor];

    let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
        .viewports(viewports)
        .scissors(scissors);

    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(false);

    // No textures -> No MSAA
    // Still need to define it due to the render pass expecting it
    let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlags::_8)
        .min_sample_shading(1.0);

    let attachment = vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(vk::ColorComponentFlags::all())
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA) // Optional
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA) // Optional
        .color_blend_op(vk::BlendOp::ADD) // Optional
        .src_alpha_blend_factor(vk::BlendFactor::ONE) // Optional
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO) // Optional
        .alpha_blend_op(vk::BlendOp::ADD); // Optional

    let attachments = &[attachment];
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);

    // TODO: check push constant range
    let vert_push_constant_range = vk::PushConstantRange::builder()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(64 /* 16 × 4 byte floats */);

    let frag_push_constant_range = vk::PushConstantRange::builder()
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        .offset(64)
        .size(4);

    let set_layouts = &[data.descriptor_set_layout];
    let push_constant_ranges = &[vert_push_constant_range, frag_push_constant_range];
    let layout_info = vk::PipelineLayoutCreateInfo::builder()
        .set_layouts(set_layouts)
        .push_constant_ranges(push_constant_ranges);

    data.voxel_pipeline_layout = device.create_pipeline_layout(&layout_info, None)?;

    let stages = &[vert_stage, frag_stage];
    let info = vk::GraphicsPipelineCreateInfo::builder()
        .stages(stages)
        .vertex_input_state(&vertex_input_state)
        .input_assembly_state(&input_assembly_state)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization_state)
        .multisample_state(&multisample_state)
        .depth_stencil_state(&depth_stencil_state)
        .color_blend_state(&color_blend_state)
        .layout(data.voxel_pipeline_layout)
        .render_pass(data.render_pass)
        .subpass(0)
        .base_pipeline_handle(vk::Pipeline::null()) // Optional.
        .base_pipeline_index(-1); // Optional.

    let voxel_pipeline = device
        .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)?
        .0[0];

    data.voxel_pipeline = voxel_pipeline;

    device.destroy_shader_module(vert_shader_module, None);
    device.destroy_shader_module(frag_shader_module, None);

    Ok(())
}
