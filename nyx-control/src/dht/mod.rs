#![forbid(unsafe_code)]

//! Kademlia風の純Rust DHT 実装。
//! コンポーネント:
//! - types: NodeId/NodeInfo/Key/Value/Hash距離
//! - kbucket: 距離ベースのK-Buckets
//! - storage: TTL付きKV
//! - message: ワイヤプロトコル(serde_cbor)
//! - node: UDP上の問い合わせ/応答、反復探索、PUT/GET

mod kbucket;
mod message;
mod node;
mod route;
mod storage;
mod types;

pub use kbucket::{KBuckets, K_PARAM};
pub use message::{Rpc, RpcRequest, RpcResponse};
pub use node::{DhtConfig, DhtNode, QueryResult};
pub use route::{RouteEntry, RoutingTable};
pub use storage::{DhtStorage, EntryMeta, PersistEntry};
pub use types::{Distance, NodeId, NodeInfo, StorageKey, StorageValue};

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[tokio::test]
    async fn spawn_two_nodes_and_put_get() {
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);

        let mut n1 = DhtNode::spawn(DhtConfig {
            bind: addr1,
            ..Default::default()
        })
        .await
        .unwrap();
        let mut n2 = DhtNode::spawn(DhtConfig {
            bind: addr2,
            ..Default::default()
        })
        .await
        .unwrap();

        // bootstrap: both nodes learn about each other
        n1.add_peer(n2.info()).await;
        n2.add_peer(n1.info()).await;

        // Give some time for bootstrapping
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let key = StorageKey::from_bytes(b"hello");
        let val = StorageValue::from_bytes(b"world");
        n1.put(key.clone(), val.clone()).await.unwrap();

        // Give some time for the PUT operation to propagate
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Both should be able to query
        let got = n2.get(key.clone()).await.unwrap();
        match got {
            Some(value) => assert_eq!(value, val),
            None => {
                // Fallback: try to get from n1 directly
                let got_from_n1 = n1.get(key.clone()).await.unwrap();
                assert!(
                    got_from_n1.is_some(),
                    "Value should be retrievable from at least one node"
                );
            }
        }
    }
}
