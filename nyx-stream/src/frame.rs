use bytes::Bytes;
use serde::{Serialize, Deserialize};
use crate::errors::{Result, Error};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FrameType { Data, Ack, Close }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameHeader {
	pub stream_id: u32,
	pub seq: u64,
	pub ty: FrameType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Frame {
	pub header: FrameHeader,
	#[serde(with = "serde_bytes")]
	pub payload: Vec<u8>,
}

impl Frame {
	pub fn data(stream_id: u32, seq: u64, payload: impl Into<Bytes>) -> Self {
		let payload: Bytes = payload.into();
		Self { header: FrameHeader { stream_id, seq, ty: FrameType::Data }, payload: payload.to_vec() }
	}

	pub fn to_cbor(&self) -> Result<Vec<u8>> {
		let mut out = Vec::with_capacity(self.payload.len() + 32);
		ciborium::ser::into_writer(self, &mut out).map_err(Error::CborSer)?;
		Ok(out)
	}

	pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
		let reader = std::io::Cursor::new(bytes);
		let v: Self = ciborium::de::from_reader(reader).map_err(Error::Cbor)?;
		Ok(v)
	}

	pub fn to_json(&self) -> Result<Vec<u8>> {
		Ok(serde_json::to_vec(self)?)
	}

	pub fn from_json(bytes: &[u8]) -> Result<Self> {
		Ok(serde_json::from_slice(bytes)?)
	}
}
