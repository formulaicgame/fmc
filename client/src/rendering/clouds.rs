use bevy::{
    asset::{load_internal_asset, weak_handle, RenderAssetUsages, UntypedAssetId},
    core_pipeline::{
        core_3d::{
            graph::{Core3d, Node3d},
            CORE_3D_DEPTH_FORMAT,
        },
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
        oit::{
            resolve::node::OitResolvePass,
            {OitBuffers, OrderIndependentTransparencySettings},
        },
        prepass::ViewPrepassTextures,
    },
    ecs::{
        component::Tick,
        query::QueryItem,
        system::{lifetimeless::SRes, SystemParamItem},
    },
    math::primitives::{Cuboid, Sphere},
    math::FloatOrd,
    pbr::{
        DrawMesh, FogMeta, GpuFog, GpuLights, LightMeta, MeshInputUniform, MeshPipeline,
        MeshPipelineKey, MeshPipelineViewLayoutKey, MeshUniform, RenderMeshInstances,
        SetMeshBindGroup, SetMeshViewBindGroup, ViewFogUniformOffset, ViewLightsUniformOffset,
    },
    platform::collections::HashSet,
    prelude::*,
    render::{
        batching::{
            gpu_preprocessing::{
                batch_and_prepare_sorted_render_phase, GpuPreprocessingMode,
                GpuPreprocessingSupport, IndirectParametersCpuMetadata,
                UntypedPhaseIndirectParametersBuffers,
            },
            GetBatchData, GetFullBatchData,
        },
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        mesh::{
            allocator::{MeshAllocator, SlabId},
            MeshVertexBufferLayoutRef, RenderMesh,
        },
        render_asset::RenderAssets,
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_phase::{
            AddRenderCommand, BinnedPhaseItem, BinnedRenderPhasePlugin, BinnedRenderPhaseType,
            CachedRenderPipelinePhaseItem, DrawFunctionId, DrawFunctions, PhaseItem,
            PhaseItemBatchSetKey, PhaseItemExtraIndex, SetItemPipeline, ViewBinnedRenderPhases,
        },
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        sync_world::MainEntity,
        texture::{CachedTexture, TextureCache},
        view::{
            ExtractedView, NoIndirectDrawing, RenderVisibleEntities, RetainedViewEntity,
            ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
        },
        Extract, Render, RenderApp, RenderDebugFlags, RenderSet,
    },
    tasks::Task,
};
use nonmax::NonMaxU32;
use std::ops::Range;

use crate::{
    assets::AssetState,
    game_state::GameState,
    player::Player,
    world::{world_map::chunk::Chunk, Origin},
};

pub const CLOUD_PREPASS_SHADER: Handle<Shader> =
    weak_handle!("a206b28c-44ed-49eb-bddd-4ce631db920f");
pub const CLOUD_POST_PROCESS_SHADER: Handle<Shader> =
    weak_handle!("fb9a8e52-d5a6-458a-9c10-62e78154d5c3");

pub struct CloudPlugin;
impl Plugin for CloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PostProcessPlugin, CloudPhasePlugin))
            .add_systems(OnEnter(AssetState::Loading), setup)
            .add_systems(OnEnter(GameState::Launcher), cleanup)
            .add_systems(Update, move_clouds.run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component, Default)]
struct Clouds {
    origin_shift: IVec3,
    task: Option<Task<Vec<Transform>>>,
}

impl Clouds {
    fn hash(values: &[i32]) -> f32 {
        let mut hash: u32 = 11;
        for value in values {
            hash = hash.wrapping_add(*value as u32);
            hash = hash.wrapping_add(hash << 10);
            hash ^= hash >> 6;
        }
        hash = hash.wrapping_add(hash << 3);
        hash ^= hash >> 11;
        hash = hash.wrapping_add(hash << 15);

        // Only want 23 bits of the result for the mantissa, rest is discarded and replaced
        // with exponent of 127 so the result is in range 1..2 then -1 to move the range down
        // to 0..1
        return f32::from_bits((hash >> 9) | (127 << 23)) - 1.0;
    }

