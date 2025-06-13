use std::collections::HashMap;

use bevy::{
    ecs::system::SystemState,
    input::{keyboard::KeyboardInput, ButtonState},
    math::{Mat3A, Vec3A},
    prelude::*,
    render::primitives::Aabb,
    window::{CursorGrabMode, PrimaryWindow},
};
use fmc_protocol::messages;
use wasmtime::{component::Linker, Engine, Store};

use crate::{
    assets::models::{Model, Models},
    game_state::GameState,
    networking::NetworkClient,
    player::Player,
    world::{blocks::Blocks, models::ModelEntities, world_map::WorldMap, Origin},
};

pub struct WasmPlugin;
impl Plugin for WasmPlugin {
    fn build(&self, app: &mut App) {
        let engine = Engine::default();
        let store = Store::new(&engine, WasmState::default());
        let mut linker = wasmtime::component::Linker::new(&engine);
        wit::Plugin::add_to_linker(&mut linker, |state: &mut WasmState| state).unwrap();

        let keyboard_events = SystemState::new(app.world_mut());
        app.insert_resource(WasmHost {
            engine,
            store,
            linker,
            enabled: HashMap::new(),
            disabled: HashMap::new(),
            keyboard_events,
        });

        app.add_systems(
            Update,
            (run_plugins, plugin_activation).run_if(in_state(GameState::Playing)),
        );
    }
}

const PLUGIN_PATH: &str = "server_assets/active/plugins/";

pub(super) fn load_plugins(host: ResMut<WasmHost>, net: Res<NetworkClient>) {
    let directory = match std::fs::read_dir(PLUGIN_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(&format!(
                "Misconfigured assets: Failed to read plugin directory at '{}'\n Error: {}",
                PLUGIN_PATH, e
            ));
            return;
        }
    };

    // Needed for split borrowing
    let host = host.into_inner();
    host.enabled.clear();
    host.disabled.clear();
    host.store = Store::new(&host.engine, WasmState::default());

    for dir_entry in directory {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured assets: Failed to read the file path of a plugin\n\
                    Error: {}",
                    e
                ));
                return;
            }
        };

        let plugin_name = path.file_stem().unwrap().to_string_lossy().into_owned();

        let component = wasmtime::component::Component::from_file(&host.engine, path).unwrap();
        let plugin = wit::Plugin::instantiate(&mut host.store, &component, &host.linker).unwrap();

        // TODO: Assuming no error
        plugin.call_init_plugin(&mut host.store).unwrap();

        let update_frequency = plugin
            .call_set_update_frequency(&mut host.store)
            // TODO: Assuming no error
            .unwrap()
            .map(|freq| Timer::from_seconds(freq, TimerMode::Repeating));

        host.disabled.insert(
            plugin_name,
            Instance {
                plugin,
                update_frequency,
            },
        );
    }
}

// TODO: Even though dependencies are -O3 optimized in debug running the player physics still
// jump from ~300 micros to ~30 when going from debug to release. What causes this?
fn run_plugins(world: &mut World) {
    let mut host = world.remove_resource::<WasmHost>().unwrap();
    let mut data_events = world
        .remove_resource::<Events<messages::PluginData>>()
        .unwrap();

    let state = host.store.data_mut();
    state._world = Some(world as *mut World);

    for plugin_data in data_events.update_drain() {
        let Some(plugin) = host.enabled.get(&plugin_data.plugin) else {
            error!(
                "Received data for plugin with name '{}', but there is no plugin by that name",
                &plugin_data.plugin
            );
            continue;
        };

        plugin
            .plugin
            .call_handle_server_data(&mut host.store, &plugin_data.data)
            .unwrap();
    }

    host.cache_keyboard_input();

    let time = world.get_resource::<Time>().unwrap();
    for plugin in host.enabled.values_mut() {
        if let Some(timer) = &mut plugin.update_frequency {
            // The state time is the delta time exposed to the plugin
            let state = host.store.data_mut();
            state.delta_time = timer.duration().as_secs_f32().min(time.delta_secs());

            timer.tick(time.delta());

            for _ in 0..timer.times_finished_this_tick().max(1) as usize {
                plugin.plugin.call_update(&mut host.store).unwrap();
            }
        } else {
            let state = host.store.data_mut();
            state.delta_time = time.delta_secs();
            plugin.plugin.call_update(&mut host.store).unwrap();
        }
    }

    world.insert_resource(data_events);
    world.insert_resource(host);
}

