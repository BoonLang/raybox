// Include generated shader bindings
#[allow(unused, non_snake_case, non_camel_case_types, non_upper_case_globals)]
pub mod shader_bindings {
    include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
}

pub mod camera;
pub mod constants;
pub mod text;

#[cfg(feature = "windowed")]
pub mod input;

#[cfg(target_arch = "wasm32")]
mod web;