    fn generate(distance: f32, origin: IVec3) -> Vec<Transform> {
        // Spacing is in block units
        let spacing: i32 = 10;
        // Make it repeat every 1 million blocks to avoid precision issues.
        let origin_x = origin.x % 1000000;
        let origin_z = origin.z % 1000000;
        let offset_x = origin_x % spacing;
        let offset_z = origin_z % spacing;
        let grid_x = origin_x / spacing;
        let grid_z = origin_z / spacing;

        let cells = (distance as i32 / spacing) as usize;

        let noise = fmc_noise::Noise::simplex(0.05).fbm(3, 0.5, 2.0);
        let (samples, _, _) = noise.generate_2d(grid_x as f32, grid_z as f32, cells, cells);

        let mut clouds = Vec::with_capacity(cells * cells);
        for x in 0..cells {
            for z in 0..cells {
                let noise = samples[x * cells + z];

                if noise < 0.0 {
                    continue;
                }

                let mut x_position =
                    (-distance / 2.0) + x as f32 * spacing as f32 - offset_x as f32;
                let mut z_position =
                    (-distance / 2.0) + z as f32 * spacing as f32 - offset_z as f32;
                let y_position = 60.0 + 32.0 * noise - origin.y as f32;

                // Uniformly remove ~50% of the clouds
                let sparsity = 0.8;
                if Self::hash(&[grid_x + x as i32, grid_z + z as i32]) <= sparsity {
                    continue;
                }

                // Offset for variety
                let seed = noise.to_bits() as u64;
                let mut rng = crate::utils::Rng::new(seed);

                x_position += (rng.next_f32() - 0.5) * 2.0;
                z_position += (rng.next_f32() - 0.5) * 2.0;

                let width = 2.5 + 3.5 * rng.next_f32();
                let scale = Vec3::new(width, 1.0, width);

                clouds.push(
                    Transform::from_xyz(x_position, y_position, z_position).with_scale(scale),
                );
            }
        }

        return clouds;
    }
}

fn setup(
    mut commands: Commands,
    origin: Res<Origin>,
    player_query: Query<Entity, With<Player>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Clouds::default(),
        Transform::default(),
        Visibility::default(),
    ));
}

fn cleanup(mut commands: Commands, cloud_query: Query<Entity, With<Clouds>>) {
    if let Ok(entity) = cloud_query.single() {
        commands.entity(entity).despawn();
    }
}

fn move_clouds(
    mut commands: Commands,
    origin: Res<Origin>,
    time: Res<Time>,
    mut clouds: Query<(Entity, &mut Transform, &mut Clouds)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (entity, mut transform, mut clouds) = clouds.single_mut().unwrap();
    transform.translation.z += time.delta_secs() * 1.5;
    let origin_shift = transform.translation.as_ivec3() / IVec3::splat(Chunk::SIZE as i32);
    if origin_shift != IVec3::ZERO || origin.is_changed() {
        clouds.origin_shift += origin_shift * Chunk::SIZE as i32;
        transform.translation %= Vec3::splat(Chunk::SIZE as f32);

        let mut ecs = commands.entity(entity);
        ecs.despawn_related::<Children>();

        let mut mesh = Cuboid::new(5.0, 5.0, 5.0).mesh().build();
        mesh.asset_usage = RenderAssetUsages::RENDER_WORLD;
        let mesh_handle = Mesh3d(meshes.add(mesh));

        ecs.with_children(|parent| {
            for mut transform in Clouds::generate(32.0 * 16.0, origin.0 - clouds.origin_shift) {
                parent.spawn((
                    mesh_handle.clone(),
                    transform,
                    Cloud,
                    crate::world::MovesWithOrigin,
                ));
            }
        });
    }
}

#[derive(Component, ExtractComponent, Clone, Copy, Default)]
struct Cloud;

