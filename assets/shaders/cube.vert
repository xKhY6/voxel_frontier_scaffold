#version 330 core

layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_color;
layout (location = 2) in vec3 a_normal;

// Produit projection * vue, calculé côté CPU à partir de la caméra.
uniform mat4 u_view_proj;
uniform mat4 u_model;

out vec3 v_color;
out vec3 v_normal;

void main() {
    gl_Position = u_view_proj * u_model * vec4(a_position, 1.0);
    v_color = a_color;
    v_normal = mat3(transpose(inverse(u_model))) * a_normal;
}