fn plugin_activation(
    mut wasm_host: ResMut<WasmHost>,
    mut plugins_events: EventReader<messages::Plugin>,
) {
    for event in plugins_events.read() {
        match event {
            messages::Plugin::Enable(name) => {
                if let Some((name, instance)) = wasm_host.disabled.remove_entry(name) {
                    wasm_host.enabled.insert(name, instance);
                }
            }
            messages::Plugin::Disable(name) => {
                if let Some((name, instance)) = wasm_host.enabled.remove_entry(name) {
                    wasm_host.disabled.insert(name, instance);
                }
            }
        }
    }
}

mod wit {
    wasmtime::component::bindgen!({
        path: "./api/src/api.wit",
    });
    // TODO: I'd like to just use bevy's types, but it complains about traits that are not derived,
    // maybe the foreign type constraint?
    use self::fmc::api::math::*;

    impl From<bevy::math::IVec3> for IVec3 {
        fn from(value: bevy::math::IVec3) -> Self {
            IVec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            }
        }
    }

    impl From<IVec3> for bevy::math::IVec3 {
        fn from(value: IVec3) -> Self {
            bevy::math::IVec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            }
        }
    }

    impl From<bevy::math::Vec3> for Vec3 {
        fn from(value: bevy::math::Vec3) -> Self {
            Vec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            }
        }
    }

    impl From<bevy::math::Vec3A> for Vec3 {
        fn from(value: bevy::math::Vec3A) -> Self {
            Vec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            }
        }
    }

    impl From<Vec3> for bevy::math::Vec3 {
        fn from(value: Vec3) -> Self {
            bevy::math::Vec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            }
        }
    }

    impl From<bevy::math::Quat> for DQuat {
        fn from(value: bevy::math::Quat) -> Self {
            DQuat {
                x: value.x as f64,
                y: value.y as f64,
                z: value.z as f64,
                w: value.w as f64,
            }
        }
    }

    impl From<DQuat> for bevy::math::Quat {
        fn from(value: DQuat) -> Self {
            bevy::math::Quat::from_xyzw(
                value.x as f32,
                value.y as f32,
                value.z as f32,
                value.w as f32,
            )
        }
    }
}

struct Instance {
    plugin: wit::Plugin,
    update_frequency: Option<Timer>,
}

#[derive(Resource)]
pub(super) struct WasmHost {
    engine: Engine,
    store: Store<WasmState>,
    linker: Linker<WasmState>,
    enabled: HashMap<String, Instance>,
    disabled: HashMap<String, Instance>,
    keyboard_events: SystemState<EventReader<'static, 'static, KeyboardInput>>,
}

impl WasmHost {
    fn cache_keyboard_input(&mut self) {
        let state = self.store.data_mut();
        state.keyboard_events.clear();
        for key in self.keyboard_events.get_mut(state.world()).read() {
            let wit_key = match key.key_code {
                KeyCode::KeyW => wit::Key::KeyW,
                KeyCode::KeyA => wit::Key::KeyA,
                KeyCode::KeyS => wit::Key::KeyS,
                KeyCode::KeyD => wit::Key::KeyD,
                KeyCode::Space => wit::Key::Space,
                KeyCode::ShiftLeft | KeyCode::ShiftRight => wit::Key::Shift,
                KeyCode::ControlLeft | KeyCode::ControlRight => wit::Key::Control,
                _ => continue,
            };
            state.keyboard_events.push(wit::KeyboardKey {
                key: wit_key,
                released: key.state == ButtonState::Released,
                repeat: key.repeat,
            });
        }
    }
}

#[derive(Default)]
struct WasmState {
    delta_time: f32,
    keyboard_events: Vec<wit::KeyboardKey>,
    _world: Option<*mut World>,
}

