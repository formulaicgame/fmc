use bevy::{
    math::Vec3A,
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialPipeline, MaterialPipelineKey,
        StandardMaterialFlags, StandardMaterialUniform, MAX_CASCADES_PER_LIGHT,
        MAX_DIRECTIONAL_LIGHTS,
    },
    prelude::*,
    reflect::TypePath,
    render::{
        mesh::{MeshVertexBufferLayout, VertexAttributeValues},
        render_asset::RenderAssets,
        render_resource::*,
    },
};

use crate::{
    rendering::lighting::{Light, LightMap},
    world::Origin,
};

use super::ATTRIBUTE_PACKED_BITS_0;

pub struct PbrMaterialPlugin;
impl Plugin for PbrMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<
            ExtendedMaterial<StandardMaterial, PbrLightExtension>,
        >::default())
            // Some weird schedule ordering here to avoid flickering when replacing the meshes.
            // Chosen at random until it worked.
            .add_systems(PostUpdate, replace_material_and_mesh)
            .add_systems(Last, update_light);
    }
}

fn update_light(
    origin: Res<Origin>,
    light_map: Res<LightMap>,
    mesh_query: Query<
        (&GlobalTransform, &mut Handle<Mesh>),
        (
            With<Handle<ExtendedMaterial<StandardMaterial, PbrLightExtension>>>,
            Or<(
                Changed<GlobalTransform>,
                Added<Handle<ExtendedMaterial<StandardMaterial, PbrLightExtension>>>,
            )>,
        ),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (transform, mesh_handle) in mesh_query.iter() {
        let transform = transform.compute_transform();
        let mesh = meshes.get_mut(mesh_handle).unwrap();
        let mut mesh_aabb = mesh.compute_aabb().unwrap();
        mesh_aabb.center *= Vec3A::from(transform.scale);
        mesh_aabb.half_extents *= Vec3A::from(transform.scale);

        let position = transform.translation + origin.0.as_vec3();

        // There's an assumption here that lighting is finsihed before this first runs that I don't
        // know if holds true.
        let mut new_light = Light(0);
        for (i, offset) in mesh_aabb.half_extents.to_array().into_iter().enumerate() {
            let mut offset_vec = Vec3::ZERO;
            offset_vec[i] = offset;
            for direction in [-1.0, 1.0] {
                let position = (position + Vec3::from(mesh_aabb.center) + offset_vec * direction)
                    .floor()
                    .as_ivec3();
                if let Some(light) = light_map.get_light(position) {
                    if light.sunlight() > new_light.sunlight() {
                        new_light.set_sunlight(light);
                    }
                    if light.artificial() > new_light.artificial() {
                        new_light.set_artificial(light.artificial());
                    }
                }
            }
        }

        if let Some(light_attr) = mesh.attribute(ATTRIBUTE_PACKED_BITS_0) {
            let light_attr = match light_attr {
                VertexAttributeValues::Uint32(l) => l,
                _ => unreachable!(),
            };
            if let Some(old_light) = light_attr.get(0) {
                if new_light == Light(*old_light as u8) {
                    continue;
                }
            }
        }

        let len = match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            VertexAttributeValues::Float32x3(positions) => positions.len(),
            _ => unreachable!(),
        };

        let new_light = vec![new_light.0 as u32; len];
        mesh.insert_attribute(ATTRIBUTE_PACKED_BITS_0, new_light);
    }
}

// Gltf's automatically use StandardMaterial, and their meshes are shared between all instances of
// the object. Since the light level is unique to each object, a new mesh needs to be inserted for
// each as well as replacing the material it uses.
fn replace_material_and_mesh(
    mut commands: Commands,
    material_query: Query<
        (Entity, &Handle<StandardMaterial>, &Handle<Mesh>),
        Added<Handle<StandardMaterial>>,
    >,
    standard_materials: Res<Assets<StandardMaterial>>,
    mut pbr_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, PbrLightExtension>>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, standard_handle, mesh_handle) in material_query.iter() {
        let standard_material = standard_materials.get(standard_handle).unwrap();
        let extension_handle = pbr_materials.add(ExtendedMaterial {
            base: standard_material.clone(),
            extension: PbrLightExtension::default(),
        });
        let mut entity_commands = commands.entity(entity);
        entity_commands.remove::<Handle<StandardMaterial>>();
        entity_commands.insert(extension_handle);
        let mesh = meshes.get(mesh_handle).unwrap().clone();
        entity_commands.insert(meshes.add(mesh));
    }
}

