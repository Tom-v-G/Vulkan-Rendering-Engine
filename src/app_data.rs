use crate::vertex::Vertex;
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk;

#[derive(Clone, Debug, Default)]
pub struct DeletionQueue {
    pub buffers: Vec<vk::Buffer>,
    pub buffer_memories: Vec<vk::DeviceMemory>,
}

impl DeletionQueue {
    pub unsafe fn push(&mut self, buffer: vk::Buffer, buffer_memory: vk::DeviceMemory) {
        self.buffers.push(buffer);
        self.buffer_memories.push(buffer_memory);
    }

    pub unsafe fn flush(&mut self, device: &Device) {
        for buffer in self.buffers.drain(..) {
            device.destroy_buffer(buffer, None);
        }
        for buffer_memory in self.buffer_memories.drain(..) {
            device.free_memory(buffer_memory, None);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameData {
    pub in_flight_fence: vk::Fence,
    pub image_available_semaphore: vk::Semaphore,
    pub render_finished_semaphore: vk::Semaphore,
    pub deletion_queue: DeletionQueue,
}

impl FrameData {
    pub unsafe fn destroy(&mut self, device: &Device) {
        device.destroy_fence(self.in_flight_fence, None);
        device.destroy_semaphore(self.image_available_semaphore, None);
        device.destroy_semaphore(self.render_finished_semaphore, None);
        self.deletion_queue.flush(device);
    }
}

/// The Vulkan handles and associated properties used by our Vulkan app.
#[derive(Clone, Debug, Default)]
pub struct AppData {
    pub surface: vk::SurfaceKHR,
    pub messenger: vk::DebugUtilsMessengerEXT,
    pub physical_device: vk::PhysicalDevice,
    pub msaa_samples: vk::SampleCountFlags,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub swapchain_format: vk::Format,
    pub swapchain_extent: vk::Extent2D,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_images: Vec<vk::Image>,
    pub swapchain_image_views: Vec<vk::ImageView>,
    pub render_pass: vk::RenderPass,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub gui_framebuffers: Vec<vk::Framebuffer>,
    pub command_pool: vk::CommandPool,
    pub command_pools: Vec<vk::CommandPool>,
    pub command_buffers: Vec<vk::CommandBuffer>,
    pub secondary_command_buffers: Vec<Vec<vk::CommandBuffer>>,
    pub frames: Vec<FrameData>,
    pub images_in_flight: Vec<vk::Fence>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub vertex_buffer: vk::Buffer,
    pub vertex_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
    pub uniform_buffers: Vec<vk::Buffer>,
    pub uniform_buffers_memory: Vec<vk::DeviceMemory>,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub mip_levels: u32,
    pub texture_image: vk::Image,
    pub texture_image_memory: vk::DeviceMemory,
    pub texture_image_view: vk::ImageView,
    pub texture_sampler: vk::Sampler,
    pub color_image: vk::Image,
    pub color_image_memory: vk::DeviceMemory,
    pub color_image_view: vk::ImageView,
    pub depth_image: vk::Image,
    pub depth_image_memory: vk::DeviceMemory,
    pub depth_image_view: vk::ImageView,
    pub voxel_pipeline_layout: vk::PipelineLayout,
    pub voxel_pipeline: vk::Pipeline,
}
