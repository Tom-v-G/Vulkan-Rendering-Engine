use vulkanalia::vk;
use vulkanalia::Version;

pub const ENGINE_TITLE: &str = "Garbo Engine";

pub const WINDOW_WIDTH: u32 = 1920;
pub const WINDOW_HEIGHT: u32 = 1080;

pub const PORTABILITY_MACOS_VERSION: Version = Version::new(1, 3, 216);
pub const VALIDATION_ENABLED: bool = cfg!(debug_assertions);
pub const VALIDATION_LAYER: vk::ExtensionName =
    vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");
pub const DEVICE_EXTENSIONS: &[vk::ExtensionName] = &[vk::KHR_SWAPCHAIN_EXTENSION.name];

pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

// Game variables
pub const MOUSE_SENSITIVITY: f32 = 0.01;
pub const MOVEMENT_SPEED: f32 = 3.0;

pub const CHUNK_SIZE: usize = 16;
