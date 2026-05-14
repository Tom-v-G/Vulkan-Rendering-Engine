use crate::app_data::{AppData, DeletionQueue, FrameData};
use crate::constants::*;
use crate::device::QueueFamilyIndices;
use crate::utils::*;

use vulkanalia::vk;

pub unsafe fn create_command_pool(
    instance: &Instance,
    device: &Device,
    data: &mut AppData,
) -> Result<vk::CommandPool> {
    let indices = QueueFamilyIndices::get(instance, data, data.physical_device)?;

    let info = vk::CommandPoolCreateInfo::builder()
        .flags(vk::CommandPoolCreateFlags::TRANSIENT)
        .queue_family_index(indices.graphics);

    Ok(device.create_command_pool(&info, None)?)
}

pub unsafe fn create_command_pools(
    instance: &Instance,
    device: &Device,
    data: &mut AppData,
) -> Result<()> {
    data.command_pool = create_command_pool(instance, device, data)?;

    let num_images = data.swapchain_images.len();
    for _ in 0..num_images {
        let command_pool = create_command_pool(instance, device, data)?;
        data.command_pools.push(command_pool);
    }
    Ok(())
}

pub unsafe fn create_command_buffers(device: &Device, data: &mut AppData) -> Result<()> {
    let num_images = data.swapchain_images.len();
    for image_index in 0..num_images {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(data.command_pools[image_index])
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1); // data.framebuffers.len() as u32

        let command_buffer = device.allocate_command_buffers(&allocate_info)?[0];

        data.command_buffers.push(command_buffer);
    }
    data.secondary_command_buffers = vec![vec![]; data.swapchain_images.len()];

    Ok(())
}

pub unsafe fn create_sync_objects(device: &Device, data: &mut AppData) -> Result<()> {
    let semaphore_info = vk::SemaphoreCreateInfo::builder();
    let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

    for _ in 0..MAX_FRAMES_IN_FLIGHT {
        let framedata = FrameData {
            in_flight_fence: device.create_fence(&fence_info, None)?,
            image_available_semaphore: device.create_semaphore(&semaphore_info, None)?,
            render_finished_semaphore: device.create_semaphore(&semaphore_info, None)?,
            deletion_queue: DeletionQueue::default(),
        };

        data.frames.push(framedata);
    }

    data.images_in_flight = data
        .swapchain_images
        .iter()
        .map(|_| vk::Fence::null())
        .collect();
    Ok(())
}

pub unsafe fn begin_single_time_commands(
    device: &Device,
    command_pool: vk::CommandPool,
) -> Result<vk::CommandBuffer> {
    let info = vk::CommandBufferAllocateInfo::builder()
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_pool(command_pool)
        .command_buffer_count(1);

    let command_buffer = device.allocate_command_buffers(&info)?[0];

    let info =
        vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    device.begin_command_buffer(command_buffer, &info)?;

    Ok(command_buffer)
}

pub unsafe fn end_single_time_commands(
    device: &Device,
    graphics_queue: vk::Queue,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
) -> Result<()> {
    device.end_command_buffer(command_buffer)?;

    let command_buffers = &[command_buffer];
    let info = vk::SubmitInfo::builder().command_buffers(command_buffers);

    device.queue_submit(graphics_queue, &[info], vk::Fence::null())?;
    device.queue_wait_idle(graphics_queue)?;

    device.free_command_buffers(command_pool, &[command_buffer]);

    Ok(())
}
