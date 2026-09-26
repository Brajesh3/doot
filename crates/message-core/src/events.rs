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
}

use serde::{Deserialize, Serialize};

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
    Error {
        context: String,
        error: String,
    },
}