#[derive(Default, Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct PbrLightExtension {
    // XXX: This is a useless variable to satisfy the AsBindGroup requirement. Ripped from example
    #[uniform(100)]
    _dummy: u32,
}

impl MaterialExtension for PbrLightExtension {
    fn vertex_shader() -> ShaderRef {
        "src/rendering/shaders/pbr_mesh.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "src/rendering/shaders/pbr.wgsl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayout,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        //let vertex_layout = layout
        //    .get_layout(&[
        //        Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
        //        ATTRIBUTE_PACKED_BITS_0.at_shader_location(2),
        //    ])
        //    .unwrap();
        // I'll probably get bit in the ass for doing this, but I don't want to keep it in sync
        // with changes to StandardMaterial. I have no idea what side effects this might cause, I
        // just did kinda what the bevy code does.
        let index = layout
            .attribute_ids()
            .iter()
            .position(|id| *id == ATTRIBUTE_PACKED_BITS_0.at_shader_location(8).id)
            .unwrap();
        let layout_attribute = layout.layout().attributes[index];
        descriptor.vertex.buffers[0]
            .attributes
            .push(VertexAttribute {
                format: layout_attribute.format,
                offset: layout_attribute.offset,
                shader_location: ATTRIBUTE_PACKED_BITS_0
                    .at_shader_location(8)
                    .shader_location,
            });
        Ok(())
    }
}

