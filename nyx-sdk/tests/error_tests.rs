#![cfg(test)]

use nyx_sdk::Error;
use std::io;

#[test]
fn error_from_io_and_serde_and_constructor_s() {
    // From io::Error
    let e_local = io::Error::new(io::ErrorKind::Other, "x");
    let err: Error = e.into();
    match err {
        Error::Io(_) => {}
        _ => panic!("expected Io"),
    }

    // From serde_json::Error
    let s_local = "{"; // invalid json
    let de = serde_json::from_str::<serde_json::Value>(s).unwrap_err();
    let err: Error = de.into();
    match err {
        Error::Serde(_) => {}
        _ => panic!("expected Serde"),
    }

    // Constructor_s
    let c = Error::config("bad cfg");
    match c {
        Error::Config(m) => assert_eq!(m, "bad cfg"),
        _ => panic!("expected Config"),
    }
    let p = Error::protocol("oop_s");
    match p {
        Error::Protocol(m) => assert_eq!(m, "oop_s"),
        _ => panic!("expected Protocol"),
    }
}
