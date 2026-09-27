use anyhow::Result;
use message_core::{MessengerCommand, MessengerEvent, MessengerHandle};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n🚀 Initializing Iroh Test Peer Bot...");

    let temp_dir = std::env::temp_dir().join("iroh_bot_profile");
    let handle = MessengerHandle::start(Some(temp_dir), Some("Echo Bot 🤖".into())).await?;

    println!("\n=======================================================");
    println!("🤖 BOT NODE ID : {}", handle.my_node_id);
    println!("🤖 BOT TICKET  : {}", handle.my_ticket);
    println!("=======================================================\n");
    println!("👉 STEP 1: Copy the BOT TICKET above.");
    println!("👉 STEP 2: Open your Iroh Desktop Messenger window.");
    println!("👉 STEP 3: Click '+ New Chat', paste this ticket, and click 'Connect & Chat'.");
    println!("👉 STEP 4: Send any message! The bot will receive it over Iroh QUIC and reply.\n");
    println!("Listening for peer connections and messages...\n");

    loop {
        while let Some(event) = handle.try_recv_event() {
            match event {
                MessengerEvent::PeerConnected { peer_key, .. } => {
                    println!("🟢 [CONNECTED] Peer connected: {}", peer_key);
                }
                MessengerEvent::PeerDisconnected { peer_key } => {
                    println!("🔴 [DISCONNECTED] Peer disconnected: {}", peer_key);
                }
                MessengerEvent::MessageReceived { message } => {
                    println!(
                        "📩 [RECEIVED] From {}: \"{}\"",
                        message.sender_name, message.content
                    );

                    // Send typing indicator
                    let _ = handle.send_command(MessengerCommand::SendTyping {
                        peer_key: message.conversation_peer.clone(),
                        is_typing: true,
                    });

                    // Wait 500ms to simulate typing
                    sleep(Duration::from_millis(500)).await;

                    // Automatically echo reply
                    let reply = format!(
                        "Echo from Bot: '{}'! Direct P2P QUIC confirmed 🚀",
                        message.content
                    );
                    println!("📤 [REPLYING] \"{}\"", reply);

                    let _ = handle.send_command(MessengerCommand::SendTextMessage {
                        peer_key: message.conversation_peer,
                        content: reply,
                    });
                }
                MessengerEvent::FileTransferProgress {
                    file_id,
                    bytes_transferred,
                    total_bytes,
                    is_outgoing,
                    ..
                } => {
                    let dir = if is_outgoing { "Sending" } else { "Receiving" };
                    let pct = if total_bytes > 0 {
                        (bytes_transferred as f64 / total_bytes as f64) * 100.0
                    } else {
                        100.0
                    };
                    println!(
                        "⏳ [{}] File {} : {:.1}% ({}/{} bytes)",
                        dir, file_id, pct, bytes_transferred, total_bytes
                    );
                }
                MessengerEvent::FileTransferComplete {
                    peer_key,
                    file_id: _,
                    local_path,
                    is_outgoing,
                } => {
                    if !is_outgoing {
                        println!("🎉 [FILE SAVED] Stored at: {}", local_path);
                        let reply = format!(
                            "Echo Bot: Received and verified your file/folder at '{}' over Iroh QUIC! 🚀",
                            local_path
                        );
                        let _ = handle.send_command(MessengerCommand::SendTextMessage {
                            peer_key,
                            content: reply,
                        });
                    }
                }
                MessengerEvent::MessageStatusUpdated {
                    message_id: _,
                    status,
                    ..
                } => {
                    println!("📋 [DELIVERY STATUS] {:?}", status);
                }
                _ => {}
            }
        }
        sleep(Duration::from_millis(50)).await;
    }
}
