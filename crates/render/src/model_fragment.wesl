// Fragment-facing model varyings. Keep locations paired with model::VertexOutput.
struct FragmentInput {
    @builtin(position) clip_position: vec4<f32>,
    // Partially covered MSAA pixels must sample inside the authored quad.
    @location(0) @interpolate(perspective, centroid) uv: vec2<f32>,
    // Current and next animation frames share one flat slot.
    @location(1) @interpolate(flat) textures: vec2<u32>,
    @location(2) normal: vec3<f32>,
    @location(3) @interpolate(flat) material_flags: u32,
    @location(4) @interpolate(flat) local_position: vec3<f32>,
    @location(5) @interpolate(flat) biome_record: u32,
    @location(7) @interpolate(flat) frame_blend: f32,
    // x: visibility and sidedness flags; y: the exact f32 bits of the Cinnabar mask strength;
    // zw: inverted Cinnabar glow and emissive mask references (zero names none).
    @location(8) @interpolate(flat) visibility: vec4<u32>,
    @location(9) lighting: vec3<f32>,
    @if(ENHANCED)
    @location(11) sky_light: f32,
    @if(ENHANCED)
    @location(6) ambient_occlusion: f32,
    @if(!ENHANCED)
    @location(11) native_light_levels: vec2<f32>,
    @if(!ENHANCED)
    @location(6) native_ao_face: f32,
    @location(10) @interpolate(flat) world_origin: vec3<f32>,
    @location(12) world_position: vec3<f32>,
    @if(ENHANCED)
    @location(13) @interpolate(flat) surface_class: u32,
    @if(!ENHANCED)
    @location(13) @interpolate(flat) tint_gamma: vec3<f32>,
}
