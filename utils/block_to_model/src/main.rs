// TODO: This uses 6 meshes with 6 different materials to avoid bundling the images inside the gltf
// file. The images are not guaranteed to be present, which might cause issues. It also might cause
// performance issues when many blocks are dropped on the ground. Might be preferable to eat the
// size increase and bundle the images as one texture in the gltf. 
use gltf_json as json;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::mem;

use clap::Parser;
use json::validation::Checked::Valid;

// XXX: These are paths to be read at runtime, NOT files that are compiled into the gltf.
#[derive(Parser)]
#[clap(about, long_about = None)]
struct Cli {
    top: String,
    bottom: String,
    #[arg(short, long)]
    sides: Option<String>,
    #[arg(short, long)]
    left: Option<String>,
    #[arg(short, long)]
    right: Option<String>,
    #[arg(short, long)]
    front: Option<String>,
    #[arg(short, long)]
    back: Option<String>,
}

fn get_images(cli: &Cli) -> Vec<json::Image> {
    let mut images = Vec::with_capacity(6);

    images.push(json::Image {
        buffer_view: None,
        mime_type: None,
        name: None,
        uri: Some(cli.top.clone()),
        extensions: None,
        extras: Default::default(),
    });

    images.push(json::Image {
        uri: Some(cli.bottom.clone()),
        buffer_view: None,
        mime_type: None,
        name: None,
        extensions: None,
        extras: Default::default(),
    });

    for side in &[&cli.left, &cli.right, &cli.front, &cli.back] {
        if let Some(side) = side {
            images.push(json::Image {
                uri: Some(side.clone()),
                buffer_view: None,
                mime_type: None,
                name: None,
                extensions: None,
                extras: Default::default(),
            })
        } else {
            images.push(json::Image {
                uri: Some(
                    cli.sides
                        .as_ref()
                        .expect("If one of the sides are omitted, --sides must be used")
                        .clone(),
                ),
                buffer_view: None,
                mime_type: None,
                name: None,
                extensions: None,
                extras: Default::default(),
            })
        }
    }

    return images;
}

const VERTICES: [[[f32; 3]; 6]; 6] = [
    // Top
    [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
    ],
    // Bottom
    [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ],
    // Left
    [
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
    ],
    // Right
    [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
    ],
    // Front
    [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ],
    // Back
    [
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ],
];

const UVS: [[f32; 2]; 6] = [
    [0.0, 1.0],
    [0.0, 0.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [0.0, 0.0],
    [1.0, 0.0],
];

#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    uv_coord: [f32; 2],
}

fn bounding_coords(points: &[[f32; 3]; 6]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX, f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN, f32::MIN];

    for point in points {
        for i in 0..3 {
            min[i] = f32::min(min[i], point[i]);
            max[i] = f32::max(max[i], point[i]);
        }
    }
    (min, max)
}

fn align_to_multiple_of_four(n: &mut u32) {
    *n = (*n + 3) & !3;
}

fn to_padded_byte_vector<T>(vec: Vec<T>) -> Vec<u8> {
    let byte_length = vec.len() * mem::size_of::<T>();
    let byte_capacity = vec.capacity() * mem::size_of::<T>();
    let alloc = vec.into_boxed_slice();
    let ptr = Box::<[T]>::into_raw(alloc) as *mut u8;
    let mut new_vec = unsafe { Vec::from_raw_parts(ptr, byte_length, byte_capacity) };
    while new_vec.len() % 4 != 0 {
        new_vec.push(0); // pad to multiple of four bytes
    }
    new_vec
}

