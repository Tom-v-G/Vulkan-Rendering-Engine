#version 450

// Matches the vertex layout from chunkmesher::VoxelVertex
// offset 0:  vec3 pos    (R32G32B32_SFLOAT)
// offset 12: int         (S8_INT)
// offset 13: uvec4 color (R8G8B8A8_UINT alpha is ignored)
layout(location = 0) in vec3 inPosition;
layout(location = 1) in int normal;
layout(location = 2) in uvec4 inColor; 

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
} ubo;

layout(push_constant) uniform ChunkPushConstants {
    vec3 world_pos; // vec3 aligns to 16 bytes -> 12 bytes + 4 padding. Also needs to be added in the rust structs
} cpcs;

layout(location = 1) out vec4 fragColor;

const vec3 NORMALS[6] = vec3[](
    vec3( 1.0,  0.0,  0.0),  // 0: +X
    vec3(-1.0,  0.0,  0.0),  // 1: -X
    vec3( 0.0,  1.0,  0.0),  // 2: +Y
    vec3( 0.0, -1.0,  0.0),  // 3: -Y
    vec3( 0.0,  0.0,  1.0),  // 4: +Z
    vec3( 0.0,  0.0, -1.0)  // 5: -Z
);

void main() {
    gl_Position = ubo.proj * ubo.view * vec4(inPosition + 16.0 * cpcs.world_pos, 1.0); // Note: chunk size is hardcoded. add as uniform
    fragColor = inColor / 255.0;
    // Normals unused for the moment
}