#![windows_subsystem = "windows"]

slint::include_modules!();

use message_core::{
    ConnectionType, Contact, MessageDirection, MessageStatus, MessengerCommand, MessengerEvent,
    MessengerHandle, StoredMessage,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};

#[cfg(windows)]
mod notification {
    use message_core::{MessengerCommand, MessengerHandle};
    use winrt_toast_reborn::{content::input::InputType, Action, Input, Toast, ToastManager};

    pub fn notify_with_reply(
        sender_nick: &str,
        message_text: &str,
        peer_key: &str,
        handle: MessengerHandle,
    ) {
        let peer_key_owned = peer_key.to_string();
        let manager = ToastManager::new("com.brajesh.doot").on_activated(None, move |action| {
            if let Some(act) = action {
                if let Some(reply_text) = act.values.get("replyBox") {
                    let trimmed = reply_text.trim();
                    if !trimmed.is_empty() {
                        let _ = handle.send_command(MessengerCommand::SendTextMessage {
                            peer_key: peer_key_owned.clone(),
                            content: trimmed.to_string(),
                        });
                    }
                }
            }
        });

        let mut toast = Toast::new();
        toast
            .text1(&format!("{} (Doot)", sender_nick))
            .text2(message_text)
            .input(
                Input::new("replyBox", InputType::Text)
                    .with_placeholder("Type a reply and press Send..."),
            )
            .action(Action::new("Send", peer_key, "").with_input_id("replyBox"));

        let _ = manager.show(&toast);
    }
}

#[cfg(not(windows))]
mod notification {
    use message_core::MessengerHandle;

    pub fn notify_with_reply(
        sender_nick: &str,
        message_text: &str,
        _peer_key: &str,
        _handle: MessengerHandle,
    ) {
        let _ = notify_rust::Notification::new()
            .summary(&format!("{} (Doot)", sender_nick))
            .body(message_text)
            .show();
    }
}

struct AppState {
    active_peer_key: Option<String>,
    search_query: String,
    typing_peers: HashMap<String, Instant>,
    conn_types: HashMap<String, ConnectionType>,
    contacts: Vec<Contact>,
}

fn create_tray_icon() -> tray_icon::Icon {
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - 16.0;
            let dy = y as f32 - 16.0;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= 14.0 {
                if dist <= 5.5 {
                    rgba.extend_from_slice(&[255, 255, 255, 255]); // white center
                } else {
                    rgba.extend_from_slice(&[37, 99, 235, 255]); // royal blue
                }
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]); // transparent
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, width, height).expect("Failed to create tray icon")
}

fn format_file_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let kb = bytes as f64 / 1024.0;
    let mb = kb / 1024.0;
    let gb = mb / 1024.0;
    if gb >= 1.0 {
        format!("{:.1} GB", gb)
    } else if mb >= 1.0 {
        format!("{:.1} MB", mb)
    } else if kb >= 1.0 {
        format!("{:.1} KB", kb)
    } else {
        format!("{} B", bytes)
    }
}

fn create_contacts_data(
    contacts: &[Contact],
    search_query: &str,
    typing_peers: &HashMap<String, Instant>,
    conn_types: &HashMap<String, ConnectionType>,
) -> Vec<ContactData> {
    let now = Instant::now();
    let query = search_query.to_lowercase();
    contacts
        .iter()
        .filter(|c| {
            if query.is_empty() {
                true
            } else {
                c.nickname.to_lowercase().contains(&query)
                    || c.public_key.to_lowercase().contains(&query)
            }
        })
        .map(|c| {
            let is_typing = typing_peers
                .get(&c.public_key)
                .map(|t| *t > now)
                .unwrap_or(false);
            let conn = conn_types.get(&c.public_key);
            let (is_direct, is_online, rtt_ms) = match conn {
                Some(ConnectionType::Direct { rtt_ms, .. }) => (true, true, *rtt_ms as i32),
                Some(ConnectionType::Relay { rtt_ms, .. }) => (false, true, *rtt_ms as i32),
                Some(ConnectionType::Unknown) => (false, true, 0),
                None => (false, false, 0),
            };
            let timestamp = c
                .last_seen
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_default();

            ContactData {
                peer_key: c.public_key.clone().into(),
                nickname: c.nickname.clone().into(),
                last_message: c.last_message_preview.clone().unwrap_or_default().into(),
                timestamp: timestamp.into(),
                unread_count: c.unread_count as i32,
                is_online,
                is_direct,
                rtt_ms,
                is_typing,
            }
        })
        .collect()
}

