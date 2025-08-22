#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn init() { /* no-op for now */
}

#[cfg(test)]
mod test_s {
    #[test]
    fn smoke() {
        super::init();
    }
}
