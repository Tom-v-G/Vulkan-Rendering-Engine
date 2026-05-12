pub use anyhow::{anyhow, Result};
pub use log::*;
use std::ffi::CString;

pub use vulkanalia::prelude::v1_0::*;

use vulkanalia::bytecode::Bytecode;
// use vulkanalia::vk;
use vulkanalia::vk::ExtDebugUtilsExtension;

pub unsafe fn get_memory_type_index(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    properties: vk::MemoryPropertyFlags,
    requirements: vk::MemoryRequirements,
) -> Result<u32> {
    let memory = instance.get_physical_device_memory_properties(physical_device);

    (0..memory.memory_type_count)
        .find(|i| {
            let suitable = (requirements.memory_type_bits & (1 << i)) != 0;
            let memory_type = memory.memory_types[*i as usize];
            suitable && memory_type.property_flags.contains(properties)
        })
        .ok_or_else(|| anyhow!("Failed to find suitable memory type."))
}

pub unsafe fn create_shader_module(device: &Device, bytecode: &[u8]) -> Result<vk::ShaderModule> {
    let bytecode = Bytecode::new(bytecode).unwrap();
    let info = vk::ShaderModuleCreateInfo::builder()
        .code_size(bytecode.code_size())
        .code(bytecode.code());

    Ok(device.create_shader_module(&info, None)?)
}

pub unsafe fn set_debug_name(
    instance: &Instance,
    device: &Device,
    object: u64,
    object_type: vk::ObjectType,
    name: &str,
) -> Result<()> {
    let name = CString::new(name).unwrap();

    let info = vk::DebugUtilsObjectNameInfoEXT::builder()
        .object_handle(object)
        .object_type(object_type)
        .object_name(&name.as_bytes());

    instance.set_debug_utils_object_name_ext(device.handle(), &info)?;

    return Ok(());
}