fn create_messages_data(messages: &[StoredMessage]) -> Vec<MessageData> {
    messages
        .iter()
        .map(|m| {
            let is_mine = matches!(m.direction, MessageDirection::Outgoing);
            let status = match m.status {
                MessageStatus::Sending => "Pending",
                MessageStatus::Sent => "Sent",
                MessageStatus::Delivered => "Delivered",
                MessageStatus::Failed(_) => "Failed",
            };
            let (has_attachment, file_name, file_size_text, is_dir, local_path) =
                match &m.attachment {
                    Some(att) => (
                        true,
                        att.file_name.clone(),
                        format_file_size(att.file_size),
                        att.is_directory,
                        att.local_path.clone().unwrap_or_default(),
                    ),
                    None => (false, String::new(), String::new(), false, String::new()),
                };

            MessageData {
                id: m.id.to_string().into(),
                is_mine,
                content: m.content.clone().into(),
                timestamp: m.timestamp.format("%H:%M:%S").to_string().into(),
                status: status.into(),
                has_attachment,
                file_name: file_name.into(),
                file_size_text: file_size_text.into(),
                is_directory: is_dir,
                local_path: local_path.into(),
            }
        })
        .collect()
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("message_core=debug".parse().unwrap())
                .add_directive("message_desktop=debug".parse().unwrap()),
        )
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");

    let handle = rt
        .block_on(async { MessengerHandle::start(None, None).await })
        .expect("Failed to initialize Doot Messenger engine");

    let initial_contacts = handle.get_contacts();
    let state = Arc::new(Mutex::new(AppState {
        active_peer_key: None,
        search_query: String::new(),
        typing_peers: HashMap::new(),
        conn_types: HashMap::new(),
        contacts: initial_contacts.clone(),
    }));

    // Initialize Slint App Window
    let app = AppWindow::new().expect("Failed to initialize Slint AppWindow");

    app.set_my_nickname(handle.my_nickname.clone().into());
    app.set_my_node_id(handle.my_node_id.clone().into());
    app.set_my_ticket(handle.my_ticket.clone().into());

    let initial_c_data =
        create_contacts_data(&initial_contacts, "", &HashMap::new(), &HashMap::new());
    app.set_contacts(ModelRc::new(VecModel::from(initial_c_data)));
    app.set_messages(ModelRc::new(VecModel::<MessageData>::default()));

    // System Tray Setup
    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Open Doot", true, None);
    let copy_ticket_item = MenuItem::new("Copy Sovereign Ticket", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Quit Doot", true, None);

    let _ = tray_menu.append(&show_item);
    let _ = tray_menu.append(&copy_ticket_item);
    let _ = tray_menu.append(&separator);
    let _ = tray_menu.append(&quit_item);

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Doot - Sovereign P2P Messenger")
        .with_icon(create_tray_icon())
        .build()
        .expect("Failed to create system tray icon");

    // Intercept Window Close to run in background in System Tray
    app.window().on_close_requested(|| {
        tracing::info!("Window close requested; hiding to system tray");
        slint::CloseRequestResponse::HideWindow
    });

    // Listen to Tray Menu Events
    let app_weak_tray = app.as_weak();
    let handle_tray = handle.clone();
    let show_id = show_item.id().clone();
    let copy_id = copy_ticket_item.id().clone();
    let quit_id = quit_item.id().clone();

    std::thread::spawn(move || {
        while let Ok(event) = MenuEvent::receiver().recv() {
            if event.id == show_id {
                let app_weak = app_weak_tray.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        let window = app.window();
                        let _ = window.show();
                    }
                });
            } else if event.id == copy_id {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&handle_tray.my_ticket);
                }
            } else if event.id == quit_id {
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
                break;
            }
        }
    });

    // Listen to Tray Click Events
    let app_weak_click = app.as_weak();
    std::thread::spawn(move || {
        while let Ok(event) = TrayIconEvent::receiver().recv() {
            match event {
                TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. } => {
                    let app_weak = app_weak_click.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            let window = app.window();
                            let _ = window.show();
                        }
                    });
                }
                _ => {}
            }
        }
    });

    // Callbacks implementation

    // 1. Select Contact
    let handle_select = handle.clone();
    let state_select = state.clone();
    let app_weak_select = app.as_weak();
    app.on_select_contact(move |idx| {
        let (peer_key, nickname, conn_info_str, is_direct) = {
            let mut st = state_select.lock().unwrap();
            let query = st.search_query.to_lowercase();
            let filtered: Vec<Contact> = st
                .contacts
                .iter()
                .filter(|c| {
                    if query.is_empty() {
                        true
                    } else {
                        c.nickname.to_lowercase().contains(&query)
                            || c.public_key.to_lowercase().contains(&query)
                    }
                })
                .cloned()
                .collect();

            if let Some(c) = filtered.get(idx as usize) {
                st.active_peer_key = Some(c.public_key.clone());
                let conn = st.conn_types.get(&c.public_key);
                let is_direct = matches!(conn, Some(ConnectionType::Direct { .. }));
                let conn_info_str = match conn {
                    Some(ConnectionType::Direct { addr, rtt_ms }) => {
                        format!("Direct UDP ({} • {}ms)", addr, rtt_ms)
                    }
                    Some(ConnectionType::Relay { url, rtt_ms }) => {
                        format!("Relayed ({} • {}ms)", url, rtt_ms)
                    }
                    _ => "Encrypted QUIC".to_string(),
                };
                (
                    c.public_key.clone(),
                    c.nickname.clone(),
                    conn_info_str,
                    is_direct,
                )
            } else {
                return;
            }
        };

        let _ = handle_select.mark_as_read(&peer_key);
        let msgs = handle_select.get_messages(&peer_key);
        let updated_contacts = handle_select.get_contacts();

        let (c_data, m_data) = {
            let mut st = state_select.lock().unwrap();
            st.contacts = updated_contacts;
            let c = create_contacts_data(
                &st.contacts,
                &st.search_query,
                &st.typing_peers,
                &st.conn_types,
            );
            let m = create_messages_data(&msgs);
            (c, m)
        };

        if let Some(app) = app_weak_select.upgrade() {
            app.set_messages(ModelRc::new(VecModel::from(m_data)));
            app.set_contacts(ModelRc::new(VecModel::from(c_data)));
            app.set_selected_contact_index(idx);
            app.set_active_peer_key(peer_key.into());
            app.set_active_peer_nick(nickname.into());
            app.set_active_conn_info(conn_info_str.into());
            app.set_active_is_direct(is_direct);
        }
    });

    // 2. Send Message
    let handle_send = handle.clone();
    let state_send = state.clone();
    let app_weak_send = app.as_weak();
    app.on_send_message(move |text| {
        let text_clean = text.trim().to_string();
        if text_clean.is_empty() {
            return;
        }

        let peer_key_opt = {
            let st = state_send.lock().unwrap();
            st.active_peer_key.clone()
        };

        if let Some(peer_key) = peer_key_opt {
            let _ = handle_send.send_command(MessengerCommand::SendTextMessage {
                peer_key: peer_key.clone(),
                content: text_clean,
            });
            let msgs = handle_send.get_messages(&peer_key);
            let contacts = handle_send.get_contacts();

            let (c_data, m_data) = {
                let mut st = state_send.lock().unwrap();
                st.contacts = contacts;
                let c = create_contacts_data(
                    &st.contacts,
                    &st.search_query,
                    &st.typing_peers,
                    &st.conn_types,
                );
                let m = create_messages_data(&msgs);
                (c, m)
            };

            if let Some(app) = app_weak_send.upgrade() {
                app.set_messages(ModelRc::new(VecModel::from(m_data)));
                app.set_contacts(ModelRc::new(VecModel::from(c_data)));
            }
        }
    });

    // 3. Send Typing
    let handle_typing = handle.clone();
    let state_typing = state.clone();
    app.on_send_typing(move |is_typing| {
        let peer_key_opt = {
            let st = state_typing.lock().unwrap();
            st.active_peer_key.clone()
        };
        if let Some(peer_key) = peer_key_opt {
            let _ = handle_typing.send_command(MessengerCommand::SendTyping {
                peer_key,
                is_typing,
            });
        }
    });

    // 4. Attach File
    let handle_attach = handle.clone();
    let state_attach = state.clone();
    let app_weak_attach = app.as_weak();
    app.on_attach_file(move || {
        let peer_key_opt = {
            let st = state_attach.lock().unwrap();
            st.active_peer_key.clone()
        };

        if let Some(peer_key) = peer_key_opt {
            if let Some(file_path) = rfd::FileDialog::new().pick_file() {
                let _ = handle_attach.send_file(&peer_key, PathBuf::from(&file_path), false);
                if let Some(app) = app_weak_attach.upgrade() {
                    app.set_toast_banner("Sending encrypted file over QUIC...".into());
                    app.set_show_toast(true);
                }
            }
        }
    });

    // 5. Ping Active Peer
    let handle_ping = handle.clone();
    let state_ping = state.clone();
    let app_weak_ping = app.as_weak();
    app.on_ping_active_peer(move || {
        let peer_key_opt = {
            let st = state_ping.lock().unwrap();
            st.active_peer_key.clone()
        };
        if let Some(peer_key) = peer_key_opt {
            let _ = handle_ping.send_command(MessengerCommand::PingPeer { peer_key });
            if let Some(app) = app_weak_ping.upgrade() {
                app.set_toast_banner("Measuring live QUIC round-trip latency...".into());
                app.set_show_toast(true);
            }
        }
    });

    // 6. Copy Active Peer Key
    let state_copy_peer = state.clone();
    let app_weak_copy_peer = app.as_weak();
    app.on_copy_active_peer_key(move || {
        let peer_key_opt = {
            let st = state_copy_peer.lock().unwrap();
            st.active_peer_key.clone()
        };
        if let Some(peer_key) = peer_key_opt {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(&peer_key);
            }
            if let Some(app) = app_weak_copy_peer.upgrade() {
                app.set_toast_banner("Peer cryptographic key copied!".into());
                app.set_show_toast(true);
            }
        }
    });

    // 7. Open Attachment
    app.on_open_attachment(move |path| {
        let path_str = path.to_string();
        if !path_str.is_empty() {
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("explorer")
                    .arg(&path_str)
                    .spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open").arg(&path_str).spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&path_str)
                    .spawn();
            }
        }
    });

    // 8. Connect Peer
    let handle_conn = handle.clone();
    let state_conn = state.clone();
    let app_weak_conn = app.as_weak();
    app.on_connect_peer(move |ticket, nickname| {
        let ticket_clean = ticket.trim().to_string();
        let nick_opt = if nickname.trim().is_empty() {
            None
        } else {
            Some(nickname.trim().to_string())
        };

        let res = handle_conn.send_command(MessengerCommand::ConnectPeer {
            ticket_or_id: ticket_clean,
            nickname: nick_opt,
        });

        match res {
            Ok(_) => {
                let contacts = handle_conn.get_contacts();
                let c_data = {
                    let mut st = state_conn.lock().unwrap();
                    st.contacts = contacts;
                    create_contacts_data(
                        &st.contacts,
                        &st.search_query,
                        &st.typing_peers,
                        &st.conn_types,
                    )
                };

                if let Some(app) = app_weak_conn.upgrade() {
                    app.set_contacts(ModelRc::new(VecModel::from(c_data)));
                    app.set_toast_banner("Connecting to sovereign peer over QUIC...".into());
                    app.set_show_toast(true);
                }
            }
            Err(e) => {
                if let Some(app) = app_weak_conn.upgrade() {
                    app.set_toast_banner(format!("Connection error: {}", e).into());
                    app.set_show_toast(true);
                }
            }
        }
    });

    // 9. Update Nickname
    let handle_nick = handle.clone();
    let app_weak_nick = app.as_weak();
    app.on_update_nickname(move |nick| {
        let clean = nick.trim().to_string();
        if !clean.is_empty() {
            let _ = handle_nick.send_command(MessengerCommand::UpdateNickname {
                nickname: clean.clone(),
            });
            if let Some(app) = app_weak_nick.upgrade() {
                app.set_my_nickname(clean.into());
                app.set_toast_banner("Public nickname updated!".into());
                app.set_show_toast(true);
            }
        }
    });

    // 10. Copy My Ticket
    let handle_ticket = handle.clone();
    let app_weak_ticket = app.as_weak();
    app.on_copy_my_ticket(move || {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(&handle_ticket.my_ticket);
        }
        if let Some(app) = app_weak_ticket.upgrade() {
            app.set_toast_banner("Your shareable sovereign ticket copied!".into());
            app.set_show_toast(true);
        }
    });

    // 11. Copy My Node ID
    let handle_id = handle.clone();
    let app_weak_id = app.as_weak();
    app.on_copy_my_node_id(move || {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(&handle_id.my_node_id);
        }
        if let Some(app) = app_weak_id.upgrade() {
            app.set_toast_banner("Node ID copied to clipboard!".into());
            app.set_show_toast(true);
        }
    });

    // 12. Close Toast
    let app_weak_toast = app.as_weak();
    app.on_close_toast(move || {
        if let Some(app) = app_weak_toast.upgrade() {
            app.set_show_toast(false);
        }
    });

    // Background Event Processing Task
    let app_weak_events = app.as_weak();
    let handle_events = handle.clone();
    let state_events = state.clone();

    std::thread::spawn(move || {
        loop {
            let mut updated_messages = false;
            let mut updated_contacts = false;
            let mut updated_typing = false;
            let mut incoming_notification: Option<(String, String, String)> = None;

            while let Some(event) = handle_events.try_recv_event() {
                match event {
                    MessengerEvent::MessageReceived { message } => {
                        let is_mine = matches!(message.direction, MessageDirection::Outgoing);
                        let is_active = {
                            let st = state_events.lock().unwrap();
                            st.active_peer_key.as_deref() == Some(&message.conversation_peer)
                        };

                        if is_active {
                            updated_messages = true;
                        }

                        if !is_mine {
                            let nick = {
                                let st = state_events.lock().unwrap();
                                st.contacts
                                    .iter()
                                    .find(|c| c.public_key == message.conversation_peer)
                                    .map(|c| c.nickname.clone())
                                    .unwrap_or_else(|| message.sender_name.clone())
                            };
                            let preview = message
                                .attachment
                                .as_ref()
                                .map(|a| format!("Sent file: {}", a.file_name))
                                .unwrap_or_else(|| message.content.clone());
                            incoming_notification =
                                Some((nick, preview, message.conversation_peer.clone()));
                        }

                        updated_contacts = true;
                    }
                    MessengerEvent::MessageSent { message } => {
                        let is_active = {
                            let st = state_events.lock().unwrap();
                            st.active_peer_key.as_deref() == Some(&message.conversation_peer)
                        };
                        if is_active {
                            updated_messages = true;
                        }
                        updated_contacts = true;
                    }
                    MessengerEvent::MessageStatusUpdated { .. } => {
                        updated_messages = true;
                        updated_contacts = true;
                    }
                    MessengerEvent::PeerTyping {
                        peer_key,
                        is_typing,
                    } => {
                        let mut st = state_events.lock().unwrap();
                        if is_typing {
                            st.typing_peers
                                .insert(peer_key, Instant::now() + Duration::from_secs(3));
                        } else {
                            st.typing_peers.remove(&peer_key);
                        }
                        updated_typing = true;
                        updated_contacts = true;
                    }
                    MessengerEvent::PeerConnected {
                        peer_key,
                        connection_type,
                        ..
                    } => {
                        let mut st = state_events.lock().unwrap();
                        st.conn_types.insert(peer_key, connection_type);
                        updated_contacts = true;
                    }
                    MessengerEvent::ConnectionInfoUpdated {
                        peer_key,
                        connection_type,
                    } => {
                        let mut st = state_events.lock().unwrap();
                        st.conn_types.insert(peer_key, connection_type);
                        updated_contacts = true;
                    }
                    MessengerEvent::PeerDisconnected { peer_key } => {
                        let mut st = state_events.lock().unwrap();
                        st.conn_types.remove(&peer_key);
                        updated_contacts = true;
                    }
                    MessengerEvent::FileTransferProgress { .. }
                    | MessengerEvent::FileTransferComplete { .. } => {
                        updated_messages = true;
                    }
                    _ => {}
                }
            }

            // Fire native notification with reply input if incoming message arrived
            if let Some((sender_nick, preview, peer_key)) = incoming_notification {
                notification::notify_with_reply(
                    &sender_nick,
                    &preview,
                    &peer_key,
                    handle_events.clone(),
                );
            }

            if updated_messages || updated_contacts || updated_typing {
                let handle_c = handle_events.clone();
                let state_c = state_events.clone();
                let app_weak = app_weak_events.clone();

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        let active_peer_opt = {
                            let st = state_c.lock().unwrap();
                            st.active_peer_key.clone()
                        };

                        if updated_messages {
                            if let Some(ref peer_key) = active_peer_opt {
                                let msgs = handle_c.get_messages(peer_key);
                                let m_data = create_messages_data(&msgs);
                                app.set_messages(ModelRc::new(VecModel::from(m_data)));
                            }
                        }

                        if updated_contacts {
                            let contacts = handle_c.get_contacts();
                            let mut st = state_c.lock().unwrap();
                            st.contacts = contacts;
                            let c_data = create_contacts_data(
                                &st.contacts,
                                &st.search_query,
                                &st.typing_peers,
                                &st.conn_types,
                            );
                            app.set_contacts(ModelRc::new(VecModel::from(c_data)));
                        }

                        if updated_typing {
                            let is_typing = {
                                let st = state_c.lock().unwrap();
                                active_peer_opt
                                    .as_ref()
                                    .and_then(|k| st.typing_peers.get(k))
                                    .map(|t| *t > Instant::now())
                                    .unwrap_or(false)
                            };
                            app.set_is_peer_typing(is_typing);
                        }
                    }
                });
            }

            std::thread::sleep(Duration::from_millis(60));
        }
    });

    tracing::info!("Running Slint desktop application event loop...");
    app.run().expect("Failed to run Slint event loop");
}
