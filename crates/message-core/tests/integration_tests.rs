use message_core::protocol::{decode_ticket, encode_ticket};
use message_core::store::{Contact, MessageDirection, MessageStatus, Store, StoredMessage};
use message_core::{MessengerCommand, MessengerHandle};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[test]
fn test_ticket_roundtrip() {
    let secret = iroh::SecretKey::generate();
    let pk = secret.public();
    let addr = iroh::EndpointAddr::from(pk);

    let ticket = encode_ticket(&addr).expect("encode failed");
    assert!(ticket.starts_with("iroh-msg:"));

    let decoded = decode_ticket(&ticket).expect("decode failed");
    assert_eq!(decoded.id, pk);

    // Test raw hex decoding
    let raw_pk_str = pk.to_string();
    let decoded_from_str = decode_ticket(&raw_pk_str).expect("raw decode failed");
    assert_eq!(decoded_from_str.id, pk);
}

#[test]
fn test_store_operations() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut store = Store::load(temp_dir.path().to_path_buf()).expect("store load");

    let contact = Contact {
        public_key: "abc123def456".to_string(),
        nickname: "Alice".to_string(),
        ticket: Some("iroh-msg:xyz".to_string()),
        last_seen: None,
        unread_count: 0,
        last_message_preview: None,
    };
    store
        .add_or_update_contact(contact.clone())
        .expect("add contact");

    let msg_id = Uuid::new_v4();
    let msg = StoredMessage {
        id: msg_id,
        conversation_peer: "abc123def456".to_string(),
        direction: MessageDirection::Incoming,
        sender_name: "Alice".to_string(),
        content: "Hello from test!".to_string(),
        timestamp: chrono::Utc::now(),
        status: MessageStatus::Delivered,
    };
    store.add_message(msg).expect("add message");

    let contacts = store.get_contacts();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].unread_count, 1);
    assert_eq!(
        contacts[0].last_message_preview.as_deref(),
        Some("Hello from test!")
    );

    let messages = store.get_messages("abc123def456");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello from test!");

    store.mark_as_read("abc123def456").expect("mark read");
    let contacts = store.get_contacts();
    assert_eq!(contacts[0].unread_count, 0);
}

#[tokio::test]
async fn test_p2p_messaging_two_nodes() {
    let dir_a = tempfile::tempdir().expect("tempdir a");
    let dir_b = tempfile::tempdir().expect("tempdir b");

    let node_a = MessengerHandle::start(Some(dir_a.path().to_path_buf()), Some("NodeA".into()))
        .await
        .expect("start node a");

    let node_b = MessengerHandle::start(Some(dir_b.path().to_path_buf()), Some("NodeB".into()))
        .await
        .expect("start node b");

    println!("Node A ticket: {}", node_a.my_ticket);
    println!("Node B ticket: {}", node_b.my_ticket);

    // Node A connects to Node B using Node B's shareable ticket
    node_a
        .send_command(MessengerCommand::ConnectPeer {
            ticket_or_id: node_b.my_ticket.clone(),
            nickname: Some("FriendB".into()),
        })
        .expect("send connect");

    // Wait a brief moment for QUIC handshake to finish
    let mut connected = false;
    for _ in 0..30 {
        sleep(Duration::from_millis(150)).await;
        if node_a.is_peer_connected(&node_b.my_node_id) {
            connected = true;
            break;
        }
    }
    assert!(connected, "Node A should connect to Node B over Iroh QUIC");

    // Node A sends text message to Node B
    node_a
        .send_command(MessengerCommand::SendTextMessage {
            peer_key: node_b.my_node_id.clone(),
            content: "Hey B, this is A over Iroh QUIC!".into(),
        })
        .expect("send text");

    // Check Node B receives message
    let mut message_received = false;
    for _ in 0..30 {
        sleep(Duration::from_millis(150)).await;
        let msgs = node_b.get_messages(&node_a.my_node_id);
        if let Some(m) = msgs.first() {
            if m.content == "Hey B, this is A over Iroh QUIC!" {
                message_received = true;
                break;
            }
        }
    }
    assert!(
        message_received,
        "Node B should receive the message from Node A"
    );

    // Check Node B sends a reply
    node_b
        .send_command(MessengerCommand::SendTextMessage {
            peer_key: node_a.my_node_id.clone(),
            content: "Hello back A! Direct P2P verified.".into(),
        })
        .expect("reply text");

    let mut reply_received = false;
    for _ in 0..30 {
        sleep(Duration::from_millis(150)).await;
        let msgs = node_a.get_messages(&node_b.my_node_id);
        if msgs
            .iter()
            .any(|m| m.content == "Hello back A! Direct P2P verified.")
        {
            reply_received = true;
            break;
        }
    }
    assert!(
        reply_received,
        "Node A should receive the reply from Node B"
    );
}
