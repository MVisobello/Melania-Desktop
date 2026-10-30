#version 300 es
precision highp float;
in vec2 v_uv;
out vec4 frag_color;
uniform sampler2D u_backdrop;
uniform vec2 u_texel_step; // (1/width,0) horizontal, then (0,1/height)
// Separable 9-tap Gaussian, normalized weights, premultiplied alpha input.
void main() {
    vec4 color = texture(u_backdrop, v_uv) * 0.227027;
    color += texture(u_backdrop, v_uv + u_texel_step * 1.384615) * 0.316216;
    color += texture(u_backdrop, v_uv - u_texel_step * 1.384615) * 0.316216;
    color += texture(u_backdrop, v_uv + u_texel_step * 3.230769) * 0.070270;
    color += texture(u_backdrop, v_uv - u_texel_step * 3.230769) * 0.070270;
    frag_color = color;
}
