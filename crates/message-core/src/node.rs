use crate::events::{MessengerCommand, MessengerEvent};
use crate::identity::Identity;
use crate::protocol::{
    decode_ticket, encode_ticket, recv_wire_message, send_wire_message, WireMessage, ALPN_NAME,
};
use crate::store::{Contact, MessageDirection, MessageStatus, Store, StoredMessage};
use anyhow::{Context, Result};
use chrono::Utc;
use iroh::endpoint::Connection;
use iroh::{endpoint::presets, Endpoint, EndpointAddr, PublicKey};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Clone)]
pub struct MessengerHandle {
    pub my_node_id: String,
    pub my_ticket: String,
    pub my_nickname: String,
    pub data_dir: PathBuf,
    command_tx: mpsc::UnboundedSender<MessengerCommand>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<MessengerEvent>>>,
    store: Arc<Mutex<Store>>,
    connected_peers: Arc<Mutex<HashSet<String>>>,
}

impl MessengerHandle {
    pub async fn start(
        custom_dir: Option<PathBuf>,
        default_nickname: Option<String>,
    ) -> Result<Self> {
        let identity = Identity::load_or_create(custom_dir, default_nickname)?;
        let store = Arc::new(Mutex::new(Store::load(identity.data_dir.clone())?));
        let connected_peers = Arc::new(Mutex::new(HashSet::new()));

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (evt_tx, evt_rx) = mpsc::unbounded_channel();

        let my_node_id = identity.public_key.to_string();
        let my_nickname = identity.nickname.clone();
        let data_dir = identity.data_dir.clone();

        // Bind Iroh Endpoint
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(identity.secret_key.clone())
            .alpns(vec![ALPN_NAME.to_vec()])
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

        // Spawn incoming connections acceptor
        let accept_ep = endpoint.clone();
        let accept_evt_tx = evt_tx.clone();
        let accept_store = store.clone();
        let accept_active = active_connections.clone();
        let accept_connected = connected_peers.clone();

        tokio::spawn(async move {
            tracing::info!("Iroh messenger listening for incoming connections...");
            while let Some(incoming) = accept_ep.accept().await {
                match incoming.await {
                    Ok(conn) => {
                        let remote_pk = conn.remote_id();
                        let peer_key_str = remote_pk.to_string();
                        tracing::info!("Accepted incoming connection from peer {}", peer_key_str);

                        accept_active.write().await.insert(remote_pk, conn.clone());
                        accept_connected.lock().insert(peer_key_str.clone());

                        let _ = accept_store.lock().update_contact_last_seen(&peer_key_str);
                        let _ = accept_evt_tx.send(MessengerEvent::PeerConnected {
                            peer_key: peer_key_str.clone(),
                            direct_addr: None,
                        });

                        // Spawn stream reader loop for this connection
                        spawn_stream_listener(
                            conn,
                            remote_pk,
                            accept_evt_tx.clone(),
                            accept_store.clone(),
                            accept_active.clone(),
                            accept_connected.clone(),
                        );
                    }
                    Err(err) => {
                        tracing::debug!("Incoming connection error: {}", err);
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
                            &engine_identity.nickname,
                            peer_key,
                            content,
                        )
                        .await;
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
            command_tx: cmd_tx,
            event_rx: Arc::new(Mutex::new(evt_rx)),
            store,
            connected_peers,
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

    pub fn is_peer_connected(&self, peer_key: &str) -> bool {
        self.connected_peers.lock().contains(peer_key)
    }
}

fn spawn_stream_listener(
    conn: Connection,
    remote_pk: PublicKey,
    event_tx: mpsc::UnboundedSender<MessengerEvent>,
    store: Arc<Mutex<Store>>,
    active_connections: Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: Arc<Mutex<HashSet<String>>>,
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

async fn handle_connect_peer(
    endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: &Arc<Mutex<HashSet<String>>>,
    store: &Arc<Mutex<Store>>,
    event_tx: &mpsc::UnboundedSender<MessengerEvent>,
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
            let _ = event_tx.send(MessengerEvent::PeerConnected {
                peer_key: peer_key_str,
                direct_addr: None,
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
    match tokio::time::timeout(Duration::from_secs(12), endpoint.connect(addr.clone(), ALPN_NAME)).await {
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

            let _ = event_tx.send(MessengerEvent::PeerConnected {
                peer_key: peer_key_str,
                direct_addr: None,
            });

            // Spawn listener for incoming streams from this dialed peer
            spawn_stream_listener(
                conn,
                peer_pk,
                event_tx.clone(),
                store.clone(),
                active_connections.clone(),
                connected_peers.clone(),
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

async fn get_or_dial_connection(
    endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: &Arc<Mutex<HashSet<String>>>,
    store: &Arc<Mutex<Store>>,
    event_tx: &mpsc::UnboundedSender<MessengerEvent>,
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
        .connect(addr, ALPN_NAME)
        .await
        .context(format!("Failed to connect to peer {}", peer_key_str))?;

    active_connections
        .write()
        .await
        .insert(peer_pk, conn.clone());
    connected_peers.lock().insert(peer_key_str.clone());

    spawn_stream_listener(
        conn.clone(),
        peer_pk,
        event_tx.clone(),
        store.clone(),
        active_connections.clone(),
        connected_peers.clone(),
    );

    let _ = event_tx.send(MessengerEvent::PeerConnected {
        peer_key: peer_key_str,
        direct_addr: None,
    });

    Ok(conn)
}

async fn handle_send_text(
    endpoint: &Endpoint,
    active_connections: &Arc<RwLock<HashMap<PublicKey, Connection>>>,
    connected_peers: &Arc<Mutex<HashSet<String>>>,
    store: &Arc<Mutex<Store>>,
    event_tx: &mpsc::UnboundedSender<MessengerEvent>,
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
