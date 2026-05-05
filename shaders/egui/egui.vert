#version 450

// Matches the vertex layout from Gui::create:
// offset 0:  vec2 pos   (R32G32_SFLOAT)
// offset 8:  vec2 uv    (R32G32_SFLOAT)
// offset 16: uint color (R8G8B8A8_UNORM packed as 4 bytes)
layout(location = 0) in vec2 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color; // Vulkan unpacks R8G8B8A8_UNORM to vec4 [0.0, 1.0]

layout(push_constant) uniform PushConstants {
    vec2 screen_size; // width, height in logical pixels
} push_constants;

layout(location = 0) out vec2 frag_uv;
layout(location = 1) out vec4 frag_color;

void main() {
    // Convert egui logical pixel coords to Vulkan NDC:
    // egui origin is top-left, x right, y down — same as Vulkan clip space
    // so we only need to scale and shift, no axis flip required.
    vec2 ndc = (in_pos / push_constants.screen_size) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);

    frag_uv    = in_uv;
    frag_color = in_color;
}