pub struct CloudPhasePlugin;
impl Plugin for CloudPhasePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<Cloud>::default(),
            BinnedRenderPhasePlugin::<Cloud3d, MeshPipeline>::new(RenderDebugFlags::default()),
        ));
        // We need to get the render app from the main app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<SpecializedMeshPipelines<CloudPipeline>>()
            .init_resource::<DrawFunctions<Cloud3d>>()
            .add_render_command::<Cloud3d, DrawCloud3d>()
            .init_resource::<ViewBinnedRenderPhases<Cloud3d>>()
            .add_systems(ExtractSchedule, extract_camera_phases)
            .add_systems(
                Render,
                (
                    prepare_cloud_texture.in_set(RenderSet::PrepareResources),
                    queue_custom_meshes.in_set(RenderSet::QueueMeshes),
                ),
            );

        render_app
            .add_render_graph_node::<ViewNodeRunner<CustomDrawNode>>(Core3d, CustomDrawPassLabel)
            // Tell the node to run after the main pass
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::MainOpaquePass,
                    CustomDrawPassLabel,
                    PostProcessLabel,
                ),
            );

        load_internal_asset!(
            app,
            CLOUD_PREPASS_SHADER,
            "shaders/clouds.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            CLOUD_POST_PROCESS_SHADER,
            "shaders/clouds_post.wgsl",
            Shader::from_wgsl
        );
    }

    fn finish(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        // The pipeline needs the RenderDevice to be created and it's only available once plugins
        // are initialized
        render_app.init_resource::<CloudPipeline>();
    }
}

#[derive(Resource)]
struct CloudPipeline {
    mesh_pipeline: MeshPipeline,
}

impl FromWorld for CloudPipeline {
    fn from_world(world: &mut World) -> Self {
        Self {
            mesh_pipeline: MeshPipeline::from_world(world),
        }
    }
}

