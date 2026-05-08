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

layout(push_constant) uniform PushConstants{
    mat4 model;
} pcs;

layout(location = 1) out uvec4 fragColor;

const vec3 NORMALS[6] = vec3[6](
    vec3( 1.0,  0.0,  0.0),  // 0: +X
    vec3(-1.0,  0.0,  0.0),  // 1: -X
    vec3( 0.0,  1.0,  0.0),  // 2: +Y
    vec3( 0.0, -1.0,  0.0),  // 3: -Y
    vec3( 0.0,  0.0,  1.0),  // 4: +Z
    vec3( 0.0,  0.0, -1.0),  // 5: -Z
);

void main() {
    gl_Position = ubo.proj * ubo.view * pcs.model * vec4(inPosition, 1.0);
    fragColor = inColor;
    // Normals unused for the moment
}