impl WasmState {
    // Lifetimes let you borrow world while you borrow keyboard events
    fn world<'a, 'b>(&'a mut self) -> &'b mut World {
        unsafe { self._world.unwrap().as_mut().unwrap() }
    }
}

unsafe impl Send for WasmState {}
unsafe impl Sync for WasmState {}
impl wit::fmc::api::math::Host for WasmState {}
impl wit::fmc::api::transform::Host for WasmState {}

impl wit::PluginImports for WasmState {
    fn log(&mut self, message: String) {
        info!(message);
    }

    fn delta_time(&mut self) -> f32 {
        self.delta_time
    }

    fn keyboard_input(&mut self) -> Vec<wit::KeyboardKey> {
        if self
            .world()
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(self.world())
            .unwrap()
            .cursor_options
            .grab_mode
            == CursorGrabMode::None
        {
            // TODO: Waiting for KeyboardFocus implementation
            //
            // The cursor being free is a weak signal keyboard input should not be captured.
            return Vec::new();
        };
        self.keyboard_events.clone()
    }

    fn get_player_transform(&mut self) -> wit::Transform {
        let def = Transform::default();
        let transform = self
            .world()
            .query_filtered::<&Transform, With<Player>>()
            .single(self.world());

        let transform = if let Ok(t) = transform {
            t
        } else {
            warn!("default");
            &def
        };
        wit::Transform {
            translation: transform.translation.into(),
            rotation: transform.rotation.into(),
            scale: transform.scale.into(),
        }
    }

    fn set_player_transform(&mut self, new_transform: wit::Transform) {
        let Ok(mut transform) = self
            .world()
            .query_filtered::<&mut Transform, With<Player>>()
            .single_mut(self.world())
        else {
            return;
        };

        transform.translation = new_transform.translation.into();
        transform.rotation = new_transform.rotation.into();
        transform.scale = new_transform.scale.into();
    }

    fn get_camera_transform(&mut self) -> wit::Transform {
        let def = GlobalTransform::default();
        let transform = self
            .world()
            .query_filtered::<&GlobalTransform, With<Camera>>()
            .single(self.world());

        let transform = if let Ok(t) = transform {
            t
        } else {
            warn!("default");
            &def
        };
        wit::Transform {
            translation: transform.translation().into(),
            rotation: transform.rotation().into(),
            scale: transform.scale().into(),
        }
    }

    fn set_camera_transform(&mut self, new_transform: wit::Transform) {
        let Ok(mut transform) = self
            .world()
            .query_filtered::<&mut Transform, With<Camera>>()
            .single_mut(self.world())
        else {
            return;
        };

        transform.translation = new_transform.translation.into();
        transform.rotation = new_transform.rotation.into();
        transform.scale = new_transform.scale.into();
    }

    fn get_block(&mut self, block_position: wit::IVec3) -> Option<wit::BlockId> {
        let world = self.world();
        let origin = world.get_resource::<Origin>().unwrap();
        let world_map = world.get_resource::<WorldMap>().unwrap();
        let block_position = IVec3 {
            x: block_position.x,
            y: block_position.y,
            z: block_position.z,
        } + origin.0;
        world_map.get_block(&block_position)
    }

    fn get_block_name(&mut self, block_id: wit::BlockId) -> String {
        // TODO: The wasm code can use an invalid block id
        Blocks::get().get_config(block_id).name().to_owned()
    }

    fn get_block_friction(&mut self, block_id: wit::BlockId) -> wit::Friction {
        // TODO: The wasm code can use an invalid block id
        let config = Blocks::get().get_config(block_id);
        let surface = if let Some(surface_friction) = config.friction() {
            Some(wit::SurfaceFriction {
                front: surface_friction.front,
                back: surface_friction.back,
                right: surface_friction.right,
                left: surface_friction.left,
                top: surface_friction.top,
                bottom: surface_friction.bottom,
            })
        } else {
            None
        };

        let drag = config.drag();
        let drag = wit::Vec3 {
            x: drag.x,
            y: drag.y,
            z: drag.z,
        };

        wit::Friction { surface, drag }
    }