// For more information on how SpecializedMeshPipeline work, please look at the
// specialized_mesh_pipeline example
impl SpecializedMeshPipeline for CloudPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        // We will only use the position of the mesh in our shader so we only need to specify that
        let mut vertex_attributes = Vec::new();
        if layout.0.contains(Mesh::ATTRIBUTE_POSITION) {
            // Make sure this matches the shader location
            vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));
        }
        // This will automatically generate the correct `VertexBufferLayout` based on the vertex attributes
        let vertex_buffer_layout = layout.0.get_layout(&vertex_attributes)?;

        Ok(RenderPipelineDescriptor {
            label: Some("Cloud Mesh Pipeline".into()),
            // We want to reuse the data from bevy so we use the same bind groups as the default
            // mesh pipeline
            layout: vec![
                // Bind group 0 is the view uniform
                self.mesh_pipeline
                    .get_view_layout(MeshPipelineViewLayoutKey::from(key))
                    .clone(),
                // Bind group 1 is the mesh uniform
                self.mesh_pipeline.mesh_layouts.model_only.clone(),
            ],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: CLOUD_PREPASS_SHADER.clone(),
                shader_defs: vec![],
                entry_point: "vertex".into(),
                buffers: vec![vertex_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: CLOUD_PREPASS_SHADER.clone(),
                shader_defs: vec![],
                entry_point: "fragment".into(),
                targets: vec![
                    // Clouds coverage
                    Some(ColorTargetState {
                        format: TextureFormat::R32Float,
                        blend: Some(BlendState {
                            color: BlendComponent {
                                src_factor: BlendFactor::One,
                                dst_factor: BlendFactor::One,
                                // Adds up the alpha of each cloud layer
                                operation: BlendOperation::Add,
                            },
                            alpha: BlendComponent::REPLACE,
                        }),
                        write_mask: ColorWrites::ALL,
                    }),
                    // Cloud depth
                    Some(ColorTargetState {
                        format: TextureFormat::R32Float,
                        blend: Some(BlendState {
                            color: BlendComponent {
                                src_factor: BlendFactor::One,
                                dst_factor: BlendFactor::One,
                                operation: BlendOperation::Max,
                            },
                            alpha: BlendComponent::REPLACE,
                        }),
                        write_mask: ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: PrimitiveState {
                topology: key.primitive_topology(),
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Greater,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            // It's generally recommended to specialize your pipeline for MSAA,
            // but it's not always possible
            multisample: MultisampleState::default(),
            zero_initialize_workgroup_memory: false,
        })
    }
}

// We will reuse render commands already defined by bevy to draw a 3d mesh
type DrawCloud3d = (
    SetItemPipeline,
    // This will set the view bindings in group 0
    SetMeshViewBindGroup<0>,
    // This will set the mesh bindings in group 1
    SetMeshBindGroup<1>,
    // This will draw the mesh
    DrawMesh,
);

struct Cloud3d {
    /// Determines which objects can be placed into a *batch set*.
    ///
    /// Objects in a single batch set can potentially be multi-drawn together,
    /// if it's enabled and the current platform supports it.
    pub batch_set_key: Cloud3dBatchSetKey,
    /// The key, which determines which can be batched.
    pub bin_key: Cloud3dBinKey,
    /// An entity from which data will be fetched, including the mesh if
    /// applicable.
    pub representative_entity: (Entity, MainEntity),
    /// The ranges of instances.
    pub batch_range: Range<u32>,
    /// An extra index, which is either a dynamic offset or an index in the
    /// indirect parameters list.
    pub extra_index: PhaseItemExtraIndex,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cloud3dBatchSetKey {
    /// The identifier of the render pipeline.
    pub pipeline: CachedRenderPipelineId,

    /// The function used to draw.
    pub draw_function: DrawFunctionId,

    /// The ID of the slab of GPU memory that contains vertex data.
    ///
    /// For non-mesh items, you can fill this with 0 if your items can be
    /// multi-drawn, or with a unique value if they can't.
    pub vertex_slab: SlabId,

    /// The ID of the slab of GPU memory that contains index data, if present.
    ///
    /// For non-mesh items, you can safely fill this with `None`.
    pub index_slab: Option<SlabId>,
}

impl PhaseItemBatchSetKey for Cloud3dBatchSetKey {
    fn indexed(&self) -> bool {
        self.index_slab.is_some()
    }
}

/// Data that must be identical in order to *batch* phase items together.
///
/// Note that a *batch set* (if multi-draw is in use) contains multiple batches.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cloud3dBinKey {
    /// The asset that this phase item is associated with.
    ///
    /// Normally, this is the ID of the mesh, but for non-mesh items it might be
    /// the ID of another type of asset.
    pub asset_id: UntypedAssetId,
}

impl PhaseItem for Cloud3d {
    #[inline]
    fn entity(&self) -> Entity {
        self.representative_entity.0
    }

    #[inline]
    fn main_entity(&self) -> MainEntity {
        self.representative_entity.1
    }

    #[inline]
    fn draw_function(&self) -> DrawFunctionId {
        self.batch_set_key.draw_function
    }

    #[inline]
    fn batch_range(&self) -> &Range<u32> {
        &self.batch_range
    }

    #[inline]
    fn batch_range_mut(&mut self) -> &mut Range<u32> {
        &mut self.batch_range
    }

    fn extra_index(&self) -> PhaseItemExtraIndex {
        self.extra_index.clone()
    }

    fn batch_range_and_extra_index_mut(&mut self) -> (&mut Range<u32>, &mut PhaseItemExtraIndex) {
        (&mut self.batch_range, &mut self.extra_index)
    }
}

impl BinnedPhaseItem for Cloud3d {
    type BatchSetKey = Cloud3dBatchSetKey;
    type BinKey = Cloud3dBinKey;

    #[inline]
    fn new(
        batch_set_key: Self::BatchSetKey,
        bin_key: Self::BinKey,
        representative_entity: (Entity, MainEntity),
        batch_range: Range<u32>,
        extra_index: PhaseItemExtraIndex,
    ) -> Self {
        Cloud3d {
            batch_set_key,
            bin_key,
            representative_entity,
            batch_range,
            extra_index,
        }
    }
}

impl CachedRenderPipelinePhaseItem for Cloud3d {
    #[inline]
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.batch_set_key.pipeline
    }
}

impl GetBatchData for CloudPipeline {
    type Param = (
        SRes<RenderMeshInstances>,
        SRes<RenderAssets<RenderMesh>>,
        SRes<MeshAllocator>,
    );
    type CompareData = AssetId<Mesh>;
    type BufferData = MeshUniform;

    fn get_batch_data(
        (mesh_instances, _render_assets, mesh_allocator): &SystemParamItem<Self::Param>,
        (_entity, main_entity): (Entity, MainEntity),
    ) -> Option<(Self::BufferData, Option<Self::CompareData>)> {
        let RenderMeshInstances::CpuBuilding(ref mesh_instances) = **mesh_instances else {
            error!(
                "`get_batch_data` should never be called in GPU mesh uniform \
                building mode"
            );
            return None;
        };
        let mesh_instance = mesh_instances.get(&main_entity)?;
        let first_vertex_index =
            match mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id) {
                Some(mesh_vertex_slice) => mesh_vertex_slice.range.start,
                None => 0,
            };
        let mesh_uniform = {
            let mesh_transforms = &mesh_instance.transforms;
            let (local_from_world_transpose_a, local_from_world_transpose_b) =
                mesh_transforms.world_from_local.inverse_transpose_3x3();
            MeshUniform {
                world_from_local: mesh_transforms.world_from_local.to_transpose(),
                previous_world_from_local: mesh_transforms.previous_world_from_local.to_transpose(),
                lightmap_uv_rect: UVec2::ZERO,
                local_from_world_transpose_a,
                local_from_world_transpose_b,
                flags: mesh_transforms.flags,
                first_vertex_index,
                current_skin_index: u32::MAX,
                material_and_lightmap_bind_group_slot: 0,
                tag: 0,
                pad: 0,
            }
        };
        Some((mesh_uniform, None))
    }
}

impl GetFullBatchData for CloudPipeline {
    type BufferInputData = MeshInputUniform;

    fn get_index_and_compare_data(
        (mesh_instances, _, _): &SystemParamItem<Self::Param>,
        main_entity: MainEntity,
    ) -> Option<(NonMaxU32, Option<Self::CompareData>)> {
        // This should only be called during GPU building.
        let RenderMeshInstances::GpuBuilding(ref mesh_instances) = **mesh_instances else {
            error!(
                "`get_index_and_compare_data` should never be called in CPU mesh uniform building \
                mode"
            );
            return None;
        };
        let mesh_instance = mesh_instances.get(&main_entity)?;
        Some((
            mesh_instance.current_uniform_index,
            mesh_instance
                .should_batch()
                .then_some(mesh_instance.mesh_asset_id),
        ))
    }

    fn get_binned_batch_data(
        (mesh_instances, _render_assets, mesh_allocator): &SystemParamItem<Self::Param>,
        main_entity: MainEntity,
    ) -> Option<Self::BufferData> {
        let RenderMeshInstances::CpuBuilding(ref mesh_instances) = **mesh_instances else {
            error!(
                "`get_binned_batch_data` should never be called in GPU mesh uniform building mode"
            );
            return None;
        };
        let mesh_instance = mesh_instances.get(&main_entity)?;
        let first_vertex_index =
            match mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id) {
                Some(mesh_vertex_slice) => mesh_vertex_slice.range.start,
                None => 0,
            };

        Some(MeshUniform::new(
            &mesh_instance.transforms,
            first_vertex_index,
            mesh_instance.material_bindings_index.slot,
            None,
            None,
            None,
        ))
    }

    fn write_batch_indirect_parameters_metadata(
        indexed: bool,
        base_output_index: u32,
        batch_set_index: Option<NonMaxU32>,
        indirect_parameters_buffers: &mut UntypedPhaseIndirectParametersBuffers,
        indirect_parameters_offset: u32,
    ) {
        // Note that `IndirectParameters` covers both of these structures, even
        // though they actually have distinct layouts. See the comment above that
        // type for more information.
        let indirect_parameters = IndirectParametersCpuMetadata {
            base_output_index,
            batch_set_index: match batch_set_index {
                None => !0,
                Some(batch_set_index) => u32::from(batch_set_index),
            },
        };

        if indexed {
            indirect_parameters_buffers
                .indexed
                .set(indirect_parameters_offset, indirect_parameters);
        } else {
            indirect_parameters_buffers
                .non_indexed
                .set(indirect_parameters_offset, indirect_parameters);
        }
    }

    fn get_binned_index(
        _param: &SystemParamItem<Self::Param>,
        _query_item: MainEntity,
    ) -> Option<NonMaxU32> {
        None
    }
}

