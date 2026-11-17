import bevy_render::view::View;
import bevy_render::globals::Globals;
import render::lighting::{tint_to_linear, tint_to_gamma};

struct Shape {
    transform: mat4x4<f32>,
    color: vec4<f32>,
    data: vec4<f32>,
    flags: vec4<u32>,
    lifetime: vec4<f32>,
}
struct Actor { position: vec3<f32>, valid: u32, }
struct TextRecord { rect: vec4<f32>, uv: vec4<f32>, color: vec4<f32>, flags: vec4<u32>, }
struct Frame { epoch: f32, dimension: u32, render_distance: f32, padding: u32, }
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<storage, read> shapes: array<vec4<f32>>;

// Raw storage preserves the 128-byte CPU record without a std430 struct block.
fn load_shapes(index: u32) -> Shape {
    let base = index * 8u;
    return Shape(mat4x4<f32>(vec4<f32>(shapes[base + 0u].x, shapes[base + 0u].y, shapes[base + 0u].z, shapes[base + 0u].w), vec4<f32>(shapes[base + 1u].x, shapes[base + 1u].y, shapes[base + 1u].z, shapes[base + 1u].w), vec4<f32>(shapes[base + 2u].x, shapes[base + 2u].y, shapes[base + 2u].z, shapes[base + 2u].w), vec4<f32>(shapes[base + 3u].x, shapes[base + 3u].y, shapes[base + 3u].z, shapes[base + 3u].w)),
        vec4<f32>(shapes[base + 4u].x, shapes[base + 4u].y, shapes[base + 4u].z, shapes[base + 4u].w),
        vec4<f32>(shapes[base + 5u].x, shapes[base + 5u].y, shapes[base + 5u].z, shapes[base + 5u].w),
        vec4<u32>(bitcast<u32>(shapes[base + 6u].x), bitcast<u32>(shapes[base + 6u].y), bitcast<u32>(shapes[base + 6u].z), bitcast<u32>(shapes[base + 6u].w)),
        vec4<f32>(shapes[base + 7u].x, shapes[base + 7u].y, shapes[base + 7u].z, shapes[base + 7u].w));
}
@group(0) @binding(2) var<uniform> frame: Frame;
@group(0) @binding(3) var<uniform> globals: Globals;
@group(0) @binding(4) var<storage, read> actors: array<vec4<u32>>;

// Raw storage preserves the 16-byte CPU record without a std430 struct block.
fn load_actors(index: u32) -> Actor {
    let base = index * 1u;
    return Actor(vec3<f32>(bitcast<f32>(actors[base + 0u].x), bitcast<f32>(actors[base + 0u].y), bitcast<f32>(actors[base + 0u].z)),
        actors[base + 0u].w);
}
@group(0) @binding(5) var<storage, read> texts: array<vec4<u32>>;

// Raw storage preserves the 64-byte CPU record without a std430 struct block.
fn load_texts(index: u32) -> TextRecord {
    let base = index * 4u;
    return TextRecord(vec4<f32>(bitcast<f32>(texts[base + 0u].x), bitcast<f32>(texts[base + 0u].y), bitcast<f32>(texts[base + 0u].z), bitcast<f32>(texts[base + 0u].w)),
        vec4<f32>(bitcast<f32>(texts[base + 1u].x), bitcast<f32>(texts[base + 1u].y), bitcast<f32>(texts[base + 1u].z), bitcast<f32>(texts[base + 1u].w)),
        vec4<f32>(bitcast<f32>(texts[base + 2u].x), bitcast<f32>(texts[base + 2u].y), bitcast<f32>(texts[base + 2u].z), bitcast<f32>(texts[base + 2u].w)),
        vec4<u32>(texts[base + 3u].x, texts[base + 3u].y, texts[base + 3u].z, texts[base + 3u].w));
}
@group(0) @binding(6) var atlas: texture_2d<f32>;
@group(0) @binding(7) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) textured: u32,
    @location(3) @interpolate(flat) backface: u32,
}

// Attachments past the actor arena's binding limit stay hidden, like missing actors.
fn attached(shape: Shape) -> bool {
    return shape.flags.z!=0xffffffffu;
}

// Attachment indices address one compact actor entry shared by every attached shape.
fn anchor(shape: Shape) -> vec3<f32> {
    var result=shape.transform[3].xyz;
    if attached(shape) && shape.flags.z<arrayLength(&actors) { result+=load_actors(shape.flags.z).position; }
    return result;
}

// Server-owned slots remain allocated when dimension or distance hides their geometry.
fn visible(shape: Shape, origin: vec3<f32>) -> bool {
    if shape.flags.x==0u { return false; }
    if shape.flags.w==ARROW_KIND_VALUEu && shape.data.z*shape.data.z<ARROW_MIN_LENGTH_SQUARED_VALUE { return false; }
    if shape.flags.y!=ALL_DIMENSIONS_VALUEu && shape.flags.y!=frame.dimension { return false; }
    if shape.lifetime.x>=0.0 && frame.epoch+globals.time>=shape.lifetime.x { return false; }
    if attached(shape) && (shape.flags.z>=arrayLength(&actors) || load_actors(shape.flags.z).valid==0u) { return false; }
    var position=origin;
    if shape.flags.w==BOX_KIND_VALUEu { position-=0.5*(shape.transform[0]+shape.transform[1]+shape.transform[2]).xyz; }
    let delta=position-view.world_position;
    let range=select(frame.render_distance,shape.data.w,shape.data.w>=0.0);
    if !(dot(delta,delta)<range*range) { return false; }
    return true;
}

