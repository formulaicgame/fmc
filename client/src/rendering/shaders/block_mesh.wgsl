#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings,
    mesh_bindings,
    view_transformations::position_world_to_clip
}

const HALF_PI: f32 = 1.57079632679;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
#ifdef VERTEX_TANGENTS
    @location(3) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(4) color: vec4<f32>,
#endif
#ifdef SKINNED
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
#endif
    @location(7) packed_bits: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) texture_index: i32,
#ifdef VERTEX_TANGENTS
    @location(4) world_tangent: vec4<f32>,
#endif
    @location(5) light: u32,
};

// Note: 0,0 is top left corner
const UVS: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
);

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    out.light = (vertex.packed_bits >> 22u) & 0xFFu;
    out.texture_index = i32(vertex.packed_bits & 0x0007FFFFu);

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent);
#endif

    // Derive which face of the block is being rendered from the normal. This can again be used to
    // derive the uv of the vertex by combining it with which corner it is and the world position.
    // If the block is made from custom vertices care must be taken to lign up its texture
    // with its position in the block.
    //let abs_normal: vec3<f32> = abs(out.world_normal);
    //let max_normal = max(max(abs_normal.x, abs_normal.y), abs_normal.z);
    //if max_normal == abs_normal.x {
    //    out.uv = out.world_position.zy;
    //} else if max_normal == abs_normal.y {
    //    out.uv = out.world_position.xz;
    //} else  {
    //    out.uv = out.world_position.xy;
    //}

    //// TODO: Naga might allow indexing without const value in the future
    //let uv_index: u32 = (vertex.packed_bits & 0x180000u) >> 19u;
    //if uv_index == 0u {
    //    // Top left corner
    //    out.uv = vec2(
    //        // fract is x - x.floor() so it's inversed for negative numbers
    //        fract(out.uv.x),
    //        fract(out.uv.y)
    //    );
    //} else if uv_index == 1u {
    //    // Bottom left corner
    //    out.uv = vec2(
    //        fract(out.uv.x),
    //        // Since this is on the high side of the range extracting the fract is harder since it
    //        // can be a whole number. e.g a position of 1.0 should give a fraction of 1.0, not 0.0.
    //        out.uv.y - (ceil(out.uv.y) - 1)
    //    );
    //} else if uv_index == 2u {
    //    // Top right corner
    //    out.uv = vec2(
    //        out.uv.x - (ceil(out.uv.x) - 1),
    //        fract(out.uv.y)
    //    );
    //} else if uv_index == 3u {
    //    // Bottom right corner
    //    out.uv = vec2(
    //        out.uv.x - (ceil(out.uv.x) - 1),
    //        out.uv.y - (ceil(out.uv.y) - 1)
    //    );
    //}

    out.uv = vertex.uv;

    let rotate_uv = bool((vertex.packed_bits & 0x200000u) >> 21u);
    if rotate_uv {
        out.uv = vec2<f32>(
            0.5 + cos(0.25 * HALF_PI) * (out.uv.x - 0.5) + sin(0.25 * HALF_PI) * (out.uv.y - 0.5),
            0.5 - sin(0.25 * HALF_PI) * (out.uv.x - 0.5) + cos(0.25 * HALF_PI) * (out.uv.y - 0.5),
        );
    }

    //let rotation = f32((vertex.packed_bits & 0x38000000u) >> 27u);
    //let rotation = 7.0;

    //let out_position = vec4<f32>(
    //    center.x + cos(rotation * HALF_PI) * (vertex.position.x - center.x) - sin(rotation * HALF_PI) * (vertex.position.z - center.z),
    //    vertex.position.y,
    //    center.z + sin(rotation * HALF_PI) * (vertex.position.x - center.x) + sin(rotation * HALF_PI) * (vertex.position.z - center.z),
    //    1.0
    //);

    return out;
}

