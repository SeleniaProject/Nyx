#![forbid(unsafe_code)]

//! Kademlia風の純Rust DHT 実装。
//! コンポーネント:
//! - types: NodeId/NodeInfo/Key/Value/Hash距離
//! - kbucket: 距離ベースのK-Buckets
//! - storage: TTL付きKV
//! - message: ワイヤプロトコル(serde_cbor)
//! - node: UDP上の問い合わせ/応答、反復探索、PUT/GET

mod types;
mod kbucket;
mod storage;
mod message;
mod node;
mod route;

pub use types::{NodeId, NodeInfo, StorageKey, StorageValue, Distance};
pub use kbucket::{KBuckets, K_PARAM};
pub use storage::{DhtStorage, EntryMeta, PersistEntry};
pub use message::{Rpc, RpcRequest, RpcResponse};
pub use node::{DhtNode, DhtConfig, QueryResult};
pub use route::{RouteEntry, RoutingTable};

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[tokio::test]
    async fn spawn_two_nodes_and_put_get() {
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);

    let mut n1 = DhtNode::spawn(DhtConfig{ bind: addr1, ..Default::default() }).await.unwrap();
    let n2 = DhtNode::spawn(DhtConfig{ bind: addr2, ..Default::default() }).await.unwrap();

    // bootstrap: n1 learns n2
    n1.add_peer(n2.info()).await;

        let key = StorageKey::from_bytes(b"hello");
        let val = StorageValue::from_bytes(b"world");
        n1.put(key.clone(), val.clone()).await.unwrap();

        // Both should be able to query
        let got = n2.get(key.clone()).await.unwrap().unwrap();
        assert_eq!(got, val);
    }
}
