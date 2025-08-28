#![forbid(unsafe_code)]

use crate::dht::kbucket::{KBuckets, K_PARAM};
use crate::dht::message::{NodeInfoSerializable, Rpc, RpcRequest, RpcResponse};
use crate::dht::storage::DhtStorage;
use crate::dht::types::{NodeId, NodeInfo, StorageKey, StorageValue};
use anyhow::Result;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtConfig {
    pub bind: SocketAddr,
    pub request_timeout_ms: u64,
    #[serde(default = "default_alpha")]
    pub alpha: usize,
    #[serde(default)]
    pub persist_path: Option<std::path::PathBuf>,
    #[serde(default = "default_refresh_ms")]
    pub refresh_interval_ms: u64,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:0".parse().unwrap(),
            request_timeout_ms: 1_000,
            alpha: 3,
            persist_path: None,
            refresh_interval_ms: 30_000,
        }
    }
}

const fn default_alpha() -> usize {
    3
}
const fn default_refresh_ms() -> u64 {
    30_000
}

pub struct DhtNode {
    pub local: NodeInfo,
    kb: Arc<Mutex<KBuckets>>,
    storage: Arc<Mutex<DhtStorage>>,
    sock: Arc<UdpSocket>,
    tx: mpsc::UnboundedSender<Vec<u8>>, // internal sender to IO task
    signer: Arc<SigningKey>,
    peer_keys: Arc<Mutex<HashMap<NodeId, VerifyingKey>>>,
    config: DhtConfig,
}

#[derive(Debug)]
pub struct QueryResult<T> {
    pub value: T,
    pub nearest: Vec<NodeInfo>,
}

