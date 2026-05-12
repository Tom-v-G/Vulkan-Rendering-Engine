#version 450

layout(location = 1) in vec4 fragColor;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = vec4(fragColor.rgb, 1.0);
}

