use crate::compiler::functions::builtin::{BuiltIn, library::fs::FS_FUNCTIONS};

mod fs;

pub struct Library {
    pub name: &'static str,
    pub functions: &'static [BuiltIn],
    #[allow(unused)]
    pub supported_on_wasm: bool,
}

pub(super) static LIBRARIES: [Library; 1] =
    [Library { name: "fs", functions: FS_FUNCTIONS, supported_on_wasm: false }];