// When defining a phase, we need to extract it from the main world and add it to a resource
// that will be used by the render world. We need to give that resource all views that will use
// that phase
fn extract_camera_phases(
    mut cloud_phases: ResMut<ViewBinnedRenderPhases<Cloud3d>>,
    cameras: Extract<Query<(Entity, &Camera, Has<NoIndirectDrawing>), With<Camera3d>>>,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    live_entities.clear();
    for (main_entity, camera, no_indirect_drawing) in &cameras {
        if !camera.is_active {
            continue;
        }

        let gpu_preprocessing_mode = gpu_preprocessing_support.min(if !no_indirect_drawing {
            GpuPreprocessingMode::Culling
        } else {
            GpuPreprocessingMode::PreprocessingOnly
        });

        // This is the main camera, so we use the first subview index (0)
        let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);

        cloud_phases.prepare_for_new_frame(retained_view_entity, gpu_preprocessing_mode);

        live_entities.insert(retained_view_entity);
    }

    // Clear out all dead views.
    cloud_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
}

// This is a very important step when writing a custom phase.
//
// This system determines which meshes will be added to the phase.
fn queue_custom_meshes(
    custom_draw_functions: Res<DrawFunctions<Cloud3d>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CloudPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    custom_draw_pipeline: Res<CloudPipeline>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    mesh_allocator: Res<MeshAllocator>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mut custom_render_phases: ResMut<ViewBinnedRenderPhases<Cloud3d>>,
    mut views: Query<(
        &ExtractedView,
        &RenderVisibleEntities,
        &Msaa,
        Has<OrderIndependentTransparencySettings>,
    )>,
    has_marker: Query<(), With<Cloud>>,
    mut change_tick: Local<Tick>,
) {
    for (view, visible_entities, msaa, has_oit) in &mut views {
        let Some(custom_phase) = custom_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        let draw_function = custom_draw_functions.read().id::<DrawCloud3d>();

        // Create the key based on the view.
        // In this case we only care about MSAA and HDR
        let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        if has_oit {
            view_key |= MeshPipelineKey::OIT_ENABLED;
        }

        // Since our phase can work on any 3d mesh we can reuse the default mesh 3d filter
        for (render_entity, visible_entity) in visible_entities.iter::<Mesh3d>() {
            // We only want meshes with the marker component to be queued to our phase.
            if has_marker.get(*render_entity).is_err() {
                continue;
            }
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*visible_entity)
            else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };

            // Specialize the key for the current mesh entity
            // For this example we only specialize based on the mesh topology
            // but you could have more complex keys and that's where you'd need to create those keys
            let mut mesh_key = view_key;
            mesh_key |= MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());

            let pipeline_id = pipelines.specialize(
                &pipeline_cache,
                &custom_draw_pipeline,
                mesh_key,
                &mesh.layout,
            );
            let pipeline_id = match pipeline_id {
                Ok(id) => id,
                Err(err) => {
                    error!("{}", err);
                    continue;
                }
            };
            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id);

            let batch_set_key = Cloud3dBatchSetKey {
                pipeline: pipeline_id,
                draw_function,
                vertex_slab: vertex_slab.unwrap_or_default(),
                index_slab,
            };
            let bin_key = Cloud3dBinKey {
                asset_id: mesh_instance.mesh_asset_id.into(),
            };

            let next_change_tick = change_tick.get() + 1;
            change_tick.set(next_change_tick);

            custom_phase.add(
                batch_set_key,
                bin_key,
                (*render_entity, *visible_entity),
                mesh_instance.current_uniform_index,
                BinnedRenderPhaseType::mesh(
                    mesh_instance.should_batch(),
                    &gpu_preprocessing_support,
                ),
                *change_tick,
            );
        }
    }
}

