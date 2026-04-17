// Pick physical device, create the logical device etc.

use anyhow::{anyhow, Result};
use std::collections::HashSet;
use thiserror::Error;

use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk;
use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::Entry;

use crate::app_data::AppData;
use crate::constants::*;
use crate::utils::*;

#[derive(Copy, Clone, Debug)]
pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
}

impl QueueFamilyIndices {
    pub unsafe fn get(
        instance: &Instance,
        data: &AppData,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self> {
        let properties = instance.get_physical_device_queue_family_properties(physical_device);

        let mut present = None;
        for (index, properties) in properties.iter().enumerate() {
            if instance.get_physical_device_surface_support_khr(
                physical_device,
                index as u32,
                data.surface,
            )? {
                present = Some(index as u32);
                break;
            }
        }

        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        if let (Some(graphics), Some(present)) = (graphics, present) {
            Ok(Self { graphics, present })
        } else {
            Err(anyhow!(SuitabilityError(
                "Missing required queue families."
            )))
        }
    }
}

#[derive(Clone, Debug)]
pub struct SwapchainSupport {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}

impl SwapchainSupport {
    pub unsafe fn get(
        instance: &Instance,
        data: &AppData,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self> {
        Ok(Self {
            capabilities: instance
                .get_physical_device_surface_capabilities_khr(physical_device, data.surface)?,
            formats: instance
                .get_physical_device_surface_formats_khr(physical_device, data.surface)?,
            present_modes: instance
                .get_physical_device_surface_present_modes_khr(physical_device, data.surface)?,
        })
    }
}

#[derive(Debug, Error)]
#[error("Missing {0}.")]
pub struct SuitabilityError(pub &'static str);

unsafe fn check_physical_device(
    instance: &Instance,
    data: &AppData,
    physical_device: vk::PhysicalDevice,
) -> Result<()> {
    let properties = instance.get_physical_device_properties(physical_device);
    if properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
        return Err(anyhow!(SuitabilityError(
            "Only discrete GPUs are supported."
        )));
    }

    let features = instance.get_physical_device_features(physical_device);
    if features.geometry_shader != vk::TRUE {
        return Err(anyhow!(SuitabilityError(
            "Missing geometry shader support."
        )));
    }
    if features.sampler_anisotropy != vk::TRUE {
        return Err(anyhow!(SuitabilityError("No sampler anisotropy.")));
    }
    QueueFamilyIndices::get(instance, data, physical_device)?;
    check_physical_device_extensions(instance, physical_device)?;

    let support = SwapchainSupport::get(instance, data, physical_device)?;
    if support.formats.is_empty() || support.present_modes.is_empty() {
        return Err(anyhow!(SuitabilityError("Insufficient swapchain support.")));
    }
    Ok(())
}

unsafe fn check_physical_device_extensions(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<()> {
    let extensions = instance
        .enumerate_device_extension_properties(physical_device, None)?
        .iter()
        .map(|e| e.extension_name)
        .collect::<HashSet<_>>();
    if DEVICE_EXTENSIONS.iter().all(|e| extensions.contains(e)) {
        Ok(())
    } else {
        Err(anyhow!(SuitabilityError(
            "Missing required device extensions."
        )))
    }
}

unsafe fn get_max_msaa_samples(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> vk::SampleCountFlags {
    let properties = instance.get_physical_device_properties(physical_device);
    let counts = properties.limits.framebuffer_color_sample_counts
        & properties.limits.framebuffer_depth_sample_counts;
    [
        vk::SampleCountFlags::_64,
        vk::SampleCountFlags::_32,
        vk::SampleCountFlags::_16,
        vk::SampleCountFlags::_8,
        vk::SampleCountFlags::_4,
        vk::SampleCountFlags::_2,
    ]
    .iter()
    .cloned()
    .find(|c| counts.contains(*c))
    .unwrap_or(vk::SampleCountFlags::_1)
}

pub unsafe fn pick_physical_device(instance: &Instance, data: &mut AppData) -> Result<()> {
    for physical_device in instance.enumerate_physical_devices()? {
        let properties = instance.get_physical_device_properties(physical_device);

        if let Err(error) = check_physical_device(instance, data, physical_device) {
            warn!(
                "Skipping physical device (`{}`): {}",
                properties.device_name, error
            );
        } else {
            info!("Selected physical device (`{}`).", properties.device_name);
            data.physical_device = physical_device;
            data.msaa_samples = get_max_msaa_samples(instance, data.physical_device);
            return Ok(());
        }
    }

    Err(anyhow!("Failed to find suitable physical device."))
}

pub unsafe fn create_logical_device(
    entry: &Entry,
    instance: &Instance,
    data: &mut AppData,
) -> Result<Device> {
    let indices = QueueFamilyIndices::get(instance, data, data.physical_device)?;

    let mut unique_indices = HashSet::new();
    unique_indices.insert(indices.graphics);
    unique_indices.insert(indices.present);

    let queue_priorities = &[1.0];
    let queue_infos = unique_indices
        .iter()
        .map(|i| {
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(*i)
                .queue_priorities(queue_priorities)
        })
        .collect::<Vec<_>>();

    let layers = if VALIDATION_ENABLED {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        vec![]
    };

    let mut extensions = DEVICE_EXTENSIONS
        .iter()
        .map(|n| n.as_ptr())
        .collect::<Vec<_>>();

    // Required by Vulkan SDK on macOS since 1.3.216.
    if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
        extensions.push(vk::KHR_PORTABILITY_SUBSET_EXTENSION.name.as_ptr());
    }

    let features = vk::PhysicalDeviceFeatures::builder()
        .sampler_anisotropy(true)
        .sample_rate_shading(true); // Sample shading

    let info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos)
        .enabled_layer_names(&layers)
        .enabled_extension_names(&extensions)
        .enabled_features(&features);

    let device = instance.create_device(data.physical_device, &info, None)?;
    data.graphics_queue = device.get_device_queue(indices.graphics, 0);
    data.present_queue = device.get_device_queue(indices.present, 0);

    Ok(device)
}
