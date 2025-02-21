use fmc_client_api as fmc;

struct MyPlugin;

// Required for initialization
fmc::register_plugin!(MyPlugin);

impl fmc::Plugin for MyPlugin {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }

    // Your update logic
    fn update(&mut self) {
        todo!();
    }

    // Plugins are run once per frame. You can use this to set a fixed update timer instead. Note
    // that this is only a higher bound. If the set rate is slower than the frame time, it will
    // still run once per frame.
    fn set_update_frequency(&mut self) -> Option<f32> {
        // This plugin will run AT LEAST 144 times per second
        Some(1.0 / 144.0)
    }
}
