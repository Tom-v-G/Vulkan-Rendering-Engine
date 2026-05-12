use anyhow::Result;
use bytemuck::Pod;

use vulkanalia::prelude::v1_0::*;

use crate::{
    app_data::AppData,
    commands::{begin_single_time_commands, end_single_time_commands},
    utils::get_memory_type_index,
};

use glam::Mat4;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct UniformBufferObject {
    pub view: Mat4,
    pub proj: Mat4,
}

pub unsafe fn create_buffer(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    let buffer_info = vk::BufferCreateInfo::builder()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .flags(vk::BufferCreateFlags::empty()); // Optional.
    let buffer = device.create_buffer(&buffer_info, None)?;

    let requirements = device.get_buffer_memory_requirements(buffer);

    let memory_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(requirements.size)
        .memory_type_index(get_memory_type_index(
            instance,
            physical_device,
            properties,
            requirements,
        )?);

    let buffer_memory = device.allocate_memory(&memory_info, None)?;

    device.bind_buffer_memory(buffer, buffer_memory, 0)?;

    Ok((buffer, buffer_memory))
}

pub unsafe fn copy_buffer(
    device: &Device,
    graphics_queue: vk::Queue,
    command_pool: vk::CommandPool,
    source: vk::Buffer,
    destination: vk::Buffer,
    size: vk::DeviceSize,
) -> Result<()> {
    let command_buffer = begin_single_time_commands(device, command_pool)?;

    let regions = vk::BufferCopy::builder().size(size);
    device.cmd_copy_buffer(command_buffer, source, destination, &[regions]);

    end_single_time_commands(device, graphics_queue, command_pool, command_buffer)?;

    Ok(())
}

pub unsafe fn create_uniform_buffers(
    instance: &Instance,
    device: &Device,
    data: &mut AppData,
) -> Result<()> {
    data.uniform_buffers.clear();
    data.uniform_buffers_memory.clear();

    for _ in 0..data.swapchain_images.len() {
        let (uniform_buffer, uniform_buffer_memory) = create_buffer(
            instance,
            device,
            data.physical_device,
            size_of::<UniformBufferObject>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;

        data.uniform_buffers.push(uniform_buffer);
        data.uniform_buffers_memory.push(uniform_buffer_memory);
    }

    Ok(())
}

pub unsafe fn create_vertex_buffer<V: Pod>(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    vertices: &[V],
    graphics_queue: vk::Queue,
    command_pool: vk::CommandPool,
) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    let size = (size_of::<V>() * vertices.len()) as u64;

    let (staging_buffer, staging_buffer_memory) = create_buffer(
        instance,
        device,
        physical_device,
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
    )?;

    let dst = device.map_memory(staging_buffer_memory, 0, size, vk::MemoryMapFlags::empty())?;

    let bytes: &[u8] = bytemuck::cast_slice(vertices);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.cast(), bytes.len());
    device.unmap_memory(staging_buffer_memory);

    let (vertex_buffer, vertex_buffer_memory) = create_buffer(
        instance,
        device,
        physical_device,
        size,
        vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;

    copy_buffer(
        device,
        graphics_queue,
        command_pool,
        staging_buffer,
        vertex_buffer,
        size,
    )?;

    device.destroy_buffer(staging_buffer, None);
    device.free_memory(staging_buffer_memory, None);

    Ok((vertex_buffer, vertex_buffer_memory))
}

pub unsafe fn create_index_buffer<I: Pod>(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    indices: &[I],
    graphics_queue: vk::Queue,
    command_pool: vk::CommandPool,
) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    let size = (size_of::<I>() * indices.len()) as u64;

    let (staging_buffer, staging_buffer_memory) = create_buffer(
        instance,
        device,
        physical_device,
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE,
    )?;

    let dst = device.map_memory(staging_buffer_memory, 0, size, vk::MemoryMapFlags::empty())?;

    let bytes: &[u8] = bytemuck::cast_slice(indices);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.cast(), bytes.len());

    device.unmap_memory(staging_buffer_memory);

    let (index_buffer, index_buffer_memory) = create_buffer(
        instance,
        device,
        physical_device,
        size,
        vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;

    copy_buffer(
        device,
        graphics_queue,
        command_pool,
        staging_buffer,
        index_buffer,
        size,
    )?;

    device.destroy_buffer(staging_buffer, None);
    device.free_memory(staging_buffer_memory, None);

    Ok((index_buffer, index_buffer_memory))
}
