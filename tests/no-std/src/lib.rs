#![no_std]

pub use core_json;
pub use core_json_traits;

#[allow(dead_code)]
#[derive(Default, core_json_derive::JsonDeserialize)]
struct Json {}

pub use core_json_embedded_io;
