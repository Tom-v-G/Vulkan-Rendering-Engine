#version 450

layout(location = 0) in uvec4 fragColor;

layout(location = 0) out vec4 outColor;

layout(push_constant) uniform PushConstants {
    layout(offset = 64) float opacity;
} pcs;

void main() {
    outColor = vec4(fragColor.rgb, pcs.opacity);
}