// Unit meshes use packet-time transforms; actor offsets remain independently updateable.
@vertex
fn shape_vertex(@location(0) vertex: vec4<f32>, @builtin(instance_index) index: u32) -> VertexOutput {
    let shape=load_shapes(index);
    let origin=anchor(shape);
    var point=vertex.xyz;
    if shape.flags.w==ARROW_KIND_VALUEu {
        point=vec3(vertex.xy*shape.data.y,vertex.z*shape.data.z);
        if vertex.w!=0.0 { point.z=shape.data.z-shape.data.x; }
    }
    var output:VertexOutput;
    var world=origin+(shape.transform*vec4(point,0.0)).xyz;
    if vertex.w<0.0 { world=vec3(0.0); }
    output.position=view.clip_from_world*vec4(world,1.0);
    if !visible(shape,origin) { output.position=vec4(2.0,2.0,2.0,1.0); }
    output.color=select(shape.color,vec4(0.0),vertex.w<0.0);
    output.uv=vec2(0.0);
    output.textured=2u;
    output.backface=1u;
    return output;
}

// Debug text retains its cubic billboard angles.
fn native_acos(value:f32) -> f32 {
    return (ACOS_CUBIC_VALUE*value*value*value-ACOS_LINEAR_VALUE*value)+1.5707963267948966;
}

// Facing uses the unlifted anchor, while multiline lift remains independent of scale.
@vertex
fn text_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) index:u32) -> VertexOutput {
    let record=load_texts(index);
    let in_arena=record.flags.x<(arrayLength(&shapes) / 8u);
    let shape=load_shapes(min(record.flags.x,(arrayLength(&shapes) / 8u)-1u));
    let origin=anchor(shape);
    let at=array<vec2<f32>,6>(vec2(0.0,0.0),vec2(1.0,0.0),vec2(1.0,1.0),vec2(0.0,0.0),vec2(1.0,1.0),vec2(0.0,1.0))[vertex];
    let local=mix(record.rect.xy,record.rect.zw,at);
    var right=shape.transform[0].xyz;
    var down=shape.transform[1].xyz;
    if (record.flags.z&8u)==0u {
        let direction=view.world_position-origin;
        let x=select(direction.x,HORIZONTAL_ZERO_VALUE,direction.x==0.0);
        let z=select(direction.z,HORIZONTAL_ZERO_VALUE,direction.z==0.0);
        let horizontal=sqrt(x*x+z*z);
        let yaw=native_acos(-z/horizontal)*select(1.0,-1.0,x>0.0);
        let pitch_dot=(direction.x*(x/horizontal)+direction.z*(z/horizontal))/length(direction);
        let pitch=native_acos(pitch_dot)*select(-1.0,1.0,direction.y>0.0);
        let scale=length(right)*select(1.0,-1.0,dot(cross(right,down),shape.transform[2].xyz)<0.0);
        right=vec3(-cos(yaw),0.0,sin(yaw))*scale;
        down=vec3(-sin(yaw)*sin(pitch),-cos(pitch),-cos(yaw)*sin(pitch))*scale;
    } else {
        right=-right;
        down=-down;
    }
    let textured=record.uv.z>=0.0;
    var show=in_arena&&visible(shape,origin)&&record.flags.y!=0u;
    @if(TEXT_DEPTH)
    show=show&&(record.flags.z&1u)!=0u;
    @if(!TEXT_DEPTH)
    show=show&&(record.flags.z&1u)==0u;
    @if(TEXT_GLYPH)
    show=show&&textured;
    @if(!TEXT_GLYPH)
    show=show&&!textured;
    var output:VertexOutput;
    output.position=view.clip_from_world*vec4(origin+vec3(0.0,bitcast<f32>(record.flags.w),0.0)+(right*local.x+down*local.y)*TEXT_SCALE_VALUE,1.0);
    if !show { output.position=vec4(2.0,2.0,2.0,1.0); }
    output.color=record.color;
    if textured { output.color*=shape.color; }
    output.uv=mix(record.uv.xy,record.uv.zw,at);
    output.textured=u32(textured);
    output.backface=select(record.flags.z&2u,record.flags.z&4u,textured);
    return output;
}

// Background and glyph backfaces have separate controls; geometry never alpha-blends.
@fragment
fn shape_fragment(input:VertexOutput,@builtin(front_facing) front:bool) -> @location(0) vec4<f32> {
    // All lanes contribute gradients before backface or material rejection.
    let uv_dx=dpdx(input.uv);
    let uv_dy=dpdy(input.uv);
    if !front&&input.backface==0u { discard; }
    var color=input.color;
    if input.textured==1u {
        var texel=textureSampleGrad(atlas,atlas_sampler,input.uv,uv_dx,uv_dy);
        @if(TEXT_ALPHA_TEST)
        if texel.a<0.5 { discard; }
        @if(SHAPE_GAMMA)
        texel=tint_to_gamma(texel);
        color*=texel;
    }
    @if(!SHAPE_GAMMA)
    if input.textured==2u { return tint_to_linear(color); }
    if input.textured==2u { return color; }
    if color.a<=0.0 { discard; }
    return color;
}
