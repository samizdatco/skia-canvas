#version 450
#extension GL_ARB_separate_shader_objects : enable

#include "skia.glsl"

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 outColor;

void main() {
    outColor =  texture(sampler2D(tex, smp), uv);
}