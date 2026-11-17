// One pipeline for sorted transparent terrain: water and model draws alternate back to front,
// and every program switch between them costs the GPU as much as the drawing itself. A draw whose
// first instance carries TRANSPARENT_WATER_DRAW_FLAG shades its instances as transparent liquid,
// sorted refs or direct records as `liquid_record` selects; any other draw shades model draw
// refs. Both paths run the liquid and model programs' own vertex and shading functions on their
// own outputs, so the picture matches drawing each family through its own pipeline.
import render::chunk_bindings::{TRANSPARENT_WATER_DRAW_FLAG, transparent_refs};
import render::liquid::{FragmentInput as LiquidFragmentInput, liquid_record, vertex_for_record, shade_liquid};
import render::model::{model_vertex, shade_blend};

import render::model_fragment::FragmentInput as ModelFragmentInput;

struct FragmentInput {
    @builtin(position) clip_position: vec4<f32>,
    // x: water/two-sided/depth-write bits; y: model visibility; zw: material/biome.
    @location(0) @interpolate(flat) routing: vec4<u32>,
    // xy: texture refs; z: frame-blend bits; w: Enhanced surface class.
    @location(1) @interpolate(flat) texture_frame: vec4<u32>,
    @location(2) @interpolate(flat) face_local: vec4<f32>,
    @if(ENHANCED)
    @location(3) @interpolate(flat) world_origin: vec3<f32>,
    @if(!ENHANCED)
    @location(3) @interpolate(flat) tint_gamma: vec3<f32>,
    @location(4) liquid_aux: vec4<f32>,
    @location(5) world_ao: vec4<f32>,
    @location(6) @interpolate(perspective, centroid) model_uv: vec2<f32>,
    @location(7) water_tint: vec4<f32>,
    @if(ENHANCED)
    @location(8) normal: vec3<f32>,
    @if(ENHANCED)
    @location(9) lighting: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) @invariant clip_position: vec4<f32>,
    // x: water/two-sided/depth-write bits; y: model visibility; zw: material/biome.
    @location(0) @interpolate(flat) routing: vec4<u32>,
    // xy: texture refs; z: frame-blend bits; w: Enhanced surface class.
    @location(1) @interpolate(flat) texture_frame: vec4<u32>,
    @location(2) @interpolate(flat) face_local: vec4<f32>,
    @if(ENHANCED)
    @location(3) @interpolate(flat) world_origin: vec3<f32>,
    @if(!ENHANCED)
    @location(3) @interpolate(flat) tint_gamma: vec3<f32>,
    @location(4) liquid_aux: vec4<f32>,
    @location(5) world_ao: vec4<f32>,
    @location(6) @interpolate(perspective, centroid) model_uv: vec2<f32>,
    @location(7) water_tint: vec4<f32>,
    @if(ENHANCED)
    @location(8) normal: vec3<f32>,
    @if(ENHANCED)
    @location(9) lighting: vec3<f32>,
}

@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    return terrain_vertex(vertex_index, instance_index);
}

// Shared reconstruction also serves host-meshed retained geometry.
fn terrain_vertex(vertex_index: u32, instance_index: u32) -> VertexOutput {
    var out: VertexOutput;
    if ((instance_index & TRANSPARENT_WATER_DRAW_FLAG) != 0u) {
        let record = liquid_record(instance_index & ~TRANSPARENT_WATER_DRAW_FLAG);
        let liquid = vertex_for_record(record, vertex_index);
        out.clip_position = liquid.clip_position;
        out.routing.x = (out.routing.x & ~1u) | ((1u) << 0u);
        out.texture_frame.x = liquid.textures_meta.x;
        out.texture_frame.y = liquid.textures_meta.y;
        out.texture_frame.z = bitcast<u32>(bitcast<f32>(liquid.frame_meta.x));
        out.routing.x = (out.routing.x & ~2u) | ((liquid.textures_meta.z) << 1u);
        out.world_ao = vec4(liquid.world_position, out.world_ao.w);
        @if(ENHANCED)
        out.lighting = liquid.lighting;
        @if(ENHANCED)
        out.normal = liquid.normal;
        @if(ENHANCED)
        out.liquid_aux.z = liquid.uv_aux.z;
        @if(ENHANCED)
        out.liquid_aux.w = liquid.uv_aux.w;
        @if(ENHANCED)
        out.texture_frame.w = liquid.frame_meta.z;
        @if(!ENHANCED)
        { let levels_ao = vec3(liquid.uv_aux.zw, 0.0); out.liquid_aux = vec4(out.liquid_aux.xy, levels_ao.xy); out.world_ao.w = levels_ao.z; }
        out.liquid_aux = vec4(liquid.uv_aux.xy, out.liquid_aux.zw);
        out.water_tint = liquid.water_tint;
        out.routing.x = (out.routing.x & ~4u) | ((liquid.textures_meta.w) << 2u);
        out.face_local.w = bitcast<f32>(liquid.frame_meta.y);
        return out;
    }
    let model = model_vertex(vertex_index, instance_index);
    out.clip_position = model.clip_position;
    out.routing.x = (out.routing.x & ~1u) | ((0u) << 0u);
    out.texture_frame.x = model.textures_meta.x;
    out.texture_frame.y = model.textures_meta.y;
    out.texture_frame.z = bitcast<u32>(bitcast<f32>(model.textures_meta.w));
    out.world_ao = vec4(model.world_ao.xyz, out.world_ao.w);
    @if(ENHANCED)
    out.lighting = model.lighting;
    @if(ENHANCED)
    out.normal = model.normal_sky.xyz;
    @if(ENHANCED)
    out.liquid_aux.z = model.normal_sky.w;
    @if(ENHANCED)
    out.liquid_aux.w = model.world_ao.w;
    @if(ENHANCED)
    out.texture_frame.w = model.biome_surface.y;
    @if(!ENHANCED)
    { let levels_ao = vec3(model.native_light_levels, model.world_ao.w); out.liquid_aux = vec4(out.liquid_aux.xy, levels_ao.xy); out.world_ao.w = levels_ao.z; }
    @if(!ENHANCED)
    out.tint_gamma = model.tint_gamma;
    out.model_uv = model.uv;
    out.routing.z = model.textures_meta.z;
    @if(ENHANCED)
    out.face_local = vec4(model.local_position, out.face_local.w);
    @if(ENHANCED)
    out.routing.w = model.biome_surface.x;
    out.routing.y = model.visibility.x;
    @if(ENHANCED)
    out.world_origin = model.world_origin;
    return out;
}

