mod transform;

pub trait Plugin: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    fn update(&mut self);

    fn handle_server_data(&mut self, _data: Vec<u8>) {}

    fn set_update_frequency(&mut self) -> Option<f32> {
        None
    }
}

#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        #[export_name = "init-plugin"]
        pub extern "C" fn __init_plugin() {
            fmc_client_api::register_plugin(|| {
                Box::new(<$plugin_type as fmc_client_api::Plugin>::new())
            });
        }
    };
}

static mut PLUGIN: Option<Box<dyn Plugin>> = None;

#[doc(hidden)]
pub fn register_plugin(build_plugin: fn() -> Box<dyn Plugin>) {
    unsafe { PLUGIN = Some((build_plugin)()) }
}

fn plugin() -> &'static mut dyn Plugin {
    unsafe { PLUGIN.as_deref_mut().unwrap() }
}

mod wit {
    use std::hash::{Hash, Hasher};

    wit_bindgen::generate!({
        skip: ["init-plugin"],
        path: "./src/api.wit",
        with: {
            "fmc:api/math": crate::math,
            "fmc:api/transform": crate::transform
        },
    });

    impl Hash for Key {
        fn hash<H: Hasher>(&self, state: &mut H) {
            let discriminant = std::mem::discriminant(self);
            core::hash::Hash::hash(&discriminant, state);
        }
    }
}

pub use wit::{
    delta_time, get_block, get_block_state, get_camera_transform, get_model_transform, get_models,
    get_player_transform, keyboard_input, log, set_camera_transform, set_player_transform, Key,
    KeyboardKey,
};

pub mod math {
    pub use glam::*;
}

pub mod prelude {
    pub use crate::math::{DQuat, IVec3, Quat, Vec3};
    pub use crate::transform::Transform;
}

wit::export!(Component with_types_in wit);

struct Component;

impl wit::Guest for Component {
    fn set_update_frequency() -> Option<f32> {
        plugin().set_update_frequency()
    }

    fn update() {
        plugin().update()
    }

    fn handle_server_data(data: Vec<u8>) {
        plugin().handle_server_data(data)
    }
}
