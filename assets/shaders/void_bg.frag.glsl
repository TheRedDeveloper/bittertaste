#version 100
precision mediump float;
varying vec2 uv;
uniform sampler2D Texture;
uniform float u_tension;
uniform float u_relief;
uniform vec2 u_resolution;

void main() {
  float aspect = u_resolution.x / max(u_resolution.y, 1.0);
  vec2 centered = (uv - 0.5) * vec2(aspect, 1.0);
  float dist = length(centered);

  // Clean light emptiness gradient: warm alabaster -> cool overcast -> pale mist
  vec3 warm_center = vec3(0.975, 0.965, 0.945);
  vec3 tense_center = vec3(0.915, 0.922, 0.938);
  vec3 relief_center = vec3(0.950, 0.968, 0.972);

  vec3 center_col = mix(warm_center, tense_center, clamp(u_tension, 0.0, 1.0));
  center_col = mix(center_col, relief_center, clamp(u_relief, 0.0, 1.0));

  vec3 edge_col = mix(vec3(0.845, 0.828, 0.804), vec3(0.800, 0.816, 0.840), clamp(u_tension, 0.0, 1.0));
  float vig = smoothstep(0.10, 0.88 * max(aspect, 1.0), dist);
  vec3 col = mix(center_col, edge_col, vig);

  gl_FragColor = vec4(col, 1.0);
}
