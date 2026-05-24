#[cfg(not(target_arch = "wasm32"))]
use mimalloc::MiMalloc;

#[global_allocator]
#[cfg(not(target_arch = "wasm32"))]
static GLOBAL: MiMalloc = MiMalloc;

/// Live long and prosper
fn main() {
    keel::main();
}