fn main() {
    let cli = Cli::parse();
    let images = get_images(&cli);
    let mut triangle_vertices = Vec::new();
    let mut accessors = Vec::new();
    let mut textures = Vec::new();
    let mut materials = Vec::new();
    let mut primitives = Vec::new();

    for (i, side) in VERTICES.iter().enumerate() {
        let (min, max) = bounding_coords(side);
        // positions
        accessors.push(json::Accessor {
            buffer_view: Some(json::Index::new(0)),
            //byte_offset: (triangle_vertices.len() * mem::size_of::<Vertex>()) as u32,
            byte_offset: None,
            count: gltf_json::validation::USize64(6),
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::F32,
            )),
            type_: Valid(json::accessor::Type::Vec3),
            min: Some(json::Value::from(Vec::from(min))),
            max: Some(json::Value::from(Vec::from(max))),
            normalized: false,
            sparse: None,
            extensions: None,
            extras: Default::default(),
            name: None,
        });

        // uvs
        accessors.push(json::Accessor {
            buffer_view: Some(json::Index::new(0)),
            //byte_offset: (triangle_vertices.len() * mem::size_of::<Vertex>()
            //    + 3 * mem::size_of::<f32>()) as u32,
            byte_offset: None,
            count: gltf_json::validation::USize64(6),
            component_type: Valid(json::accessor::GenericComponentType(
                json::accessor::ComponentType::F32,
            )),
            type_: Valid(json::accessor::Type::Vec2),
            min: None,
            max: None,
            normalized: false,
            sparse: None,
            extensions: None,
            extras: Default::default(),
            name: None,
        });

        for (i, pos) in side.iter().enumerate() {
            triangle_vertices.push(Vertex {
                //position: [pos[0], pos[1], pos[2]],
                position: pos.clone(),
                uv_coord: UVS[i],
            })
        }

        textures.push(json::Texture {
            name: None,
            sampler: Some(json::Index::new(0)),
            source: json::Index::new(i as u32),
            extensions: None,
            extras: Default::default(),
        });

        materials.push(json::Material {
            pbr_metallic_roughness: json::material::PbrMetallicRoughness {
                base_color_texture: Some(json::texture::Info {
                    index: json::Index::new(i as u32),
                    tex_coord: 0,
                    extensions: None,
                    extras: Default::default(),
                }),
                ..Default::default()
            },
            ..Default::default()
        });

        primitives.push(json::mesh::Primitive {
            attributes: {
                let mut map = BTreeMap::new();
                map.insert(Valid(json::mesh::Semantic::Positions), json::Index::new(i as u32 * 2));
                map.insert(Valid(json::mesh::Semantic::TexCoords(0)), json::Index::new(i as u32 * 2 + 1));
                map
            },
            material: Some(json::Index::new(i as u32)),
            mode: Valid(json::mesh::Mode::Triangles),
            indices: None,
            extensions: None,
            extras: Default::default(),
            targets: None,
        });
    }


    let buffer_length = (triangle_vertices.len() * mem::size_of::<Vertex>()) as u64;
    let buffer = json::Buffer {
        byte_length: gltf_json::validation::USize64(buffer_length),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: None,
    };

    let buffer_view = json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: buffer.byte_length,
        byte_offset: None,
        byte_stride: Some(json::buffer::Stride(mem::size_of::<Vertex>())),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ArrayBuffer)),
    };

    let mesh = json::Mesh {
        extensions: None,
        extras: Default::default(),
        name: None,
        primitives,
        weights: None,
    };
    
    let sampler = json::texture::Sampler {
        mag_filter: Some(Valid(json::texture::MagFilter::Nearest)),
        min_filter: Some(Valid(json::texture::MinFilter::Nearest)),
        name: None,
        wrap_s: Valid(json::texture::WrappingMode::ClampToEdge),
        wrap_t: Valid(json::texture::WrappingMode::ClampToEdge),
        extensions: None,
        extras: Default::default()
    };

    let node = json::Node {
        camera: None,
        children: None,
        extensions: Default::default(),
        extras: Default::default(),
        matrix: None,
        mesh: Some(json::Index::new(0)),
        name: None,
        rotation: None,
        scale: None,
        translation: None,
        skin: None,
        weights: None,
    };

    let root = json::Root {
        accessors,
        animations: vec![left_click],
        buffers: vec![buffer],
        buffer_views: vec![buffer_view],
        meshes: vec![mesh],
        nodes: vec![node],
        scenes: vec![json::Scene {
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            nodes: vec![json::Index::new(0)],
        }],
        materials,
        textures,
        images,
        samplers: vec![sampler],
        ..Default::default()
    };

    let json_string = json::serialize::to_string(&root).expect("Serialization error");
    let mut json_offset = json_string.len() as u32;

    align_to_multiple_of_four(&mut json_offset);

    let glb = gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: json_offset + buffer_length as u32,
        },
        bin: Some(Cow::Owned(to_padded_byte_vector(triangle_vertices))),
        json: Cow::Owned(json_string.into_bytes()),
    };

    let file = std::fs::File::create("output.glb").expect("I/O error");
    glb.to_writer(file).expect("glTF binary output error");
}
