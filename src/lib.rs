// Include generated shader bindings
#[allow(unused, non_snake_case, non_camel_case_types, non_upper_case_globals)]
pub mod shader_bindings {
    include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
}

pub mod camera;
pub mod constants;
pub mod demo_core;
pub mod text;

#[cfg(feature = "windowed")]
pub mod input;

#[cfg(feature = "windowed")]
pub mod overlay;

#[cfg(feature = "overlay")]
pub mod simple_overlay;

#[cfg(feature = "windowed")]
pub mod demos;

#[cfg(feature = "control")]
pub mod control;

#[cfg(feature = "hot-reload")]
pub mod hot_reload;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
mod web_input;

#[cfg(target_arch = "wasm32")]
mod web_control;
