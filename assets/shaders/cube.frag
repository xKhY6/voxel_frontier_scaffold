#version 330 core

in vec3 v_color;
in vec3 v_normal;

uniform float u_ambient;
uniform vec3 u_sun_direction;

out vec4 frag_color;

void main() {
    float sun = max(dot(normalize(v_normal), normalize(u_sun_direction)), 0.0);
    float directional_light = 0.65 + 0.35 * sun;
    frag_color = vec4(v_color * u_ambient * directional_light, 1.0);
}
