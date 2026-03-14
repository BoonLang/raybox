// Include generated shader bindings
#[allow(unused, non_snake_case, non_camel_case_types, non_upper_case_globals)]
pub mod shader_bindings {
    include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
}

pub mod ui2d_shader_bindings {
    pub use crate::shader_bindings::sdf_todomvc::*;
}

pub mod ui_physical_shader_bindings {
    pub use crate::shader_bindings::sdf_todomvc_3d::*;
}

pub mod camera;
pub mod constants;
pub mod demo_core;
pub mod gpu_runtime_common;
pub mod retained;
pub mod text;
#[path = "demos/todomvc_retained.rs"]
pub mod todomvc_retained;
pub mod todomvc_shared;
#[path = "demos/ui_physical_theme.rs"]
pub mod ui_physical_theme;

#[cfg(feature = "windowed")]
pub mod input;

#[cfg(feature = "overlay")]
pub mod simple_overlay;

#[cfg(feature = "windowed")]
pub mod demos;

#[cfg(feature = "control")]
pub mod control;

#[cfg(feature = "control")]
pub mod browser_launch;

#[cfg(feature = "hot-reload")]
pub mod hot_reload;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
mod web_input;

#[cfg(target_arch = "wasm32")]
mod web_control;

#[cfg(test)]
mod architecture_guard;
