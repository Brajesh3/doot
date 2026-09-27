use crate::store::{Contact, MessageStatus, StoredMessage};
use uuid::Uuid;

#[derive(Debug)]
pub enum MessengerCommand {
    ConnectPeer {
        ticket_or_id: String,
        nickname: Option<String>,
    },
    DisconnectPeer {
        peer_key: String,
    },
    SendTextMessage {
        peer_key: String,
        content: String,
    },
    SendTyping {
        peer_key: String,
        is_typing: bool,
    },
    UpdateNickname {
        nickname: String,
    },
    MarkConversationRead {
        peer_key: String,
    },
    SendFile {
        peer_key: String,
        path: std::path::PathBuf,
        is_directory: bool,
    },
    PingPeer {
        peer_key: String,
    },
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum ConnectionType {
    Direct { addr: String, rtt_ms: u32 },
    Relay { url: String, rtt_ms: u32 },
    Unknown,
}

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::Direct { addr, rtt_ms } => {
                write!(f, "Direct P2P ({} • {}ms)", addr, rtt_ms)
            }
            ConnectionType::Relay { url, rtt_ms } => write!(f, "Relayed ({} • {}ms)", url, rtt_ms),
            ConnectionType::Unknown => write!(f, "Connecting..."),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessengerEvent {
    EngineReady {
        my_node_id: String,
        my_ticket: String,
        my_nickname: String,
        contacts: Vec<Contact>,
    },
    PeerConnecting {
        peer_key: String,
    },
    PeerConnected {
        peer_key: String,
        direct_addr: Option<String>,
        connection_type: ConnectionType,
    },
    ConnectionInfoUpdated {
        peer_key: String,
        connection_type: ConnectionType,
    },
    PeerDisconnected {
        peer_key: String,
    },
    PeerTyping {
        peer_key: String,
        is_typing: bool,
    },
    MessageReceived {
        message: StoredMessage,
    },
    MessageSent {
        message: StoredMessage,
    },
    MessageStatusUpdated {
        peer_key: String,
        message_id: Uuid,
        status: MessageStatus,
    },
    ContactListUpdated {
        contacts: Vec<Contact>,
    },
    FileTransferProgress {
        peer_key: String,
        file_id: Uuid,
        bytes_transferred: u64,
        total_bytes: u64,
        is_outgoing: bool,
    },
    FileTransferComplete {
        peer_key: String,
        file_id: Uuid,
        local_path: String,
        is_outgoing: bool,
    },
    Error {
        context: String,
        error: String,
    },
}
