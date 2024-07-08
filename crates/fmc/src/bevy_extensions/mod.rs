// f64 is needed for precision in large worlds. This creates a lot of unfortunate mess, bevy might
// add it in the future, see: https://github.com/bevyengine/bevy/issues/1680
//
// It is too early, but seeing as this makes the code so ugly it could be an idea to fork bevy and
// replace, just so Vec3 Quat etc names can be reclaimed.
pub mod f64_transform;
