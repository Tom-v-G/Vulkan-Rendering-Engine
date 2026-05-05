#version 450

layout(binding = 0) uniform sampler2D font_texture;

layout(location = 0) in vec2 frag_uv;
layout(location = 1) in vec4 frag_color;

layout(location = 0) out vec4 out_color;

void main() {
    // egui uses pre-multiplied alpha throughout, and the pipeline blend state
    // in Gui::create is set up for pre-multiplied alpha, so no extra work here.
    // Multiplying the vertex color (which carries tint + opacity) by the
    // texture sample gives the correct composited output.
    out_color = frag_color * texture(font_texture, frag_uv);
}