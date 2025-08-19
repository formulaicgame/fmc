use bevy::{
    asset::{load_internal_asset, Handle},
    image::Image,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::{
        alpha::AlphaMode,
        mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
        render_asset::RenderAssets,
        render_resource::*,
        texture::GpuImage,
    },
};

const BLOCK_SHADER: Handle<Shader> = Handle::weak_from_u128(234982304982304);

pub struct BlockMaterialPlugin;
impl Plugin for BlockMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<BlockMaterial> {
            shadows_enabled: false,
            prepass_enabled: true,
            ..default()
        });

        load_internal_asset!(
            app,
            BLOCK_SHADER,
            "../shaders/blocks.wgsl",
            Shader::from_wgsl
        );
    }
}

// TODO: For a 32x world meshes take up around 2gb of memory. Each vertex is a 3xf32, each normal
// the same, and each uv 2xf32. This can be packed, there exists only 4 uvs, that is 2 bits. For a
// cube there exists only 6 normals(3 bits). To accomplish this in a seamsless way two materials
// should be crated for each one defined. One for normal blocks and one for blocks
// which have custom normals. This is for example blocks like water. The materials will not be
// any different, only stored under different names. This way you can define a set a blocks with
// the same material without having to worry about choosing the correct material. The meshes can
// then be separated easily and can be distinguished from each other in the shader by the
// VERTEX_NORMALS shader def.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(BlockMaterialKey)]
#[uniform(0, BlockMaterialUniform)]
pub struct BlockMaterial {
    /// Defaults to [`Color::WHITE`].
    pub base_color: LinearRgba,

    /// Whether to cull the "front", "back" or neither side of a mesh.
    pub cull_mode: Option<Face>,

    /// See [`AlphaMode`] for details. Defaults to [`AlphaMode::Opaque`].
    pub alpha_mode: AlphaMode,

    /// Adjust rendered depth.
    ///
    /// A material with a positive depth bias will render closer to the
    /// camera while negative values cause the material to render behind
    /// other objects.
    ///
    /// [z-fighting]: https://en.wikipedia.org/wiki/Z-fighting
    pub depth_bias: f32,

    /// Texture array of faces for all blocks that use this material.
    /// Inedexed within the shader by an index bit-packed in the mesh.
    #[texture(1, dimension = "2d_array")]
    #[sampler(2)]
    pub texture_array: Option<Handle<Image>>,

    // TODO: Need a way to define the length of the animation too.
    /// Cycle through the n next textures in the texture array. Defaults to 1(no animation)
    pub animation_frames: u32,
}

impl BlockMaterial {
    pub const ATTRIBUTE_PACKED_BITS: MeshVertexAttribute =
        MeshVertexAttribute::new("Packed_bits", 10, VertexFormat::Uint32);
}

bitflags::bitflags! {
    /// Bitflags info about the material a shader is currently rendering.
    /// This is accessible in the shader in the [`BlockMaterialUniform`]
    #[repr(transparent)]
    pub struct BlockMaterialFlags: u32 {
        const ALPHA_MODE_RESERVED_BITS   = (Self::ALPHA_MODE_MASK_BITS << Self::ALPHA_MODE_SHIFT_BITS); // ← Bitmask reserving bits for the `AlphaMode`
        const ALPHA_MODE_OPAQUE          = (0 << Self::ALPHA_MODE_SHIFT_BITS);                          // ← Values are just sequential values bitshifted into
        const ALPHA_MODE_MASK            = (1 << Self::ALPHA_MODE_SHIFT_BITS);                          //   the bitmask, and can range from 0 to 7.
        const ALPHA_MODE_BLEND           = (2 << Self::ALPHA_MODE_SHIFT_BITS);                          //
        const ALPHA_MODE_PREMULTIPLIED   = (3 << Self::ALPHA_MODE_SHIFT_BITS);                          //
        const ALPHA_MODE_ADD             = (4 << Self::ALPHA_MODE_SHIFT_BITS);                          //   Right now only values 0–5 are used, which still gives
        const ALPHA_MODE_MULTIPLY        = (5 << Self::ALPHA_MODE_SHIFT_BITS);                          // ← us "room" for two more modes without adding more bits
        const NONE                       = 0;
        const UNINITIALIZED              = 0xFFFF;
    }
}

impl BlockMaterialFlags {
    const ALPHA_MODE_MASK_BITS: u32 = 0b111;
    const ALPHA_MODE_SHIFT_BITS: u32 = 32 - Self::ALPHA_MODE_MASK_BITS.count_ones();
}

#[derive(Clone, Default, ShaderType)]
pub struct BlockMaterialUniform {
    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
    /// in between.
    pub base_color: Vec4,
    /// The [`StandardMaterialFlags`] accessible in the `wgsl` shader.
    pub flags: u32,
    /// When the alpha mode mask flag is set, any base color alpha above this cutoff means fully opaque,
    /// and any below means fully transparent.
    pub alpha_cutoff: f32,
    /// How many textures from the texture array the material should cycle through.
    pub animation_frames: u32,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BlockMaterialKey {
    cull_mode: Option<Face>,
    depth_bias: i32,
}

impl From<&BlockMaterial> for BlockMaterialKey {
    fn from(material: &BlockMaterial) -> Self {
        BlockMaterialKey {
            cull_mode: material.cull_mode,
            depth_bias: material.depth_bias as i32,
        }
    }
}

impl Material for BlockMaterial {
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Self::ATTRIBUTE_PACKED_BITS.at_shader_location(3),
        ])?;

        descriptor.vertex.buffers = vec![vertex_layout];

        descriptor.primitive.cull_mode = key.bind_group_data.cull_mode;

        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.bias.constant = key.bind_group_data.depth_bias;
        }

        return Ok(());
    }

    fn vertex_shader() -> ShaderRef {
        BLOCK_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        BLOCK_SHADER.into()
    }

    #[inline]
    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    #[inline]
    fn depth_bias(&self) -> f32 {
        self.depth_bias
    }
}

impl AsBindGroupShaderType<BlockMaterialUniform> for BlockMaterial {
    fn as_bind_group_shader_type(&self, _images: &RenderAssets<GpuImage>) -> BlockMaterialUniform {
        let mut flags = BlockMaterialFlags::NONE;

        let mut alpha_cutoff = 0.5;
        match self.alpha_mode {
            AlphaMode::Opaque => flags |= BlockMaterialFlags::ALPHA_MODE_OPAQUE,
            AlphaMode::Mask(c) => {
                alpha_cutoff = c;
                flags |= BlockMaterialFlags::ALPHA_MODE_MASK;
            }
            AlphaMode::Blend => flags |= BlockMaterialFlags::ALPHA_MODE_BLEND,
            AlphaMode::Premultiplied => flags |= BlockMaterialFlags::ALPHA_MODE_PREMULTIPLIED,
            AlphaMode::Add => flags |= BlockMaterialFlags::ALPHA_MODE_ADD,
            AlphaMode::Multiply => flags |= BlockMaterialFlags::ALPHA_MODE_MULTIPLY,
            _ => (),
        };

        BlockMaterialUniform {
            base_color: LinearRgba::from(self.base_color).to_vec4(),
            flags: flags.bits(),
            alpha_cutoff,
            animation_frames: self.animation_frames,
        }
    }
}
