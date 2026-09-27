use crate::events::{ConnectionType, MessengerCommand, MessengerEvent};
use crate::folder;
use crate::identity::Identity;
use crate::protocol::{
    decode_ticket, encode_ticket, recv_wire_message, send_wire_message, WireMessage,
    DOOT_CHAT_ALPN, LEGACY_ALPN_NAME,
};
use crate::store::{
    Contact, FileAttachment, MessageDirection, MessageStatus, Store, StoredMessage,
};
use anyhow::{Context, Result};
use chrono::Utc;
use iroh::endpoint::{Connection, PathEvent};
use iroh::protocol::{AcceptError, ProtocolHandler, Router};
use iroh::{endpoint::presets, Endpoint, EndpointAddr, PublicKey};
use iroh_blobs::store::mem::MemStore;
use iroh_blobs::BlobsProtocol;
use iroh_gossip::net::{Gossip, GOSSIP_ALPN};
use iroh_ping::Ping;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct PendingFileTransfer {
    pub file_id: Uuid,
    pub peer_key: String,
    pub sender_name: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: String,
    pub blake3_hash: String,
    pub is_directory: bool,
}

#[derive(Clone, Debug)]
pub struct DootChatProtocol {
    active_connections: Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: Arc<Mutex<HashSet<String>>>,
    store: Arc<Mutex<Store>>,
    evt_tx: mpsc::UnboundedSender<MessengerEvent>,
    pending_transfers: Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>>,
    downloads_dir: PathBuf,
}

impl ProtocolHandler for DootChatProtocol {
    async fn accept(&self, conn: Connection) -> std::result::Result<(), AcceptError> {
        let remote_pk = conn.remote_id();
        let peer_key_str = remote_pk.to_string();
        tracing::info!(
            "Accepted incoming Doot chat connection from peer {}",
            peer_key_str
        );

        self.active_connections
            .write()
            .await
            .insert(remote_pk, conn.clone());
        self.connected_peers.lock().insert(peer_key_str.clone());

        let _ = self.store.lock().update_contact_last_seen(&peer_key_str);

        let initial_conn_type = get_connection_type(&conn);
        let direct_addr = match &initial_conn_type {
            ConnectionType::Direct { addr, .. } => Some(addr.clone()),
            _ => None,
        };

        let _ = self.evt_tx.send(MessengerEvent::PeerConnected {
            peer_key: peer_key_str.clone(),
            direct_addr,
            connection_type: initial_conn_type,
        });

        // Spawn path watcher for live connection_type and RTT updates
        spawn_path_watcher(conn.clone(), peer_key_str.clone(), self.evt_tx.clone());

        // Spawn stream reader loop for this connection
        spawn_stream_listener(
            conn.clone(),
            remote_pk,
            self.evt_tx.clone(),
            self.store.clone(),
            self.active_connections.clone(),
            self.connected_peers.clone(),
            self.pending_transfers.clone(),
            self.downloads_dir.clone(),
        );

        conn.closed().await;
        Ok(())
    }
}

pub fn get_connection_type(conn: &Connection) -> ConnectionType {
    for path in conn.paths().iter() {
        if path.is_selected() {
            let rtt_ms = path.rtt().as_millis() as u32;
            if path.is_ip() {
                return ConnectionType::Direct {
                    addr: path.remote_addr().to_string(),
                    rtt_ms,
                };
            } else if path.is_relay() {
                return ConnectionType::Relay {
                    url: path.remote_addr().to_string(),
                    rtt_ms,
                };
            }
        }
    }
    for path in conn.paths().iter() {
        let rtt_ms = path.rtt().as_millis() as u32;
        if path.is_ip() {
            return ConnectionType::Direct {
                addr: path.remote_addr().to_string(),
                rtt_ms,
            };
        } else if path.is_relay() {
            return ConnectionType::Relay {
                url: path.remote_addr().to_string(),
                rtt_ms,
            };
        }
    }
    ConnectionType::Unknown
}

fn spawn_path_watcher(
    conn: Connection,
    peer_key: String,
    event_tx: mpsc::UnboundedSender<MessengerEvent>,
) {
    tokio::spawn(async move {
        use futures_util::StreamExt;
        let mut path_events = conn.path_events();
        while let Some(event) = path_events.next().await {
            match event {
                PathEvent::Selected {
                    id, remote_addr, ..
                } => {
                    let rtt_ms = conn.rtt(id).map(|d| d.as_millis() as u32).unwrap_or(0);
                    let ct = if remote_addr.is_ip() {
                        ConnectionType::Direct {
                            addr: remote_addr.to_string(),
                            rtt_ms,
                        }
                    } else if remote_addr.is_relay() {
                        ConnectionType::Relay {
                            url: remote_addr.to_string(),
                            rtt_ms,
                        }
                    } else {
                        ConnectionType::Unknown
                    };
                    if ct != ConnectionType::Unknown {
                        let _ = event_tx.send(MessengerEvent::ConnectionInfoUpdated {
                            peer_key: peer_key.clone(),
                            connection_type: ct,
                        });
                    }
                }
                PathEvent::Opened { .. } | PathEvent::Closed { .. } | PathEvent::Lagged { .. } => {
                    let ct = get_connection_type(&conn);
                    if ct != ConnectionType::Unknown {
                        let _ = event_tx.send(MessengerEvent::ConnectionInfoUpdated {
                            peer_key: peer_key.clone(),
                            connection_type: ct,
                        });
                    }
                }
                _ => {}
            }
        }
    });
}