    fn get_block_aabb(&mut self, block_id: wit::BlockId) -> Option<(wit::Vec3, wit::Vec3)> {
        let config = Blocks::get().get_config(block_id);
        let aabb = config.aabb()?;
        Some((aabb.center.into(), aabb.half_extents.into()))
    }

    fn get_models(&mut self, min: wit::Vec3, max: wit::Vec3) -> Vec<u32> {
        let world = self.world();
        let mut model_query = world.query::<(Entity, &GlobalTransform, &Model)>();
        let configs = world.get_resource::<Models>().unwrap();
        let model_entites = world.get_resource::<ModelEntities>().unwrap();

        let min = Vec3A::new(min.x, min.y, min.z);
        let max = Vec3A::new(max.x, max.y, max.z);

        let mut models = Vec::new();
        for (entity, transform, model) in model_query.iter(&world) {
            let aabb = match model {
                Model::Asset(asset_id) => {
                    let model_config = configs.get_config(asset_id).unwrap();
                    &model_config.aabb
                }
                Model::Custom { aabb } => aabb,
            };

            let transformed_aabb = transform_aabb(transform, aabb);

            if min.cmple(transformed_aabb.max()).all() || max.cmpge(transformed_aabb.min()).all() {
                let Some(model_id) = model_entites.get_model_id(&entity) else {
                    // TODO: This should be a safe unwrap but fails intermittently.
                    error!("Found unregistered model when running plugin");
                    continue;
                };
                models.push(model_id);
            }
        }

        models
    }

    fn get_model_aabb(&mut self, model_id: u32) -> (wit::Vec3, wit::Vec3) {
        let world = self.world();
        let mut model_query = world.query::<(&GlobalTransform, &Model)>();
        let model_entities = world.get_resource::<ModelEntities>().unwrap();
        let configs = world.get_resource::<Models>().unwrap();

        // TODO: Not safe to unwrap
        let entity = model_entities.get_entity(&model_id).unwrap();
        let (transform, model) = model_query.get(world, entity).unwrap();

        let aabb = match model {
            Model::Asset(asset_id) => {
                let config = configs.get_config(asset_id).unwrap();
                &config.aabb
            }
            Model::Custom { aabb } => aabb,
        };

        let transformed_aabb = transform_aabb(transform, aabb);

        return (
            transformed_aabb.center.into(),
            transformed_aabb.half_extents.into(),
        );
    }
}

fn transform_aabb(transform: &GlobalTransform, aabb: &Aabb) -> Aabb {
    // If you rotate a square normally, its aabb will grow larger at 45 degrees because the
    // diagonal of the square is longer and pointing in the axis direction. We don't want
    // our aabbs to grow larger, we want a constant volume because they are easier to deal with
    // in physics. Lets us use uniform aabbs without worrying about contortions.
    //
    // let abs_rot_mat = DMat3::from_cols(
    //     rot_mat.x_axis.abs(),
    //     rot_mat.y_axis.abs(),
    //     rot_mat.z_axis.abs(),
    // );
    //
    // This is how you do it normally, each column will have a euclidean distance of 1. At a 45
    // degree angle around the y axis, this will give an x_axis of
    // [sqrt(2)/2=0.707, 0.0, 0.707], i.e. take 70% of the x extent and 70% of the z
    // extent. We want it to only take 50%. This is done by normalizing it so its total
    // sum is 1.
    let rot_mat = Mat3A::from_quat(transform.rotation());
    let abs_rot_mat = Mat3A::from_cols(
        rot_mat.x_axis.abs() / rot_mat.x_axis.abs().element_sum(),
        rot_mat.y_axis.abs() / rot_mat.y_axis.abs().element_sum(),
        rot_mat.z_axis.abs() / rot_mat.z_axis.abs().element_sum(),
    );

    Aabb {
        center: rot_mat * aabb.center * Vec3A::from(transform.scale())
            + transform.translation_vec3a(),
        half_extents: abs_rot_mat * aabb.half_extents * Vec3A::from(transform.scale()),
    }
}
