#version 450
#extension GL_ARB_separate_shader_objects : enable

#include "skia.glsl"

// @[semantic("POSITION")]
layout(location = 0) in vec2 inPosition;
// @[semantic("TEXCOORD")]
layout(location = 1) in vec2 inTexCoord;

layout(location = 0) out vec2 fragTexCoord;

void main() {
    gl_Position = vec4(inPosition, 0.0, 1.0);
    fragTexCoord = inTexCoord;
}