#[derive(Clone)]
pub struct MessengerHandle {
    pub my_node_id: String,
    pub my_ticket: String,
    pub my_nickname: String,
    pub data_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub router: Router,
    pub blobs_store: MemStore,
    pub gossip: Gossip,
    pub ping: Ping,
    command_tx: mpsc::UnboundedSender<MessengerCommand>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<MessengerEvent>>>,
    store: Arc<Mutex<Store>>,
    connected_peers: Arc<Mutex<HashSet<String>>>,
    pending_transfers: Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>>,
}

impl MessengerHandle {
    pub fn endpoint(&self) -> &Endpoint {
        self.router.endpoint()
    }

    pub fn blobs_store(&self) -> &MemStore {
        &self.blobs_store
    }

    pub fn gossip(&self) -> &Gossip {
        &self.gossip
    }

    pub fn ping(&self) -> &Ping {
        &self.ping
    }

    pub async fn ping_peer(&self, peer_addr: EndpointAddr) -> Result<Duration> {
        self.ping.ping(self.endpoint(), peer_addr).await
    }

    pub async fn start(
        custom_dir: Option<PathBuf>,
        default_nickname: Option<String>,
    ) -> Result<Self> {
        let identity = Identity::load_or_create(custom_dir, default_nickname)?;
        let store = Arc::new(Mutex::new(Store::load(identity.data_dir.clone())?));
        let connected_peers = Arc::new(Mutex::new(HashSet::new()));
        let pending_transfers: Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (evt_tx, evt_rx) = mpsc::unbounded_channel();

        let my_node_id = identity.public_key.to_string();
        let my_nickname = identity.nickname.clone();
        let data_dir = identity.data_dir.clone();
        let downloads_dir = data_dir.join("downloads");
        let _ = fs::create_dir_all(&downloads_dir);

        // Bind Iroh Endpoint
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(identity.secret_key.clone())
            .bind()
            .await
            .context("Failed to bind Iroh Endpoint")?;

        let my_addr = endpoint.addr();
        let my_ticket = encode_ticket(&my_addr)?;

        // Send initial ready event
        let initial_contacts = store.lock().get_contacts();
        let _ = evt_tx.send(MessengerEvent::EngineReady {
            my_node_id: my_node_id.clone(),
            my_ticket: my_ticket.clone(),
            my_nickname: my_nickname.clone(),
            contacts: initial_contacts,
        });

        let active_connections: Arc<RwLock<HashMap<PublicKey, Connection>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let blobs_store = MemStore::new();
        let blobs_proto = BlobsProtocol::new(&blobs_store, None);
        let gossip = Gossip::builder().spawn(endpoint.clone());
        let ping = Ping::new();

        let chat_proto = DootChatProtocol {
            active_connections: active_connections.clone(),
            connected_peers: connected_peers.clone(),
            store: store.clone(),
            evt_tx: evt_tx.clone(),
            pending_transfers: pending_transfers.clone(),
            downloads_dir: downloads_dir.clone(),
        };

        // Official Router::builder to manage multiple protocols over the single QUIC endpoint
        let router = Router::builder(endpoint.clone())
            .accept(iroh_blobs::ALPN, blobs_proto)
            .accept(GOSSIP_ALPN, gossip.clone())
            .accept(iroh_ping::ALPN, ping.clone())
            .accept(DOOT_CHAT_ALPN, chat_proto.clone())
            .accept(LEGACY_ALPN_NAME, chat_proto)
            .spawn();

        // Spawn periodic heartbeat to refresh connection path info and RTT
        let heartbeat_active = active_connections.clone();
        let heartbeat_evt = evt_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(4));
            loop {
                interval.tick().await;
                let conns = heartbeat_active.read().await;
                for (pk, conn) in conns.iter() {
                    if conn.close_reason().is_none() {
                        let ct = get_connection_type(conn);
                        if ct != ConnectionType::Unknown {
                            let _ = heartbeat_evt.send(MessengerEvent::ConnectionInfoUpdated {
                                peer_key: pk.to_string(),
                                connection_type: ct,
                            });
                        }
                    }
                }
            }
        });

        // Spawn command processor loop
        let engine_ep = endpoint.clone();
        let engine_active = active_connections.clone();
        let engine_connected = connected_peers.clone();
        let engine_store = store.clone();
        let engine_evt_tx = evt_tx.clone();
        let engine_pending = pending_transfers.clone();
        let engine_dl_dir = downloads_dir.clone();
        let engine_ping = ping.clone();
        let mut engine_identity = identity;

        tokio::spawn(async move {
            let mut cmd_rx = cmd_rx;
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    MessengerCommand::ConnectPeer {
                        ticket_or_id,
                        nickname,
                    } => {
                        handle_connect_peer(
                            &engine_ep,
                            &engine_active,
                            &engine_connected,
                            &engine_store,
                            &engine_evt_tx,
                            &engine_pending,
                            &engine_dl_dir,
                            ticket_or_id,
                            nickname,
                        )
                        .await;
                    }
                    MessengerCommand::SendTextMessage { peer_key, content } => {
                        handle_send_text(
                            &engine_ep,
                            &engine_active,
                            &engine_connected,
                            &engine_store,
                            &engine_evt_tx,
                            &engine_pending,
                            &engine_dl_dir,
                            &engine_identity.nickname,
                            peer_key,
                            content,
                        )
                        .await;
                    }
                    MessengerCommand::SendFile {
                        peer_key,
                        path,
                        is_directory,
                    } => {
                        handle_send_file(
                            &engine_ep,
                            &engine_active,
                            &engine_connected,
                            &engine_store,
                            &engine_evt_tx,
                            &engine_pending,
                            &engine_dl_dir,
                            &engine_identity.nickname,
                            peer_key,
                            path,
                            is_directory,
                        )
                        .await;
                    }
                    MessengerCommand::PingPeer { peer_key } => {
                        if let Ok(pk) = PublicKey::from_str(&peer_key) {
                            let maybe_ticket = engine_store
                                .lock()
                                .get_contact(&peer_key)
                                .and_then(|c| c.ticket);
                            let addr = if let Some(ticket) = maybe_ticket {
                                decode_ticket(&ticket).unwrap_or_else(|_| EndpointAddr::from(pk))
                            } else {
                                EndpointAddr::from(pk)
                            };
                            let ep = engine_ep.clone();
                            let ping_client = engine_ping.clone();
                            let evt_tx = engine_evt_tx.clone();
                            let active = engine_active.clone();
                            tokio::spawn(async move {
                                match ping_client.ping(&ep, addr).await {
                                    Ok(rtt) => {
                                        let rtt_ms = rtt.as_millis() as u32;
                                        let ct = if let Some(conn) = active.read().await.get(&pk) {
                                            let mut curr = get_connection_type(conn);
                                            match &mut curr {
                                                ConnectionType::Direct { rtt_ms: m, .. }
                                                | ConnectionType::Relay { rtt_ms: m, .. } => {
                                                    *m = rtt_ms;
                                                }
                                                _ => {}
                                            }
                                            curr
                                        } else {
                                            ConnectionType::Direct {
                                                addr: "iroh/ping/0".into(),
                                                rtt_ms,
                                            }
                                        };
                                        let _ =
                                            evt_tx.send(MessengerEvent::ConnectionInfoUpdated {
                                                peer_key,
                                                connection_type: ct,
                                            });
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed pinging peer {}: {}", peer_key, e);
                                    }
                                }
                            });
                        }
                    }
                    MessengerCommand::SendTyping {
                        peer_key,
                        is_typing,
                    } => {
                        handle_send_typing(&engine_ep, &engine_active, peer_key, is_typing).await;
                    }
                    MessengerCommand::DisconnectPeer { peer_key } => {
                        if let Ok(pk) = PublicKey::from_str(&peer_key) {
                            if let Some(conn) = engine_active.write().await.remove(&pk) {
                                conn.close(0u32.into(), b"disconnected by user");
                            }
                            engine_connected.lock().remove(&peer_key);
                            let _ =
                                engine_evt_tx.send(MessengerEvent::PeerDisconnected { peer_key });
                        }
                    }
                    MessengerCommand::UpdateNickname { nickname } => {
                        if let Err(e) = engine_identity.set_nickname(nickname.clone()) {
                            let _ = engine_evt_tx.send(MessengerEvent::Error {
                                context: "UpdateNickname".into(),
                                error: e.to_string(),
                            });
                        }
                    }
                    MessengerCommand::MarkConversationRead { peer_key } => {
                        let _ = engine_store.lock().mark_as_read(&peer_key);
                        let contacts = engine_store.lock().get_contacts();
                        let _ = engine_evt_tx.send(MessengerEvent::ContactListUpdated { contacts });
                    }
                }
            }
        });

        Ok(Self {
            my_node_id,
            my_ticket,
            my_nickname,
            data_dir,
            downloads_dir,
            router,
            blobs_store,
            gossip,
            ping,
            command_tx: cmd_tx,
            event_rx: Arc::new(Mutex::new(evt_rx)),
            store,
            connected_peers,
            pending_transfers,
        })
    }

    pub fn try_recv_event(&self) -> Option<MessengerEvent> {
        self.event_rx.lock().try_recv().ok()
    }

    pub fn send_command(&self, cmd: MessengerCommand) -> Result<()> {
        self.command_tx
            .send(cmd)
            .map_err(|_| anyhow::anyhow!("Background messenger engine has shut down"))
    }

    pub fn get_contacts(&self) -> Vec<Contact> {
        self.store.lock().get_contacts()
    }

    pub fn get_messages(&self, peer_key: &str) -> Vec<StoredMessage> {
        self.store.lock().get_messages(peer_key)
    }

    pub fn mark_as_read(&self, peer_key: &str) -> Result<()> {
        self.send_command(MessengerCommand::MarkConversationRead {
            peer_key: peer_key.to_string(),
        })
    }

    pub fn send_file(&self, peer_key: &str, path: PathBuf, is_directory: bool) -> Result<()> {
        self.send_command(MessengerCommand::SendFile {
            peer_key: peer_key.to_string(),
            path,
            is_directory,
        })
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.downloads_dir.clone()
    }

    pub fn get_pending_transfers(&self) -> Vec<PendingFileTransfer> {
        self.pending_transfers.lock().values().cloned().collect()
    }

    pub fn is_peer_connected(&self, peer_key: &str) -> bool {
        self.connected_peers.lock().contains(peer_key)
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_stream_listener(
    conn: Connection,
    remote_pk: PublicKey,
    event_tx: mpsc::UnboundedSender<MessengerEvent>,
    store: Arc<Mutex<Store>>,
    active_connections: Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: Arc<Mutex<HashSet<String>>>,
    pending_transfers: Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>>,
    downloads_dir: PathBuf,
) {
    let peer_key_str = remote_pk.to_string();
    tokio::spawn(async move {
        loop {
            match conn.accept_bi().await {
                Ok((mut send, mut recv)) => {
                    match recv_wire_message(&mut recv).await {
                        Ok(WireMessage::Text {
                            id,
                            sender_name,
                            timestamp_millis: _,
                            content,
                        }) => {
                            // Automatically acknowledge text receipt
                            let _ =
                                send_wire_message(&mut send, &WireMessage::Ack { message_id: id })
                                    .await;
                            let _ = send.finish();

                            let stored = StoredMessage {
                                id,
                                conversation_peer: peer_key_str.clone(),
                                direction: MessageDirection::Incoming,
                                sender_name,
                                content,
                                timestamp: Utc::now(),
                                status: MessageStatus::Delivered,
                                attachment: None,
                            };

                            {
                                let mut s = store.lock();
                                let _ = s.add_message(stored.clone());
                                let _ = s.update_contact_last_seen(&peer_key_str);
                            }

                            let _ =
                                event_tx.send(MessengerEvent::MessageReceived { message: stored });
                            let contacts = store.lock().get_contacts();
                            let _ = event_tx.send(MessengerEvent::ContactListUpdated { contacts });
                        }
                        Ok(WireMessage::FileOffer {
                            id,
                            sender_name,
                            timestamp_millis: _,
                            file_name,
                            file_size,
                            mime_type,
                            blake3_hash,
                            is_directory,
                        }) => {
                            // Acknowledge offer receipt
                            let _ =
                                send_wire_message(&mut send, &WireMessage::Ack { message_id: id })
                                    .await;
                            let _ = send.finish();

                            // Track pending file transfer details
                            pending_transfers.lock().insert(
                                id,
                                PendingFileTransfer {
                                    file_id: id,
                                    peer_key: peer_key_str.clone(),
                                    sender_name: sender_name.clone(),
                                    file_name: file_name.clone(),
                                    file_size,
                                    mime_type: mime_type.clone(),
                                    blake3_hash: blake3_hash.clone(),
                                    is_directory,
                                },
                            );

                            let attachment = FileAttachment {
                                file_id: id,
                                file_name: file_name.clone(),
                                file_size,
                                mime_type,
                                blake3_hash,
                                is_directory,
                                local_path: None,
                            };

                            let stored = StoredMessage {
                                id,
                                conversation_peer: peer_key_str.clone(),
                                direction: MessageDirection::Incoming,
                                sender_name,
                                content: format!(
                                    "Received {}: {}",
                                    if is_directory { "folder" } else { "file" },
                                    file_name
                                ),
                                timestamp: Utc::now(),
                                status: MessageStatus::Sending, // in-flight receiving
                                attachment: Some(attachment),
                            };

                            {
                                let mut s = store.lock();
                                let _ = s.add_message(stored.clone());
                                let _ = s.update_contact_last_seen(&peer_key_str);
                            }

                            let _ =
                                event_tx.send(MessengerEvent::MessageReceived { message: stored });
                            let contacts = store.lock().get_contacts();
                            let _ = event_tx.send(MessengerEvent::ContactListUpdated { contacts });
                        }
                        Ok(WireMessage::FileStreamStart { file_id }) => {
                            // Lookup pending transfer information
                            let pending_opt = pending_transfers.lock().get(&file_id).cloned();
                            if let Some(pending) = pending_opt {
                                let peer_key_clone = peer_key_str.clone();
                                let evt_tx = event_tx.clone();
                                let store_clone = store.clone();
                                let dl_dir = downloads_dir.clone();
                                tokio::spawn(async move {
                                    receive_file_stream(
                                        send,
                                        recv,
                                        pending,
                                        peer_key_clone,
                                        dl_dir,
                                        store_clone,
                                        evt_tx,
                                    )
                                    .await;
                                });
                            } else {
                                tracing::warn!(
                                    "Received FileStreamStart for unknown file_id {}",
                                    file_id
                                );
                            }
                        }
                        Ok(WireMessage::Ack { message_id }) => {
                            {
                                let mut s = store.lock();
                                let _ = s.update_message_status(
                                    &peer_key_str,
                                    message_id,
                                    MessageStatus::Delivered,
                                );
                            }
                            let _ = event_tx.send(MessengerEvent::MessageStatusUpdated {
                                peer_key: peer_key_str.clone(),
                                message_id,
                                status: MessageStatus::Delivered,
                            });
                        }
                        Ok(WireMessage::Typing { is_typing }) => {
                            let _ = event_tx.send(MessengerEvent::PeerTyping {
                                peer_key: peer_key_str.clone(),
                                is_typing,
                            });
                        }
                        Ok(WireMessage::Ping) => {
                            let _ = send_wire_message(&mut send, &WireMessage::Pong).await;
                            let _ = send.finish();
                        }
                        Ok(WireMessage::Pong) => {}
                        Err(e) => {
                            tracing::debug!("Stream ended or error reading wire message: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "Connection accept_bi terminated for {}: {}",
                        peer_key_str,
                        e
                    );
                    break;
                }
            }
        }

        // Clean up connection
        active_connections.write().await.remove(&remote_pk);
        connected_peers.lock().remove(&peer_key_str);
        let _ = event_tx.send(MessengerEvent::PeerDisconnected {
            peer_key: peer_key_str,
        });
    });
}

/// Receives raw binary file stream on dedicated QUIC bi-stream, verifies BLAKE3 hash,
/// and unpacks if directory.
async fn receive_file_stream(
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    pending: PendingFileTransfer,
    peer_key: String,
    downloads_dir: PathBuf,
    store: Arc<Mutex<Store>>,
    event_tx: mpsc::UnboundedSender<MessengerEvent>,
) {
    let file_id = pending.file_id;
    let _ = tokio::fs::create_dir_all(&downloads_dir).await;

    let temp_download_path = downloads_dir.join(format!(".tmp_{}.bin", file_id));
    let mut file = match tokio::fs::File::create(&temp_download_path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to create temp download file: {}", e);
            let _ = store.lock().update_message_status(
                &peer_key,
                file_id,
                MessageStatus::Failed(format!("Disk write error: {}", e)),
            );
            return;
        }
    };

    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 128 * 1024];
    let mut transferred = 0u64;
    let total_bytes = pending.file_size;
    let mut last_progress_time = std::time::Instant::now();
    let mut stream_ok = true;

    while let Ok(Some(n)) = recv.read(&mut buffer).await {
        if let Err(e) = file.write_all(&buffer[..n]).await {
            tracing::error!("Failed writing to download file: {}", e);
            stream_ok = false;
            break;
        }
        hasher.update(&buffer[..n]);
        transferred += n as u64;

        if last_progress_time.elapsed() >= Duration::from_millis(150) || transferred == total_bytes
        {
            last_progress_time = std::time::Instant::now();
            let _ = event_tx.send(MessengerEvent::FileTransferProgress {
                peer_key: peer_key.clone(),
                file_id,
                bytes_transferred: transferred,
                total_bytes,
                is_outgoing: false,
            });
        }
    }

    let _ = file.flush().await;
    drop(file);

    if !stream_ok {
        let _ = tokio::fs::remove_file(&temp_download_path).await;
        let _ = store.lock().update_message_status(
            &peer_key,
            file_id,
            MessageStatus::Failed("Stream interrupted".into()),
        );
        return;
    }

    let calculated_hash = hasher.finalize().to_hex().to_string();
    if calculated_hash != pending.blake3_hash {
        tracing::error!(
            "BLAKE3 hash mismatch for {}: expected {}, got {}",
            pending.file_name,
            pending.blake3_hash,
            calculated_hash
        );
        let _ = tokio::fs::remove_file(&temp_download_path).await;
        let _ = store.lock().update_message_status(
            &peer_key,
            file_id,
            MessageStatus::Failed("Integrity check failed: BLAKE3 hash mismatch".into()),
        );
        return;
    }

    // Hash matches! Send 1-byte ACK back to sender
    let _ = send.write_all(&[1u8]).await;
    let _ = send.finish();

    let final_target_path: PathBuf = if pending.is_directory {
        let target_folder = unique_path(&downloads_dir, &pending.file_name);
        match folder::unpack_directory_from_file(&temp_download_path, &target_folder) {
            Ok(()) => {
                let _ = tokio::fs::remove_file(&temp_download_path).await;
                target_folder
            }
            Err(e) => {
                tracing::error!("Failed unpacking folder {}: {}", pending.file_name, e);
                let _ = store.lock().update_message_status(
                    &peer_key,
                    file_id,
                    MessageStatus::Failed(format!("Folder unpack error: {}", e)),
                );
                return;
            }
        }
    } else {
        let final_path = unique_path(&downloads_dir, &pending.file_name);
        if let Err(e) = tokio::fs::rename(&temp_download_path, &final_path).await {
            tracing::error!("Failed moving downloaded file: {}", e);
            temp_download_path
        } else {
            final_path
        }
    };

    let local_path_str = final_target_path.to_string_lossy().to_string();
    {
        let mut s = store.lock();
        let _ = s.update_attachment_local_path(&peer_key, file_id, local_path_str.clone());
        let _ = s.update_message_status(&peer_key, file_id, MessageStatus::Delivered);
    }

    let _ = event_tx.send(MessengerEvent::FileTransferProgress {
        peer_key: peer_key.clone(),
        file_id,
        bytes_transferred: total_bytes,
        total_bytes,
        is_outgoing: false,
    });
    let _ = event_tx.send(MessengerEvent::FileTransferComplete {
        peer_key: peer_key.clone(),
        file_id,
        local_path: local_path_str,
        is_outgoing: false,
    });
    let _ = event_tx.send(MessengerEvent::MessageStatusUpdated {
        peer_key,
        message_id: file_id,
        status: MessageStatus::Delivered,
    });
}

#[allow(clippy::too_many_arguments)]
async fn handle_connect_peer(
    endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: &Arc<Mutex<HashSet<String>>>,
    store: &Arc<Mutex<Store>>,
    event_tx: &mpsc::UnboundedSender<MessengerEvent>,
    pending_transfers: &Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>>,
    downloads_dir: &Path,
    ticket_or_id: String,
    nickname: Option<String>,
) {
    let addr = match decode_ticket(&ticket_or_id) {
        Ok(a) => a,
        Err(e) => {
            let _ = event_tx.send(MessengerEvent::Error {
                context: "ConnectPeer".into(),
                error: format!("Invalid ticket/address '{}': {}", ticket_or_id, e),
            });
            return;
        }
    };

    let peer_pk = addr.id;
    let peer_key_str = peer_pk.to_string();

    let _ = event_tx.send(MessengerEvent::PeerConnecting {
        peer_key: peer_key_str.clone(),
    });

    // Check if already active
    if let Some(conn) = active_connections.read().await.get(&peer_pk) {
        if conn.close_reason().is_none() {
            let conn_type = get_connection_type(conn);
            let direct_addr = match &conn_type {
                ConnectionType::Direct { addr, .. } => Some(addr.clone()),
                _ => None,
            };
            let _ = event_tx.send(MessengerEvent::PeerConnected {
                peer_key: peer_key_str,
                direct_addr,
                connection_type: conn_type,
            });
            return;
        }
    }

    // Add contact immediately so the UI reflects the pending peer
    let display_name = nickname.unwrap_or_else(|| {
        let short = if peer_key_str.len() > 6 {
            &peer_key_str[..6]
        } else {
            &peer_key_str
        };
        format!("Peer-{}", short)
    });

    let contact = Contact {
        public_key: peer_key_str.clone(),
        nickname: display_name,
        ticket: Some(ticket_or_id.clone()),
        last_seen: None,
        unread_count: 0,
        last_message_preview: None,
    };

    let _ = store.lock().add_or_update_contact(contact);
    let contacts = store.lock().get_contacts();
    let _ = event_tx.send(MessengerEvent::ContactListUpdated { contacts });

    // Dial peer with 12-second timeout
    tracing::info!("Dialing peer {}", peer_key_str);
    match tokio::time::timeout(
        Duration::from_secs(12),
        endpoint.connect(addr.clone(), DOOT_CHAT_ALPN),
    )
    .await
    {
        Ok(Ok(conn)) => {
            tracing::info!("Successfully connected to peer {}", peer_key_str);

            active_connections
                .write()
                .await
                .insert(peer_pk, conn.clone());
            connected_peers.lock().insert(peer_key_str.clone());

            let _ = store.lock().update_contact_last_seen(&peer_key_str);
            let contacts = store.lock().get_contacts();
            let _ = event_tx.send(MessengerEvent::ContactListUpdated { contacts });

            let initial_conn_type = get_connection_type(&conn);
            let direct_addr = match &initial_conn_type {
                ConnectionType::Direct { addr, .. } => Some(addr.clone()),
                _ => None,
            };

            let _ = event_tx.send(MessengerEvent::PeerConnected {
                peer_key: peer_key_str.clone(),
                direct_addr,
                connection_type: initial_conn_type,
            });

            // Spawn path events watcher
            spawn_path_watcher(conn.clone(), peer_key_str.clone(), event_tx.clone());

            // Spawn listener for incoming streams from this dialed peer
            spawn_stream_listener(
                conn,
                peer_pk,
                event_tx.clone(),
                store.clone(),
                active_connections.clone(),
                connected_peers.clone(),
                pending_transfers.clone(),
                downloads_dir.to_path_buf(),
            );
        }
        Ok(Err(err)) => {
            tracing::warn!("Failed connecting to peer {}: {}", peer_key_str, err);
            let _ = event_tx.send(MessengerEvent::Error {
                context: format!("Connect to {}", peer_key_str),
                error: format!("Connection failed: {}", err),
            });
        }
        Err(_) => {
            tracing::warn!("Connection to peer {} timed out", peer_key_str);
            let _ = event_tx.send(MessengerEvent::Error {
                context: format!("Connect to {}", peer_key_str),
                error: "Connection timed out (peer unreachable or offline)".into(),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn get_or_dial_connection(
    endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: &Arc<Mutex<HashSet<String>>>,
    store: &Arc<Mutex<Store>>,
    event_tx: &mpsc::UnboundedSender<MessengerEvent>,
    pending_transfers: &Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>>,
    downloads_dir: &Path,
    peer_pk: PublicKey,
) -> Result<Connection> {
    // 1. Check existing connection
    if let Some(conn) = active_connections.read().await.get(&peer_pk) {
        if conn.close_reason().is_none() {
            return Ok(conn.clone());
        }
    }

    // 2. Need to dial
    let peer_key_str = peer_pk.to_string();
    let maybe_ticket = store
        .lock()
        .get_contact(&peer_key_str)
        .and_then(|c| c.ticket);

    let addr = if let Some(ticket) = maybe_ticket {
        decode_ticket(&ticket).unwrap_or_else(|_| EndpointAddr::from(peer_pk))
    } else {
        EndpointAddr::from(peer_pk)
    };

    let conn = endpoint
        .connect(addr, DOOT_CHAT_ALPN)
        .await
        .context(format!("Failed to connect to peer {}", peer_key_str))?;

    active_connections
        .write()
        .await
        .insert(peer_pk, conn.clone());
    connected_peers.lock().insert(peer_key_str.clone());

    let initial_conn_type = get_connection_type(&conn);
    let direct_addr = match &initial_conn_type {
        ConnectionType::Direct { addr, .. } => Some(addr.clone()),
        _ => None,
    };

    let _ = event_tx.send(MessengerEvent::PeerConnected {
        peer_key: peer_key_str.clone(),
        direct_addr,
        connection_type: initial_conn_type,
    });

    spawn_path_watcher(conn.clone(), peer_key_str.clone(), event_tx.clone());

    spawn_stream_listener(
        conn.clone(),
        peer_pk,
        event_tx.clone(),
        store.clone(),
        active_connections.clone(),
        connected_peers.clone(),
        pending_transfers.clone(),
        downloads_dir.to_path_buf(),
    );

    Ok(conn)
}

#[allow(clippy::too_many_arguments)]
async fn handle_send_text(
    endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: &Arc<Mutex<HashSet<String>>>,
    store: &Arc<Mutex<Store>>,
    event_tx: &mpsc::UnboundedSender<MessengerEvent>,
    pending_transfers: &Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>>,
    downloads_dir: &Path,
    my_name: &str,
    peer_key: String,
    content: String,
) {
    let peer_pk = match PublicKey::from_str(&peer_key) {
        Ok(pk) => pk,
        Err(e) => {
            let _ = event_tx.send(MessengerEvent::Error {
                context: "SendTextMessage".into(),
                error: format!("Invalid peer public key: {}", e),
            });
            return;
        }
    };

    let message_id = Uuid::new_v4();
    let stored = StoredMessage {
        id: message_id,
        conversation_peer: peer_key.clone(),
        direction: MessageDirection::Outgoing,
        sender_name: my_name.to_string(),
        content: content.clone(),
        timestamp: Utc::now(),
        status: MessageStatus::Sending,
        attachment: None,
    };

    // Save to store and emit sent event immediately
    {
        let mut s = store.lock();
        let _ = s.add_message(stored.clone());
    }
    let _ = event_tx.send(MessengerEvent::MessageSent {
        message: stored.clone(),
    });
    let contacts = store.lock().get_contacts();
    let _ = event_tx.send(MessengerEvent::ContactListUpdated { contacts });

    // Ensure connection
    let conn = match get_or_dial_connection(
        endpoint,
        active_connections,
        connected_peers,
        store,
        event_tx,
        pending_transfers,
        downloads_dir,
        peer_pk,
    )
    .await
    {
        Ok(c) => c,
        Err(err) => {
            let error_msg = err.to_string();
            {
                let mut s = store.lock();
                let _ = s.update_message_status(
                    &peer_key,
                    message_id,
                    MessageStatus::Failed(error_msg.clone()),
                );
            }
            let _ = event_tx.send(MessengerEvent::MessageStatusUpdated {
                peer_key,
                message_id,
                status: MessageStatus::Failed(error_msg),
            });
            return;
        }
    };

    // Send on bi-stream
    let wire_msg = WireMessage::Text {
        id: message_id,
        sender_name: my_name.to_string(),
        timestamp_millis: Utc::now().timestamp_millis(),
        content,
    };

    let send_result: Result<MessageStatus> = async {
        let (mut send, mut recv) = conn.open_bi().await?;
        send_wire_message(&mut send, &wire_msg).await?;
        send.finish()?;

        // Wait up to 5s for delivery Ack
        match timeout(Duration::from_secs(5), recv_wire_message(&mut recv)).await {
            Ok(Ok(WireMessage::Ack { message_id: ack_id })) if ack_id == message_id => {
                Ok(MessageStatus::Delivered)
            }
            _ => Ok(MessageStatus::Sent),
        }
    }
    .await;

    let final_status = match send_result {
        Ok(status) => status,
        Err(e) => MessageStatus::Failed(e.to_string()),
    };

    {
        let mut s = store.lock();
        let _ = s.update_message_status(&peer_key, message_id, final_status.clone());
    }

    let _ = event_tx.send(MessengerEvent::MessageStatusUpdated {
        peer_key,
        message_id,
        status: final_status,
    });
}

#[allow(clippy::too_many_arguments)]
async fn handle_send_file(
    endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: &Arc<Mutex<HashSet<String>>>,
    store: &Arc<Mutex<Store>>,
    event_tx: &mpsc::UnboundedSender<MessengerEvent>,
    pending_transfers: &Arc<Mutex<HashMap<Uuid, PendingFileTransfer>>>,
    downloads_dir: &Path,
    my_name: &str,
    peer_key: String,
    path: PathBuf,
    is_directory: bool,
) {
    if !path.exists() {
        let _ = event_tx.send(MessengerEvent::Error {
            context: "SendFile".into(),
            error: format!("Path does not exist: {:?}", path),
        });
        return;
    }

    let peer_pk = match PublicKey::from_str(&peer_key) {
        Ok(pk) => pk,
        Err(e) => {
            let _ = event_tx.send(MessengerEvent::Error {
                context: "SendFile".into(),
                error: format!("Invalid peer key: {}", e),
            });
            return;
        }
    };

    let file_id = Uuid::new_v4();
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    // Prepare payload source file
    let (source_file_path, file_size, blake3_hash, mime_type, cleanup_needed) = if is_directory {
        let temp_archive = downloads_dir.join(format!(".send_{}.irdir", file_id));
        match folder::pack_directory_to_file(&path, &temp_archive) {
            Ok(size) => match folder::compute_file_blake3(&temp_archive) {
                Ok((hash, _)) => (
                    temp_archive,
                    size,
                    hash,
                    "application/x-directory".to_string(),
                    true,
                ),
                Err(e) => {
                    let _ = event_tx.send(MessengerEvent::Error {
                        context: "SendFile".into(),
                        error: format!("Failed computing archive hash: {}", e),
                    });
                    return;
                }
            },
            Err(e) => {
                let _ = event_tx.send(MessengerEvent::Error {
                    context: "SendFile".into(),
                    error: format!("Failed packing directory: {}", e),
                });
                return;
            }
        }
    } else {
        match folder::compute_file_blake3(&path) {
            Ok((hash, size)) => {
                let mime = folder::detect_mime_type(&file_name, false);
                (path.clone(), size, hash, mime.to_string(), false)
            }
            Err(e) => {
                let _ = event_tx.send(MessengerEvent::Error {
                    context: "SendFile".into(),
                    error: format!("Failed reading file: {}", e),
                });
                return;
            }
        }
    };

    let attachment = FileAttachment {
        file_id,
        file_name: file_name.clone(),
        file_size,
        mime_type: mime_type.clone(),
        blake3_hash: blake3_hash.clone(),
        is_directory,
        local_path: Some(path.to_string_lossy().to_string()),
    };

    let stored = StoredMessage {
        id: file_id,
        conversation_peer: peer_key.clone(),
        direction: MessageDirection::Outgoing,
        sender_name: my_name.to_string(),
        content: format!(
            "Sent {}: {}",
            if is_directory { "folder" } else { "file" },
            file_name
        ),
        timestamp: Utc::now(),
        status: MessageStatus::Sending,
        attachment: Some(attachment),
    };

    {
        let mut s = store.lock();
        let _ = s.add_message(stored.clone());
    }
    let _ = event_tx.send(MessengerEvent::MessageSent {
        message: stored.clone(),
    });
    let contacts = store.lock().get_contacts();
    let _ = event_tx.send(MessengerEvent::ContactListUpdated { contacts });

    // Connect
    let conn = match get_or_dial_connection(
        endpoint,
        active_connections,
        connected_peers,
        store,
        event_tx,
        pending_transfers,
        downloads_dir,
        peer_pk,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            if cleanup_needed {
                let _ = tokio::fs::remove_file(&source_file_path).await;
            }
            let err_str = e.to_string();
            let _ = store.lock().update_message_status(
                &peer_key,
                file_id,
                MessageStatus::Failed(err_str.clone()),
            );
            let _ = event_tx.send(MessengerEvent::MessageStatusUpdated {
                peer_key,
                message_id: file_id,
                status: MessageStatus::Failed(err_str),
            });
            return;
        }
    };

    // Send FileOffer on a bi-stream
    let offer = WireMessage::FileOffer {
        id: file_id,
        sender_name: my_name.to_string(),
        timestamp_millis: Utc::now().timestamp_millis(),
        file_name,
        file_size,
        mime_type,
        blake3_hash,
        is_directory,
    };

    let offer_res: Result<()> = async {
        let (mut send, mut recv) = conn.open_bi().await?;
        send_wire_message(&mut send, &offer).await?;
        send.finish()?;
        let _ = timeout(Duration::from_secs(5), recv_wire_message(&mut recv)).await?;
        Ok(())
    }
    .await;

    if let Err(e) = offer_res {
        tracing::warn!("Failed sending file offer: {}", e);
    }

    // Now open dedicated transfer stream and stream the file data
    let peer_key_for_stream = peer_key.clone();
    let evt_tx = event_tx.clone();
    let store_clone = store.clone();
    let original_path_str = path.to_string_lossy().to_string();

    tokio::spawn(async move {
        let send_res: Result<()> = async {
            let (mut send, mut recv) = conn.open_bi().await?;
            send_wire_message(&mut send, &WireMessage::FileStreamStart { file_id }).await?;

            let mut file = tokio::fs::File::open(&source_file_path).await?;
            let mut buffer = [0u8; 128 * 1024];
            let mut transferred = 0u64;
            let mut last_progress_time = std::time::Instant::now();

            loop {
                let n = file.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }
                send.write_all(&buffer[..n]).await?;
                transferred += n as u64;

                if last_progress_time.elapsed() >= Duration::from_millis(150)
                    || transferred == file_size
                {
                    last_progress_time = std::time::Instant::now();
                    let _ = evt_tx.send(MessengerEvent::FileTransferProgress {
                        peer_key: peer_key_for_stream.clone(),
                        file_id,
                        bytes_transferred: transferred,
                        total_bytes: file_size,
                        is_outgoing: true,
                    });
                }
            }

            send.finish()?;

            // Wait for receiver 1-byte ACK (with 30s timeout)
            let mut ack = [0u8; 1];
            let read_res = timeout(Duration::from_secs(30), recv.read(&mut ack)).await??;
            if read_res != Some(1) || ack[0] != 1u8 {
                anyhow::bail!("Receiver rejected or corrupted transfer");
            }
            Ok(())
        }
        .await;

        if cleanup_needed {
            let _ = tokio::fs::remove_file(&source_file_path).await;
        }

        match send_res {
            Ok(()) => {
                {
                    let mut s = store_clone.lock();
                    let _ = s.update_message_status(
                        &peer_key_for_stream,
                        file_id,
                        MessageStatus::Delivered,
                    );
                }
                let _ = evt_tx.send(MessengerEvent::FileTransferProgress {
                    peer_key: peer_key_for_stream.clone(),
                    file_id,
                    bytes_transferred: file_size,
                    total_bytes: file_size,
                    is_outgoing: true,
                });
                let _ = evt_tx.send(MessengerEvent::FileTransferComplete {
                    peer_key: peer_key_for_stream.clone(),
                    file_id,
                    local_path: original_path_str,
                    is_outgoing: true,
                });
                let _ = evt_tx.send(MessengerEvent::MessageStatusUpdated {
                    peer_key: peer_key_for_stream,
                    message_id: file_id,
                    status: MessageStatus::Delivered,
                });
            }
            Err(e) => {
                let err_msg = e.to_string();
                {
                    let mut s = store_clone.lock();
                    let _ = s.update_message_status(
                        &peer_key_for_stream,
                        file_id,
                        MessageStatus::Failed(err_msg.clone()),
                    );
                }
                let _ = evt_tx.send(MessengerEvent::MessageStatusUpdated {
                    peer_key: peer_key_for_stream,
                    message_id: file_id,
                    status: MessageStatus::Failed(err_msg),
                });
            }
        }
    });
}

async fn handle_send_typing(
    _endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    peer_key: String,
    is_typing: bool,
) {
    if let Ok(peer_pk) = PublicKey::from_str(&peer_key) {
        if let Some(conn) = active_connections.read().await.get(&peer_pk) {
            if let Ok((mut send, _)) = conn.open_bi().await {
                let _ = send_wire_message(&mut send, &WireMessage::Typing { is_typing }).await;
                let _ = send.finish();
            }
        }
    }
}

/// Generates a non-colliding destination path if file/dir already exists in target directory.
fn unique_path(dir: &Path, file_name: &str) -> PathBuf {
    let mut path = dir.join(file_name);
    if !path.exists() {
        return path;
    }
    let p = Path::new(file_name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mut i = 1;
    loop {
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };
        path = dir.join(&new_name);
        if !path.exists() {
            return path;
        }
        i += 1;
    }
}