impl DhtNode {
    pub async fn spawn(cfg: DhtConfig) -> Result<Self> {
        // Generate keypair and derive NodeId from public key
        let signer = SigningKey::generate(&mut OsRng);
        let id = NodeId::from_pubkey(signer.verifying_key().as_bytes());
        let sock = Arc::new(UdpSocket::bind(cfg.bind).await?);
        let local_addr = sock.local_addr()?;
        let local = NodeInfo::new(id, local_addr);
        let kb = Arc::new(Mutex::new(KBuckets::new(local.id.clone())));
        let storage = Arc::new(Mutex::new(DhtStorage::new()));
        let mut node = Self {
            kb: kb.clone(),
            storage: storage.clone(),
            sock: sock.clone(),
            local,
            tx: mpsc::unbounded_channel().0,
            signer: Arc::new(signer),
            peer_keys: Arc::new(Mutex::new(HashMap::new())),
            config: cfg.clone(),
        };
        // load persisted snapshot if configured
        if let Some(path) = &node.config.persist_path {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(snapshot) =
                    ciborium::de::from_reader::<Vec<crate::dht::PersistEntry>, _>(&bytes[..])
                {
                    node.storage.lock().await.import_persist(snapshot);
                }
            }
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let sock_clone = node.sock.clone();
        tokio::spawn(async move {
            while let Some(buf) = rx.recv().await {
                // for now broadcast not used; reserved for future
                let _ = sock_clone;
                let _ = buf; // no-op
            }
        });
        node.tx = tx;
        // Spawn receive loop
        let recv_sock = node.sock.clone();
        let kb_arc = kb.clone();
        let storage_arc = storage.clone();
        let peer_keys = node.peer_keys.clone();
        let signer = node.signer.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            while let Ok((n, from)) = recv_sock.recv_from(&mut buf).await {
                if let Ok(rpc) = ciborium::de::from_reader::<Rpc, _>(&buf[..n]) {
                    match rpc {
                        Rpc::Req {
                            from: sender,
                            sig,
                            req,
                        } => {
                            // learn peer address
                            kb_arc
                                .lock()
                                .await
                                .upsert(NodeInfo::new(sender.clone(), from));
                            // if signature exists and key unknown, fetch pubkey first
                            if sig.is_some() {
                                let known = { peer_keys.lock().await.contains_key(&sender) };
                                if !known {
                                    let me = NodeId::from_pubkey(signer.verifying_key().as_bytes());
                                    let req_body = RpcRequest::GetPubKey;
                                    let s = sign_response(&signer, &RpcResponse::Ok); // reuse signer to sign some content (not used by peer)
                                    let req_rpc = Rpc::Req {
                                        from: me,
                                        sig: Some(s),
                                        req: req_body,
                                    };
                                    if let Ok(Rpc::Res {
                                        res: RpcResponse::PubKey(pk_bytes),
                                        ..
                                    }) = request_response(
                                        &recv_sock,
                                        req_rpc,
                                        from,
                                        Duration::from_millis(300),
                                    )
                                    .await
                                    {
                                        if let Ok(vk) = VerifyingKey::from_bytes(&pk_bytes) {
                                            peer_keys.lock().await.insert(sender.clone(), vk);
                                        }
                                    }
                                }
                                let ok = register_or_verify(&peer_keys, &sender, &sig, &req).await;
                                if !ok {
                                    continue;
                                }
                            }
                            let res =
                                handle_request(&kb_arc, &storage_arc, &signer, req, from).await;
                            if let Some((msg, to)) = res {
                                let _ = send_rpc(&recv_sock, &msg, to).await;
                            }
                        }
                        Rpc::Res { .. } => { /* Ignore unsolicited responses at server loop */ }
                    }
                }
            }
        });

        // periodic snapshot persistence
        {
            let storage_for_persist = storage.clone();
            let persist_path = node.config.persist_path.clone();
            let interval_ms = node.config.refresh_interval_ms;
            tokio::spawn(async move {
                if let Some(path) = persist_path {
                    loop {
                        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
                        let mut st = storage_for_persist.lock().await;
                        let entries = st.export_persist();
                        let mut buf = Vec::new();
                        if ciborium::ser::into_writer(&entries, &mut buf).is_ok() {
                            let _ = std::fs::write(&path, buf);
                        }
                    }
                }
            });
        }
        Ok(node)
    }

    pub fn info(&self) -> NodeInfo {
        self.local.clone()
    }
    pub async fn add_peer(&mut self, n: NodeInfo) {
        self.kb.lock().await.upsert(n.clone());
        let _ = self.fetch_and_store_peer_key(n.addr, n.id.clone()).await;
    }

    pub async fn put(&mut self, key: StorageKey, value: StorageValue) -> Result<()> {
        // Store locally and try to send to K nearest known peers
        {
            let mut st = self.storage.lock().await;
            let _ = st.put(key.clone(), value.clone());
        }
        let peers = self
            .kb
            .lock()
            .await
            .nearest(&self.local.id, K_PARAM.min(self.config.alpha));
        for p in peers {
            let req_body = RpcRequest::Put {
                key: key.clone(),
                value: value.clone(),
                ttl_secs: 3600,
            };
            let sig = self.sign_request(&req_body);
            let req = Rpc::Req {
                from: self.local.id.clone(),
                sig: Some(sig),
                req: req_body,
            };
            let _ = request_response(&self.sock, req, p.addr, Duration::from_millis(500)).await;
        }
        Ok(())
    }

    pub async fn get(&self, key: StorageKey) -> Result<Option<StorageValue>> {
        {
            let mut st = self.storage.lock().await;
            if let Some(v) = st.get(&key) {
                return Ok(Some(v));
            }
        }
        // iterative lookup: ask nearest peers
        // α並列で3ピアに問い合わせ
        let peers = self
            .kb
            .lock()
            .await
            .nearest(&self.local.id, K_PARAM.min(self.config.alpha));
        let mut futs = Vec::new();
        for p in peers {
            let req_body = RpcRequest::Get { key: key.clone() };
            let sig = self.sign_request(&req_body);
            let req = Rpc::Req {
                from: self.local.id.clone(),
                sig: Some(sig),
                req: req_body,
            };
            futs.push(request_response(
                &self.sock,
                req,
                p.addr,
                Duration::from_millis(500),
            ));
        }
        for f in futs {
            if let Ok(Rpc::Res {
                res: RpcResponse::Value(v),
                ..
            }) = f.await
            {
                return Ok(v);
            }
        }
        Ok(None)
    }
}

async fn send_rpc(sock: &UdpSocket, rpc: &Rpc, to: SocketAddr) -> Result<()> {
    let mut buf = Vec::with_capacity(256);
    ciborium::ser::into_writer(rpc, &mut buf)?;
    sock.send_to(&buf, to).await?;
    Ok(())
}

async fn request_response(
    sock: &UdpSocket,
    rpc: Rpc,
    to: SocketAddr,
    timeout_dur: Duration,
) -> Result<Rpc> {
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&rpc, &mut buf)?;
    sock.send_to(&buf, to).await?;
    let mut rbuf = vec![0u8; 2048];
    let fut = async {
        let (n, _from) = sock.recv_from(&mut rbuf).await?;
        let msg = ciborium::de::from_reader::<Rpc, _>(&rbuf[..n])?;
        Ok::<Rpc, anyhow::Error>(msg)
    };
    let msg = timeout(timeout_dur, fut).await??;
    Ok(msg)
}