// Render label used to order our render graph node that will render our phase
#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
struct CustomDrawPassLabel;

#[derive(Default)]
struct CustomDrawNode;
impl ViewNode for CustomDrawNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewDepthTexture,
        &'static CloudTextures,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, depth, cloud_textures): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // First, we need to get our phases resource
        let Some(cloud_phases) = world.get_resource::<ViewBinnedRenderPhases<Cloud3d>>() else {
            return Ok(());
        };

        // Get the view entity from the graph
        let view_entity = graph.view_entity();

        // Get the phase for the current view running our node
        let Some(cloud_phase) = cloud_phases.get(&view.retained_view_entity) else {
            return Ok(());
        };

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("cloud prepass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &cloud_textures.coverage.default_view,
                    resolve_target: None,
                    ops: Operations::default(),
                }),
                Some(RenderPassColorAttachment {
                    view: &cloud_textures.depth.default_view,
                    resolve_target: None,
                    ops: Operations::default(),
                }),
            ],
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some(viewport) = camera.viewport.as_ref() {
            render_pass.set_camera_viewport(viewport);
        }

        // Render the phase
        // This will execute each draw functions of each phase items queued in this phase
        if let Err(err) = cloud_phase.render(&mut render_pass, world, view_entity) {
            error!("Error encountered while rendering the stencil phase {err:?}");
        }

        Ok(())
    }
}

