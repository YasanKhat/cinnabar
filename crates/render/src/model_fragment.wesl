// Fragment-facing model varyings; paired with model::VertexOutput.
struct FragmentInput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(perspective, centroid) uv: vec2<f32>,
    @location(1) @interpolate(flat) textures_meta: vec4<u32>,
    @location(2) @interpolate(flat) visibility: vec4<u32>,
    @location(3) world_ao: vec4<f32>,
    @if(!ENHANCED)
    @location(4) native_light_levels: vec2<f32>,
    @if(!ENHANCED)
    @location(5) @interpolate(flat) tint_gamma: vec3<f32>,
    @if(ENHANCED)
    @location(4) normal_sky: vec4<f32>,
    @if(ENHANCED)
    @location(5) @interpolate(flat) local_position: vec3<f32>,
    @if(ENHANCED)
    @location(6) @interpolate(flat) biome_surface: vec2<u32>,
    @if(ENHANCED)
    @location(7) lighting: vec3<f32>,
    @if(ENHANCED)
    @location(8) @interpolate(flat) world_origin: vec3<f32>,
}