async fn handle_request(
    kb: &Arc<Mutex<KBuckets>>,
    storage: &Arc<Mutex<DhtStorage>>,
    signer: &Arc<SigningKey>,
    req: RpcRequest,
    from: SocketAddr,
) -> Option<(Rpc, SocketAddr)> {
    match req {
        RpcRequest::Ping(id) => {
            // Update KB with sender
            kb.lock().await.upsert(NodeInfo::new(id, from));
            let me = NodeId::from_pubkey(signer.verifying_key().as_bytes());
            let res_body = RpcResponse::Pong(me.clone());
            let sig = sign_response(signer, &res_body);
            Some((
                Rpc::Res {
                    from: me,
                    sig: Some(sig),
                    res: res_body,
                },
                from,
            ))
        }
        RpcRequest::FindNode { target } => {
            let nearest = kb.lock().await.nearest(&target, K_PARAM);
            let list: Vec<NodeInfoSerializable> = nearest.iter().map(|n| n.into()).collect();
            let me = NodeId::from_pubkey(signer.verifying_key().as_bytes());
            let res_body = RpcResponse::Nodes(list);
            let sig = sign_response(signer, &res_body);
            Some((
                Rpc::Res {
                    from: me,
                    sig: Some(sig),
                    res: res_body,
                },
                from,
            ))
        }
        RpcRequest::Get { key } => {
            let mut st = storage.lock().await;
            let v = st.get(&key);
            let me = NodeId::from_pubkey(signer.verifying_key().as_bytes());
            let res_body = RpcResponse::Value(v);
            let sig = sign_response(signer, &res_body);
            Some((
                Rpc::Res {
                    from: me,
                    sig: Some(sig),
                    res: res_body,
                },
                from,
            ))
        }
        RpcRequest::Put {
            key,
            value,
            ttl_secs,
        } => {
            let mut st = storage.lock().await;
            let _ = st.put_with_ttl(key, value, Duration::from_secs(ttl_secs));
            let me = NodeId::from_pubkey(signer.verifying_key().as_bytes());
            let res_body = RpcResponse::Ok;
            let sig = sign_response(signer, &res_body);
            Some((
                Rpc::Res {
                    from: me,
                    sig: Some(sig),
                    res: res_body,
                },
                from,
            ))
        }
        RpcRequest::GetPubKey => {
            let me = NodeId::from_pubkey(signer.verifying_key().as_bytes());
            let res_body = RpcResponse::PubKey(*signer.verifying_key().as_bytes());
            let sig = sign_response(signer, &res_body);
            Some((
                Rpc::Res {
                    from: me,
                    sig: Some(sig),
                    res: res_body,
                },
                from,
            ))
        }
    }
}

fn sign_response(signer: &SigningKey, res: &RpcResponse) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = ciborium::ser::into_writer(res, &mut buf);
    let sig: Signature = signer.sign(&buf);
    sig.to_bytes().to_vec()
}

impl DhtNode {
    fn sign_request(&self, req: &RpcRequest) -> Vec<u8> {
        let mut buf = Vec::new();
        let _ = ciborium::ser::into_writer(req, &mut buf);
        let sig: Signature = self.signer.sign(&buf);
        sig.to_bytes().to_vec()
    }
}

async fn register_or_verify(
    store: &Arc<Mutex<HashMap<NodeId, VerifyingKey>>>,
    from: &NodeId,
    sig: &Option<Vec<u8>>,
    req: &RpcRequest,
) -> bool {
    if sig.is_none() {
        return true;
    }
    // try to verify using known key; if unknown, accept but do not mark verified
    let maybe_key = { store.lock().await.get(from).cloned() };
    if let Some(pk) = maybe_key {
        let mut buf = Vec::new();
        let _ = ciborium::ser::into_writer(req, &mut buf);
        if let Some(sig_bytes) = sig.as_ref() {
            if sig_bytes.len() == 64 {
                let mut arr = [0u8; 64];
                arr.copy_from_slice(&sig_bytes[..]);
                let signature = Signature::from_bytes(&arr);
                return pk.verify(&buf, &signature).is_ok();
            }
        }
    }
    true
}

impl DhtNode {
    /// ピアの公開鍵取得を試み、記録する
    pub async fn fetch_and_store_peer_key(
        &self,
        addr: SocketAddr,
        peer_id: NodeId,
    ) -> Result<Option<VerifyingKey>> {
        let req_body = RpcRequest::GetPubKey;
        let sig = self.sign_request(&req_body);
        let req = Rpc::Req {
            from: self.local.id.clone(),
            sig: Some(sig),
            req: req_body,
        };
        if let Ok(Rpc::Res {
            res: RpcResponse::PubKey(pk_bytes),
            ..
        }) = request_response(&self.sock, req, addr, Duration::from_millis(500)).await
        {
            if let Ok(vk) = VerifyingKey::from_bytes(&pk_bytes) {
                self.peer_keys.lock().await.insert(peer_id.clone(), vk);
                return Ok(Some(vk));
            }
        }
        Ok(None)
    }

    /// スナップショットを保存
    pub async fn persist_snapshot(&self) -> Result<()> {
        if let Some(path) = &self.config.persist_path {
            let mut st = self.storage.lock().await;
            let entries = st.export_persist();
            let mut buf = Vec::new();
            ciborium::ser::into_writer(&entries, &mut buf)?;
            std::fs::write(path, buf)?;
        }
        Ok(())
    }
}

// no extra helpers