struct PostProcessPlugin;
impl Plugin for PostProcessPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_graph_node::<ViewNodeRunner<PostProcessNode>>(
                // Specify the label of the graph, in this case we want the graph for 3d
                Core3d,
                // It also needs the label of the node
                PostProcessLabel,
            )
            .add_render_graph_edges(
                Core3d,
                // Specify the node ordering.
                // This will automatically create all required node edges to enforce the given ordering.
                (PostProcessLabel, OitResolvePass),
            );
    }

    fn finish(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Initialize the pipeline
            .init_resource::<PostProcessPipeline>();
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct PostProcessLabel;

// The post process node used for the render graph
#[derive(Default)]
struct PostProcessNode;

// The ViewNode trait is required by the ViewNodeRunner
impl ViewNode for PostProcessNode {
    // The node needs a query to gather data from the ECS in order to do its rendering,
    // but it's not a normal system so we need to define it manually.
    //
    // This query will only run on the view entity
    type ViewQuery = (
        &'static ViewTarget,
        &'static CloudTextures,
        &'static ViewUniformOffset,
        &'static ViewLightsUniformOffset,
        &'static ViewFogUniformOffset,
    );

    // Runs the node logic
    // This is where you encode draw commands.
    //
    // This will run on every view on which the graph is running.
    // If you don't want your effect to run on every camera,
    // you'll need to make sure you have a marker component as part of [`ViewQuery`]
    // to identify which camera(s) should run the effect.
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (
            view_target,
            cloud_textures,
            view_uniform_offset,
            lights_uniform_offset,
            fog_uniform_offset,
        ): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // Get the pipeline resource that contains the global data we need
        // to create the render pipeline
        let post_process_pipeline = world.resource::<PostProcessPipeline>();

        // The pipeline cache is a cache of all previously created pipelines.
        // It is required to avoid creating a new pipeline each frame,
        // which is expensive due to shader compilation.
        let pipeline_cache = world.resource::<PipelineCache>();

        // Get the pipeline from the cache
        let Some(pipeline) = pipeline_cache.get_render_pipeline(post_process_pipeline.pipeline_id)
        else {
            return Ok(());
        };

        // This will start a new "post process write", obtaining two texture
        // views from the view target - a `source` and a `destination`.
        // `source` is the "current" main texture and you _must_ write into
        // `destination` because calling `post_process_write()` on the
        // [`ViewTarget`] will internally flip the [`ViewTarget`]'s main
        // texture to the `destination` texture. Failing to do so will cause
        // the current main texture information to be lost.
        let post_process = view_target.post_process_write();

        let oit_buffers = world.resource::<OitBuffers>();
        let view_uniforms = world.resource::<ViewUniforms>();
        let light_meta = world.resource::<LightMeta>();
        let fog_meta = world.resource::<FogMeta>();
        // The bind_group gets created each frame.
        //
        // Normally, you would create a bind_group in the Queue set,
        // but this doesn't work with the post_process_write().
        // The reason it doesn't work is because each post_process_write will alternate the source/destination.
        // The only way to have the correct source/destination for the bind_group
        // is to make sure you get it during the node execution.
        let bind_group = render_context.render_device().create_bind_group(
            "post_process_bind_group",
            &post_process_pipeline.layout,
            // It's important for this to match the BindGroupLayout defined in the PostProcessPipeline
            &BindGroupEntries::sequential((
                view_uniforms.uniforms.binding().unwrap().clone(),
                light_meta.view_gpu_lights.binding().unwrap().clone(),
                fog_meta.gpu_fogs.binding().unwrap().clone(),
                &cloud_textures.coverage.default_view,
                &cloud_textures.depth.default_view,
                // Make sure to use the source view
                post_process.source,
                // Use the sampler created for the pipeline
                &post_process_pipeline.sampler,
                oit_buffers.layers.binding().unwrap().clone(),
                oit_buffers.layer_ids.binding().unwrap().clone(),
                oit_buffers.settings.binding().unwrap().clone(),
            )),
        );

        // Begin the render pass
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("post_process_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                // We need to specify the post process destination view here
                // to make sure we write to the appropriate texture.
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // This is mostly just wgpu boilerplate for drawing a fullscreen triangle,
        // using the pipeline/bind_group created above
        render_pass.set_render_pipeline(pipeline);

        // By passing in the index of the post process settings on this view, we ensure
        // that in the event that multiple settings were sent to the GPU (as would be the
        // case with multiple cameras), we use the correct one.
        render_pass.set_bind_group(
            0,
            &bind_group,
            &[
                view_uniform_offset.offset,
                lights_uniform_offset.offset,
                fog_uniform_offset.offset,
            ],
        );

        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

// This contains global data used by the render pipeline. This will be created once on startup.
#[derive(Resource)]
struct PostProcessPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for PostProcessPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        // We need to define the bind group layout used for our pipeline
        let layout = render_device.create_bind_group_layout(
            "post_process_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                // The layout entries will only be visible in the fragment stage
                ShaderStages::FRAGMENT,
                (
                    binding_types::uniform_buffer::<ViewUniform>(true),
                    binding_types::uniform_buffer::<GpuLights>(true),
                    binding_types::uniform_buffer::<GpuFog>(true),
                    // Cloud texture
                    binding_types::texture_2d(TextureSampleType::Float { filterable: true }),
                    // Cloud depth texture
                    binding_types::texture_2d(TextureSampleType::Float { filterable: true }),
                    // Screen texture
                    binding_types::texture_2d(TextureSampleType::Float { filterable: true }),
                    // Screen texture sampler
                    binding_types::sampler(SamplerBindingType::Filtering),
                    // OIT
                    binding_types::storage_buffer_sized(false, None),
                    binding_types::storage_buffer_sized(false, None),
                    binding_types::uniform_buffer::<OrderIndependentTransparencySettings>(false),
                ),
            ),
        );

        // We can create the sampler here since it won't change at runtime and doesn't depend on the view
        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            // This will add the pipeline to the cache and queue its creation
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("post_process_pipeline".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader: CLOUD_POST_PROCESS_SHADER.clone(),
                    shader_defs: vec!["OIT_ENABLED".into()],
                    // Make sure this matches the entry point of your shader.
                    // It can be anything as long as it matches here and in the shader.
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                // All of the following properties are not important for this effect so just use the default values.
                // This struct doesn't have the Default trait implemented because not all fields can have a default value.
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
                zero_initialize_workgroup_memory: false,
            });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

#[derive(Component)]
struct CloudTextures {
    coverage: CachedTexture,
    depth: CachedTexture,
}

fn prepare_cloud_texture(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedCamera, &ExtractedView)>,
) {
    for (entity, camera, view) in &views {
        let Some(physical_target_size) = camera.physical_target_size else {
            continue;
        };

        let coverage = TextureDescriptor {
            label: Some("cloud coverage"),
            size: Extent3d {
                depth_or_array_layers: 1,
                width: physical_target_size.x,
                height: physical_target_size.y,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let depth = TextureDescriptor {
            label: Some("cloud depth"),
            size: Extent3d {
                depth_or_array_layers: 1,
                width: physical_target_size.x,
                height: physical_target_size.y,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let textures = CloudTextures {
            coverage: texture_cache.get(&render_device, coverage),
            depth: texture_cache.get(&render_device, depth),
        };

        commands.entity(entity).insert(textures);
    }
}