// Each sorted face keeps the ordinary liquid/model decoder and fragment shader.
@if(MOD_BLOCKS)
@vertex
fn vertex_blocks(@builtin(vertex_index) corner: u32, @builtin(instance_index) face: u32) -> VertexOutput {
    let reference = transparent_refs(face);
    return terrain_vertex(reference.metadata_index + corner, reference.liquid_record_index);
}

fn liquid_output(in: FragmentInput) -> LiquidFragmentInput {
    var liquid: LiquidFragmentInput;
    liquid.clip_position = in.clip_position;
    liquid.uv_aux = vec4(in.liquid_aux.xy, liquid.uv_aux.zw);
    liquid.textures_meta.x = in.texture_frame.x;
    liquid.textures_meta.y = in.texture_frame.y;
    liquid.frame_meta.x = bitcast<u32>(bitcast<f32>(in.texture_frame.z));
    liquid.water_tint = in.water_tint;
    @if(ENHANCED)
    liquid.lighting = in.lighting;
    @if(ENHANCED)
    liquid.uv_aux.z = in.liquid_aux.z;
    @if(ENHANCED)
    liquid.uv_aux.w = in.liquid_aux.w;
    @if(ENHANCED)
    liquid.normal = in.normal;
    @if(ENHANCED)
    liquid.frame_meta.z = in.texture_frame.w;
    @if(!ENHANCED)
    liquid.uv_aux = vec4(liquid.uv_aux.xy, in.liquid_aux.zw);
    liquid.textures_meta.w = ((in.routing.x >> 2u) & 1u);
    liquid.world_position = in.world_ao.xyz;
    liquid.frame_meta.y = bitcast<u32>(in.face_local.w);
    liquid.textures_meta.z = ((in.routing.x >> 1u) & 1u);
    return liquid;
}

fn model_output(in: FragmentInput) -> ModelFragmentInput {
    var model: ModelFragmentInput;
    model.clip_position = in.clip_position;
    model.uv = in.model_uv;
    model.textures_meta.x = in.texture_frame.x;
    @if(ENHANCED)
    model.normal_sky = vec4(in.normal, model.normal_sky.w);
    model.textures_meta.z = in.routing.z;
    @if(ENHANCED)
    model.local_position = in.face_local.xyz;
    @if(ENHANCED)
    model.biome_surface.x = in.routing.w;
    model.textures_meta.y = in.texture_frame.y;
    model.textures_meta.w = bitcast<u32>(bitcast<f32>(in.texture_frame.z));
    model.visibility.x = in.routing.y;
    @if(ENHANCED)
    model.lighting = in.lighting;
    @if(ENHANCED)
    model.normal_sky.w = in.liquid_aux.z;
    @if(ENHANCED)
    model.world_ao.w = in.liquid_aux.w;
    @if(ENHANCED)
    model.biome_surface.y = in.texture_frame.w;
    @if(!ENHANCED)
    model.native_light_levels = in.liquid_aux.zw;
    @if(!ENHANCED)
    model.world_ao.w = in.world_ao.w;
    @if(!ENHANCED)
    model.tint_gamma = in.tint_gamma;
    @if(ENHANCED)
    model.world_origin = in.world_origin;
    model.world_ao = vec4(in.world_ao.xyz, model.world_ao.w);
    return model;
}

@fragment
fn fragment(in: FragmentInput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    // Derivatives stay in uniform control flow; each family only reads its own.
    let liquid_dx = dpdx(in.liquid_aux.xy);
    let liquid_dy = dpdy(in.liquid_aux.xy);
    let model_dx = dpdx(in.model_uv);
    let model_dy = dpdy(in.model_uv);
    if (((in.routing.x >> 0u) & 1u) != 0u) {
        // Packed liquid corners wind clockwise; this pipeline's front face is counter-clockwise.
        return shade_liquid(liquid_output(in), !front_facing, liquid_dx, liquid_dy);
    }
    return shade_blend(model_output(in), front_facing, model_dx, model_dy);
}
