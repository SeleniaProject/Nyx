#![forbid(unsafe_code)]

use crate::dht::types::{NodeId, NodeInfo, StorageKey, StorageValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", content = "p")]
pub enum RpcRequest {
    Ping(NodeId),
    FindNode { target: NodeId },
    Get { key: StorageKey },
    Put { key: StorageKey, value: StorageValue, ttl_secs: u64 },
    /// 要求先ノードの公開鍵を問い合わせる
    GetPubKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", content = "p")]
pub enum RpcResponse {
    Pong(NodeId),
    Nodes(Vec<NodeInfoSerializable>),
    Value(Option<StorageValue>),
    Ok,
    Err(String),
    /// 要求先ノードの公開鍵（ed25519）を返す（32バイト）
    PubKey([u8; 32]),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfoSerializable {
    pub id: NodeId,
    pub addr: String,
}

impl From<&NodeInfo> for NodeInfoSerializable {
    fn from(n: &NodeInfo) -> Self { Self { id: n.id.clone(), addr: n.addr.to_string() } }
}

impl TryFrom<NodeInfoSerializable> for NodeInfo {
    type Error = std::net::AddrParseError;
    fn try_from(v: NodeInfoSerializable) -> Result<Self, Self::Error> {
        Ok(NodeInfo { id: v.id, addr: v.addr.parse()?, last_seen: std::time::Instant::now() })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "k", content = "v")]
pub enum Rpc {
    Req { from: NodeId, sig: Option<Vec<u8>>, req: RpcRequest },
    Res { from: NodeId, sig: Option<Vec<u8>>, res: RpcResponse },
}
