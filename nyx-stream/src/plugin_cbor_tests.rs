#![cfg(test)]
#![forbid(unsafe_code)]

use crate::plugin::{PluginHeader, PluginId};
use crate::plugin_cbor::{parse_plugin_header, PluginCborError};

#[test]
fn parse_valid_header_round_trip() {
	let header = PluginHeader { id: PluginId(1), flags: 0xA5, data: vec![1,2,3] };
	let mut bytes = Vec::new();
	ciborium::ser::into_writer(&header, &mut bytes).unwrap();
	let parsed = parse_plugin_header(&bytes).unwrap();
	assert_eq!(parsed.id, header.id);
	assert_eq!(parsed.flags, header.flags);
	assert_eq!(parsed.data, header.data);
}

#[test]
fn parse_invalid_bytes_returns_error() {
	let err = parse_plugin_header(&[0xFF, 0x00, 0x10]).unwrap_err();
	match err { PluginCborError::Decode(_) => {}, _ => panic!("unexpected error type") }
}
