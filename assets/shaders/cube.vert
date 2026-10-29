#version 330 core

layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_color;

// Produit projection * vue, calculé côté CPU à partir de la caméra.
uniform mat4 u_view_proj;

out vec3 v_color;

void main() {
    gl_Position = u_view_proj * vec4(a_position, 1.0);
    v_color = a_color;
}
