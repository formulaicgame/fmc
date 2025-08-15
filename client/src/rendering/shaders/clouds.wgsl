#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip
}
#import bevy_render::maths::E;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) coverage: f32,
};

struct FragmentOutput {
    @location(0) coverage: f32,
    @location(1) depth: f32,
};

@vertex
fn vertex(
    vertex: Vertex
) -> VertexOutput {
    var out: VertexOutput;

    var world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var local_from_world = mesh_functions::get_local_from_world(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.position = position_world_to_clip(world_position.xyz);

    let local = world_position - world_from_local[3];
    let mesh_origin = world_position.xyz - local.xyz;
    let distance_to_camera = distance(view.world_position, mesh_origin);
    //let sigmoid = 2.0 / (1 + pow(e, -5 * distance_to_camera)) - 1.0;
    out.coverage = pow(clamp((distance_to_camera - 15.0) / 20.0, 0.0, 1.0), 2.0);

    return out;
}

@fragment
fn fragment(
    in: VertexOutput
) -> FragmentOutput {
    var out: FragmentOutput;
    out.coverage = in.coverage;
    out.depth = in.position.z;
    return out;
} 
