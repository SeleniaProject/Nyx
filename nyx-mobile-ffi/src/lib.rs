#![forbid(unsafe_code)]

pub extern "C" fn nyx_mobile_init() -> i32 { 0 }

#[cfg(test)]
mod tests { #[test] fn init_returns_zero() { assert_eq!(super::nyx_mobile_init(), 0); } }