// This is an identical copy of bevy's StandardMaterial. Its only difference is adding a packed bit
// field to the vertex attributes so that gltf objects can inherit the light level from the nearest
// block.
//#[derive(Asset, AsBindGroup, Debug, Clone, TypeUuid, TypePath)]
//#[uuid = "e65799f2-923e-4548-8879-be574f9db988"]
//#[bind_group_data(PbrMaterialKey)]
//#[uniform(0, StandardMaterialUniform)]
//pub struct PbrMaterial {
//    /// The color of the surface of the material before lighting.
//    ///
//    /// Doubles as diffuse albedo for non-metallic, specular for metallic and a mix for everything
//    /// in between. If used together with a `base_color_texture`, this is factored into the final
//    /// base color as `base_color * base_color_texture_value`
//    ///
//    /// Defaults to [`Color::WHITE`].
//    pub base_color: Color,
//
//    /// The texture component of the material's color before lighting.
//    /// The actual pre-lighting color is `base_color * this_texture`.
//    ///
//    /// See [`base_color`] for details.
//    ///
//    /// You should set `base_color` to [`Color::WHITE`] (the default)
//    /// if you want the texture to show as-is.
//    ///
//    /// Setting `base_color` to something else than white will tint
//    /// the texture. For example, setting `base_color` to pure red will
//    /// tint the texture red.
//    ///
//    /// [`base_color`]: StandardMaterial::base_color
//    #[texture(1)]
//    #[sampler(2)]
//    pub base_color_texture: Option<Handle<Image>>,
//
//    // Use a color for user friendliness even though we technically don't use the alpha channel
//    // Might be used in the future for exposure correction in HDR
//    /// Color the material "emits" to the camera.
//    ///
//    /// This is typically used for monitor screens or LED lights.
//    /// Anything that can be visible even in darkness.
//    ///
//    /// The emissive color is added to what would otherwise be the material's visible color.
//    /// This means that for a light emissive value, in darkness,
//    /// you will mostly see the emissive component.
//    ///
//    /// The default emissive color is black, which doesn't add anything to the material color.
//    ///
//    /// Note that **an emissive material won't light up surrounding areas like a light source**,
//    /// it just adds a value to the color seen on screen.
//    pub emissive: Color,
//
//    /// The emissive map, multiplies pixels with [`emissive`]
//    /// to get the final "emitting" color of a surface.
//    ///
//    /// This color is multiplied by [`emissive`] to get the final emitted color.
//    /// Meaning that you should set [`emissive`] to [`Color::WHITE`]
//    /// if you want to use the full range of color of the emissive texture.
//    ///
//    /// [`emissive`]: StandardMaterial::emissive
//    #[texture(3)]
//    #[sampler(4)]
//    pub emissive_texture: Option<Handle<Image>>,
//
//    /// Linear perceptual roughness, clamped to `[0.089, 1.0]` in the shader.
//    ///
//    /// Defaults to `0.5`.
//    ///
//    /// Low values result in a "glossy" material with specular highlights,
//    /// while values close to `1` result in rough materials.
//    ///
//    /// If used together with a roughness/metallic texture, this is factored into the final base
//    /// color as `roughness * roughness_texture_value`.
//    ///
//    /// 0.089 is the minimum floating point value that won't be rounded down to 0 in the
//    /// calculations used.
//    //
//    // Technically for 32-bit floats, 0.045 could be used.
//    // See <https://google.github.io/filament/Filament.html#materialsystem/parameterization/>
//    pub perceptual_roughness: f32,
//
//    /// How "metallic" the material appears, within `[0.0, 1.0]`.
//    ///
//    /// This should be set to 0.0 for dielectric materials or 1.0 for metallic materials.
//    /// For a hybrid surface such as corroded metal, you may need to use in-between values.
//    ///
//    /// Defaults to `0.00`, for dielectric.
//    ///
//    /// If used together with a roughness/metallic texture, this is factored into the final base
//    /// color as `metallic * metallic_texture_value`.
//    pub metallic: f32,
//
//    /// Metallic and roughness maps, stored as a single texture.
//    ///
//    /// The blue channel contains metallic values,
//    /// and the green channel contains the roughness values.
//    /// Other channels are unused.
//    ///
//    /// Those values are multiplied by the scalar ones of the material,
//    /// see [`metallic`] and [`perceptual_roughness`] for details.
//    ///
//    /// Note that with the default values of [`metallic`] and [`perceptual_roughness`],
//    /// setting this texture has no effect. If you want to exclusively use the
//    /// `metallic_roughness_texture` values for your material, make sure to set [`metallic`]
//    /// and [`perceptual_roughness`] to `1.0`.
//    ///
//    /// [`metallic`]: StandardMaterial::metallic
//    /// [`perceptual_roughness`]: StandardMaterial::perceptual_roughness
//    #[texture(5)]
//    #[sampler(6)]
//    pub metallic_roughness_texture: Option<Handle<Image>>,
//
//    /// Specular intensity for non-metals on a linear scale of `[0.0, 1.0]`.
//    ///
//    /// Use the value as a way to control the intensity of the
//    /// specular highlight of the material, i.e. how reflective is the material,
//    /// rather than the physical property "reflectance."
//    ///
//    /// Set to `0.0`, no specular highlight is visible, the highlight is strongest
//    /// when `reflectance` is set to `1.0`.
//    ///
//    /// Defaults to `0.5` which is mapped to 4% reflectance in the shader.
//    #[doc(alias = "specular_intensity")]
//    pub reflectance: f32,
//
//    /// Used to fake the lighting of bumps and dents on a material.
//    ///
//    /// A typical usage would be faking cobblestones on a flat plane mesh in 3D.
//    ///
//    /// # Notes
//    ///
//    /// Normal mapping with `StandardMaterial` and the core bevy PBR shaders requires:
//    /// - A normal map texture
//    /// - Vertex UVs
//    /// - Vertex tangents
//    /// - Vertex normals
//    ///
//    /// Tangents do not have to be stored in your model,
//    /// they can be generated using the [`Mesh::generate_tangents`] method.
//    /// If your material has a normal map, but still renders as a flat surface,
//    /// make sure your meshes have their tangents set.
//    ///
//    /// [`Mesh::generate_tangents`]: bevy_render::mesh::Mesh::generate_tangents
//    #[texture(9)]
//    #[sampler(10)]
//    pub normal_map_texture: Option<Handle<Image>>,
//
//    /// Normal map textures authored for DirectX have their y-component flipped. Set this to flip
//    /// it to right-handed conventions.
//    pub flip_normal_map_y: bool,
//
//    /// Specifies the level of exposure to ambient light.
//    ///
//    /// This is usually generated and stored automatically ("baked") by 3D-modelling software.
//    ///
//    /// Typically, steep concave parts of a model (such as the armpit of a shirt) are darker,
//    /// because they have little exposure to light.
//    /// An occlusion map specifies those parts of the model that light doesn't reach well.
//    ///
//    /// The material will be less lit in places where this texture is dark.
//    /// This is similar to ambient occlusion, but built into the model.
//    #[texture(7)]
//    #[sampler(8)]
//    pub occlusion_texture: Option<Handle<Image>>,
//
//    /// Support two-sided lighting by automatically flipping the normals for "back" faces
//    /// within the PBR lighting shader.
//    ///
//    /// Defaults to `false`.
//    /// This does not automatically configure backface culling,
//    /// which can be done via `cull_mode`.
//    pub double_sided: bool,
//
//    /// Whether to cull the "front", "back" or neither side of a mesh.
//    /// If set to `None`, the two sides of the mesh are visible.
//    ///
//    /// Defaults to `Some(Face::Back)`.
//    /// In bevy, the order of declaration of a triangle's vertices
//    /// in [`Mesh`] defines the triangle's front face.
//    ///
//    /// When a triangle is in a viewport,
//    /// if its vertices appear counter-clockwise from the viewport's perspective,
//    /// then the viewport is seeing the triangle's front face.
//    /// Conversely, if the vertices appear clockwise, you are seeing the back face.
//    ///
//    /// In short, in bevy, front faces winds counter-clockwise.
//    ///
//    /// Your 3D editing software should manage all of that.
//    ///
//    /// [`Mesh`]: bevy_render::mesh::Mesh
//    // TODO: include this in reflection somehow (maybe via remote types like serde https://serde.rs/remote-derive.html)
//    pub cull_mode: Option<Face>,
//
//    /// Whether to apply only the base color to this material.
//    ///
//    /// Normals, occlusion textures, roughness, metallic, reflectance, emissive,
//    /// shadows, alpha mode and ambient light are ignored if this is set to `true`.
//    pub unlit: bool,
//
//    /// Whether to enable fog for this material.
//    pub fog_enabled: bool,
//
//    /// How to apply the alpha channel of the `base_color_texture`.
//    ///
//    /// See [`AlphaMode`] for details. Defaults to [`AlphaMode::Opaque`].
//    pub alpha_mode: AlphaMode,
//
//    /// Adjust rendered depth.
//    ///
//    /// A material with a positive depth bias will render closer to the
//    /// camera while negative values cause the material to render behind
//    /// other objects. This is independent of the viewport.
//    ///
//    /// `depth_bias` affects render ordering and depth write operations
//    /// using the `wgpu::DepthBiasState::Constant` field.
//    ///
//    /// [z-fighting]: https://en.wikipedia.org/wiki/Z-fighting
//    pub depth_bias: f32,
//
//    /// The depth map used for [parallax mapping].
//    ///
//    /// It is a greyscale image where white represents bottom and black the top.
//    /// If this field is set, bevy will apply [parallax mapping].
//    /// Parallax mapping, unlike simple normal maps, will move the texture
//    /// coordinate according to the current perspective,
//    /// giving actual depth to the texture.
//    ///
//    /// The visual result is similar to a displacement map,
//    /// but does not require additional geometry.
//    ///
//    /// Use the [`parallax_depth_scale`] field to control the depth of the parallax.
//    ///
//    /// ## Limitations
//    ///
//    /// - It will look weird on bent/non-planar surfaces.
//    /// - The depth of the pixel does not reflect its visual position, resulting
//    ///   in artifacts for depth-dependent features such as fog or SSAO.
//    /// - For the same reason, the the geometry silhouette will always be
//    ///   the one of the actual geometry, not the parallaxed version, resulting
//    ///   in awkward looks on intersecting parallaxed surfaces.
//    ///
//    /// ## Performance
//    ///
//    /// Parallax mapping requires multiple texture lookups, proportional to
//    /// [`max_parallax_layer_count`], which might be costly.
//    ///
//    /// Use the [`parallax_mapping_method`] and [`max_parallax_layer_count`] fields
//    /// to tweak the shader, trading graphical quality for performance.
//    ///
//    /// To improve performance, set your `depth_map`'s [`Image::sampler_descriptor`]
//    /// filter mode to `FilterMode::Nearest`, as [this paper] indicates, it improves
//    /// performance a bit.
//    ///
//    /// To reduce artifacts, avoid steep changes in depth, blurring the depth
//    /// map helps with this.
//    ///
//    /// Larger depth maps haves a disproportionate performance impact.
//    ///
//    /// [this paper]: https://www.diva-portal.org/smash/get/diva2:831762/FULLTEXT01.pdf
//    /// [parallax mapping]: https://en.wikipedia.org/wiki/Parallax_mapping
//    /// [`parallax_depth_scale`]: StandardMaterial::parallax_depth_scale
//    /// [`parallax_mapping_method`]: StandardMaterial::parallax_mapping_method
//    /// [`max_parallax_layer_count`]: StandardMaterial::max_parallax_layer_count
//    #[texture(11)]
//    #[sampler(12)]
//    pub depth_map: Option<Handle<Image>>,
//
//    /// How deep the offset introduced by the depth map should be.
//    ///
//    /// Default is `0.1`, anything over that value may look distorted.
//    /// Lower values lessen the effect.
//    ///
//    /// The depth is relative to texture size. This means that if your texture
//    /// occupies a surface of `1` world unit, and `parallax_depth_scale` is `0.1`, then
//    /// the in-world depth will be of `0.1` world units.
//    /// If the texture stretches for `10` world units, then the final depth
//    /// will be of `1` world unit.
//    pub parallax_depth_scale: f32,
//
//    /// Which parallax mapping method to use.
//    ///
//    /// We recommend that all objects use the same [`ParallaxMappingMethod`], to avoid
//    /// duplicating and running two shaders.
//    pub parallax_mapping_method: ParallaxMappingMethod,
//
//    /// In how many layers to split the depth maps for parallax mapping.
//    ///
//    /// If you are seeing jaggy edges, increase this value.
//    /// However, this incurs a performance cost.
//    ///
//    /// Dependent on the situation, switching to [`ParallaxMappingMethod::Relief`]
//    /// and keeping this value low might have better performance than increasing the
//    /// layer count while using [`ParallaxMappingMethod::Occlusion`].
//    ///
//    /// Default is `16.0`.
//    pub max_parallax_layer_count: f32,
//}
//
//impl From<&StandardMaterial> for PbrMaterial {
//    fn from(material: &StandardMaterial) -> Self {
//        Self {
//            base_color: material.base_color,
//            base_color_texture: material.base_color_texture.clone(),
//            emissive: material.emissive,
//            emissive_texture: material.emissive_texture.clone(),
//            perceptual_roughness: material.perceptual_roughness,
//            metallic: material.metallic,
//            metallic_roughness_texture: material.metallic_roughness_texture.clone(),
//            reflectance: material.reflectance,
//            normal_map_texture: material.normal_map_texture.clone(),
//            flip_normal_map_y: material.flip_normal_map_y,
//            occlusion_texture: material.occlusion_texture.clone(),
//            double_sided: material.double_sided,
//            cull_mode: material.cull_mode,
//            unlit: material.unlit,
//            fog_enabled: material.fog_enabled,
//            alpha_mode: material.alpha_mode,
//            depth_bias: material.depth_bias,
//            depth_map: material.depth_map.clone(),
//            max_parallax_layer_count: material.max_parallax_layer_count,
//            parallax_depth_scale: material.parallax_depth_scale,
//            parallax_mapping_method: material.parallax_mapping_method,
//        }
//    }
//}
//
//#[derive(Clone, PartialEq, Eq, Hash)]
//pub struct PbrMaterialKey {
//    normal_map: bool,
//    cull_mode: Option<Face>,
//    depth_bias: i32,
//    relief_mapping: bool,
//}
//
//impl From<&PbrMaterial> for PbrMaterialKey {
//    fn from(material: &PbrMaterial) -> Self {
//        PbrMaterialKey {
//            normal_map: material.normal_map_texture.is_some(),
//            cull_mode: material.cull_mode,
//            depth_bias: material.depth_bias as i32,
//            relief_mapping: matches!(
//                material.parallax_mapping_method,
//                ParallaxMappingMethod::Relief { .. }
//            ),
//        }
//    }
//}
//
//impl Material for PbrMaterial {
//    fn specialize(
//        _pipeline: &MaterialPipeline<Self>,
//        descriptor: &mut RenderPipelineDescriptor,
//        layout: &MeshVertexBufferLayout,
//        key: MaterialPipelineKey<Self>,
//    ) -> Result<(), SpecializedMeshPipelineError> {
//        let mut shader_defs = Vec::new();
//        let mut vertex_attributes = Vec::new();
//
//        if layout.contains(Mesh::ATTRIBUTE_POSITION) {
//            shader_defs.push("VERTEX_POSITIONS".into());
//            vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));
//        }
//
//        if layout.contains(Mesh::ATTRIBUTE_NORMAL) {
//            shader_defs.push("VERTEX_NORMALS".into());
//            vertex_attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(1));
//        }
//
//        if layout.contains(Mesh::ATTRIBUTE_UV_0) {
//            shader_defs.push("VERTEX_UVS".into());
//            vertex_attributes.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(2));
//        }
//
//        if layout.contains(Mesh::ATTRIBUTE_TANGENT) {
//            shader_defs.push("VERTEX_TANGENTS".into());
//            vertex_attributes.push(Mesh::ATTRIBUTE_TANGENT.at_shader_location(3));
//        }
//
//        if layout.contains(Mesh::ATTRIBUTE_COLOR) {
//            shader_defs.push("VERTEX_COLORS".into());
//            vertex_attributes.push(Mesh::ATTRIBUTE_COLOR.at_shader_location(4));
//        }
//
//        shader_defs.push(ShaderDefVal::UInt(
//            "MAX_DIRECTIONAL_LIGHTS".to_string(),
//            MAX_DIRECTIONAL_LIGHTS as u32,
//        ));
//        shader_defs.push(ShaderDefVal::UInt(
//            "MAX_CASCADES_PER_LIGHT".to_string(),
//            MAX_CASCADES_PER_LIGHT as u32,
//        ));
//
//        vertex_attributes.push(ATTRIBUTE_PACKED_BITS_0.at_shader_location(7));
//
//        let vertex_buffer_layout = layout.get_layout(&vertex_attributes)?;
//
//        descriptor.vertex.buffers = vec![vertex_buffer_layout];
//        descriptor.vertex.shader_defs = shader_defs.clone();
//        descriptor.fragment.as_mut().unwrap().shader_defs = shader_defs;
//
//        if key.bind_group_data.normal_map {
//            if let Some(fragment) = descriptor.fragment.as_mut() {
//                fragment
//                    .shader_defs
//                    .push("STANDARDMATERIAL_NORMAL_MAP".into());
//            }
//        }
//
//        descriptor.primitive.cull_mode = key.bind_group_data.cull_mode;
//
//        if let Some(label) = &mut descriptor.label {
//            *label = format!("pbr_{}", *label).into();
//        }
//        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
//            depth_stencil.bias.constant = key.bind_group_data.depth_bias;
//        }
//        return Ok(());
//    }
//
//    fn vertex_shader() -> ShaderRef {
//        "src/rendering/shaders/pbr_mesh.wgsl".into()
//    }
//
//    fn fragment_shader() -> ShaderRef {
//        "src/rendering/shaders/pbr.wgsl".into()
//    }
//
//    #[inline]
//    fn alpha_mode(&self) -> AlphaMode {
//        self.alpha_mode
//    }
//
//    #[inline]
//    fn depth_bias(&self) -> f32 {
//        self.depth_bias
//    }
//}
//
//impl AsBindGroupShaderType<StandardMaterialUniform> for PbrMaterial {
//    fn as_bind_group_shader_type(&self, images: &RenderAssets<Image>) -> StandardMaterialUniform {
//        let mut flags = StandardMaterialFlags::NONE;
//        if self.base_color_texture.is_some() {
//            flags |= StandardMaterialFlags::BASE_COLOR_TEXTURE;
//        }
//        if self.emissive_texture.is_some() {
//            flags |= StandardMaterialFlags::EMISSIVE_TEXTURE;
//        }
//        if self.metallic_roughness_texture.is_some() {
//            flags |= StandardMaterialFlags::METALLIC_ROUGHNESS_TEXTURE;
//        }
//        if self.occlusion_texture.is_some() {
//            flags |= StandardMaterialFlags::OCCLUSION_TEXTURE;
//        }
//        if self.double_sided {
//            flags |= StandardMaterialFlags::DOUBLE_SIDED;
//        }
//        if self.unlit {
//            flags |= StandardMaterialFlags::UNLIT;
//        }
//        if self.fog_enabled {
//            flags |= StandardMaterialFlags::FOG_ENABLED;
//        }
//        if self.depth_map.is_some() {
//            flags |= StandardMaterialFlags::DEPTH_MAP;
//        }
//        let has_normal_map = self.normal_map_texture.is_some();
//        if has_normal_map {
//            if let Some(texture) = images.get(self.normal_map_texture.as_ref().unwrap()) {
//                match texture.texture_format {
//                    // All 2-component unorm formats
//                    TextureFormat::Rg8Unorm
//                    | TextureFormat::Rg16Unorm
//                    | TextureFormat::Bc5RgUnorm
//                    | TextureFormat::EacRg11Unorm => {
//                        flags |= StandardMaterialFlags::TWO_COMPONENT_NORMAL_MAP;
//                    }
//                    _ => {}
//                }
//            }
//            if self.flip_normal_map_y {
//                flags |= StandardMaterialFlags::FLIP_NORMAL_MAP_Y;
//            }
//        }
//        // NOTE: 0.5 is from the glTF default - do we want this?
//        let mut alpha_cutoff = 0.5;
//        match self.alpha_mode {
//            AlphaMode::Opaque => flags |= StandardMaterialFlags::ALPHA_MODE_OPAQUE,
//            AlphaMode::Mask(c) => {
//                alpha_cutoff = c;
//                flags |= StandardMaterialFlags::ALPHA_MODE_MASK;
//            }
//            AlphaMode::Blend => flags |= StandardMaterialFlags::ALPHA_MODE_BLEND,
//            AlphaMode::Premultiplied => flags |= StandardMaterialFlags::ALPHA_MODE_PREMULTIPLIED,
//            AlphaMode::Add => flags |= StandardMaterialFlags::ALPHA_MODE_ADD,
//            AlphaMode::Multiply => flags |= StandardMaterialFlags::ALPHA_MODE_MULTIPLY,
//        };
//
//        StandardMaterialUniform {
//            base_color: self.base_color.as_linear_rgba_f32().into(),
//            emissive: self.emissive.as_linear_rgba_f32().into(),
//            roughness: self.perceptual_roughness,
//            metallic: self.metallic,
//            reflectance: self.reflectance,
//            flags: flags.bits(),
//            alpha_cutoff,
//            parallax_depth_scale: self.parallax_depth_scale,
//            max_parallax_layer_count: self.max_parallax_layer_count,
//            max_relief_mapping_search_steps: match self.parallax_mapping_method {
//                ParallaxMappingMethod::Occlusion => 0,
//                ParallaxMappingMethod::Relief { max_steps } => max_steps,
//            },
//        }
//    }
//}
