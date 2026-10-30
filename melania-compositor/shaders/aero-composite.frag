#version 300 es
precision highp float;
in vec2 v_uv;
out vec4 frag_color;
uniform sampler2D u_blurred_backdrop;
uniform vec4 u_tint; // premultiplied surface color (blue/aqua Aero tint)
uniform float u_tint_strength; // [0,1]
uniform float u_panel_opacity; // [0,1]
void main() {
    vec4 backdrop = texture(u_blurred_backdrop, v_uv);
    vec3 tinted = mix(backdrop.rgb, u_tint.rgb, clamp(u_tint_strength, 0.0, 1.0));
    frag_color = vec4(tinted * clamp(u_panel_opacity,0.0,1.0), clamp(u_panel_opacity,0.0,1.0));
}
