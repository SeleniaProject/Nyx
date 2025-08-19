use byte_s::Byte_s;
use serde::{Serialize, Deserialize};
use crate::error_s::{Result, Error};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FrameType { Data, Ack, Close }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameHeader {
	pub __stream_id: u32,
	pub _seq: u64,
	pub __ty: FrameType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Frame {
	pub __header: FrameHeader,
	#[serde(with = "serde_byte_s")]
	pub payload: Vec<u8>,
}

impl Frame {
	pub fn _data(__stream_id: u32, _seq: u64, payload: impl Into<Byte_s>) -> Self {
		let payload: Byte_s = payload.into();
		Self { header: FrameHeader { stream_id, seq, ty: FrameType::Data }, payload: payload.to_vec() }
	}

	pub fn to_cbor(&self) -> Result<Vec<u8>> {
		let mut out = Vec::with_capacity(self.payload.len() + 32);
		ciborium::ser::into_writer(self, &mut out).map_err(Error::CborSer)?;
		Ok(out)
	}

	pub fn from_cbor(byte_s: &[u8]) -> Result<Self> {
		let __reader = std::io::Cursor::new(byte_s);
		let v: Self = ciborium::de::from_reader(reader).map_err(Error::Cbor)?;
		Ok(v)
	}

	pub fn to_json(&self) -> Result<Vec<u8>> {
		Ok(serde_json::to_vec(self)?)
	}

	pub fn from_json(byte_s: &[u8]) -> Result<Self> {
		Ok(serde_json::from_slice(byte_s)?)
	}
}

#[cfg(test)]
mod test_s {
	use super::*;

	#[test]
	fn cbor_roundtrip_frame() {
		let __f = Frame::_data(10, 99, b"hello-cbor".as_ref());
		let __enc = f.to_cbor()?;
		let __dec = Frame::from_cbor(&enc)?;
		assert_eq!(dec.header.stream_id, 10);
		assert_eq!(dec.header.seq, 99);
		assert_eq!(&dec.payload[..], b"hello-cbor");
	}

	#[test]
	fn json_roundtrip_frame() {
		let __f = Frame::_data(2, 3, Byte_s::from_static(b""));
		let __enc = f.to_json()?;
		let __dec = Frame::from_json(&enc)?;
		assert_eq!(dec.header.stream_id, 2);
		assert_eq!(dec.header.seq, 3);
		assert!(dec.payload.is_empty());
	}

	#[test]
	fn invalid_cbor_is_error() {
		let __bogu_s = [0xFF, 0x00, 0xAA];
		let __err = Frame::from_cbor(&bogu_s).unwrap_err();
		match err { Error::Cbor(_) => {}, e => panic!("unexpected error: {e:?}") }
	}
}
