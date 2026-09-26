#![windows_subsystem = "windows"]

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};
use message_core::{
    Contact, MessageDirection, MessageStatus, MessengerCommand, MessengerEvent, MessengerHandle,
    StoredMessage,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
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
        .expect("Failed to initialize Iroh Messenger engine");

    tracing::info!("Engine initialized. Configuring NativeOptions...");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 700.0])
            .with_min_inner_size([680.0, 480.0])
            .with_title("Iroh P2P Messenger")
            .with_visible(true),
        ..Default::default()
    };

    tracing::info!("Calling eframe::run_native now...");

    eframe::run_native(
        "Iroh P2P Messenger",
        native_options,
        Box::new(|cc| {
            tracing::info!("app_creator closure called!");
            let app = MessengerApp::new(cc, handle, rt);
            tracing::info!("MessengerApp constructed successfully!");
            Ok(Box::new(app))
        }),
    )
}

struct MessengerApp {
    handle: MessengerHandle,
    _rt: tokio::runtime::Runtime,

    // State
    contacts: Vec<Contact>,
    active_peer: Option<String>,
    messages: Vec<StoredMessage>,
    input_text: String,
    search_query: String,

    // Real-time tracking
    typing_peers: HashMap<String, Instant>,
    last_typed_sent: Option<Instant>,
    banner: Option<(String, Instant, bool)>, // (message, expire_at, is_error)
    copied_notification: Option<(String, Instant)>,
    scroll_to_bottom: bool,

    // Dialogs
    show_add_peer_modal: bool,
    new_peer_ticket: String,
    new_peer_nickname: String,

    show_settings_modal: bool,
    edit_nickname: String,
}

impl MessengerApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        handle: MessengerHandle,
        rt: tokio::runtime::Runtime,
    ) -> Self {
        // Configure pleasant dark theme styling
        let mut visuals = egui::Visuals::dark();
        visuals.window_corner_radius = CornerRadius::same(10);
        visuals.menu_corner_radius = CornerRadius::same(8);
        visuals.panel_fill = Color32::from_rgb(18, 20, 26);
        visuals.window_fill = Color32::from_rgb(24, 27, 36);
        cc.egui_ctx.set_visuals(visuals);

        let initial_contacts = handle.get_contacts();
        let edit_nickname = handle.my_nickname.clone();

        tracing::info!("Initializing MessengerApp state...");

        Self {
            handle,
            _rt: rt,
            contacts: initial_contacts,
            active_peer: None,
            messages: Vec::new(),
            input_text: String::new(),
            search_query: String::new(),
            typing_peers: HashMap::new(),
            last_typed_sent: None,
            banner: None,
            copied_notification: None,
            scroll_to_bottom: false,
            show_add_peer_modal: false,
            new_peer_ticket: String::new(),
            new_peer_nickname: String::new(),
            show_settings_modal: false,
            edit_nickname,
        }
    }

    fn select_peer(&mut self, peer_key: String) {
        self.active_peer = Some(peer_key.clone());
        self.messages = self.handle.get_messages(&peer_key);
        let _ = self.handle.mark_as_read(&peer_key);
        self.contacts = self.handle.get_contacts();
        self.scroll_to_bottom = true;
    }

    fn copy_to_clipboard(&mut self, ctx: &egui::Context, text: String, label: &str) {
        ctx.copy_text(text);
        self.copied_notification = Some((
            format!("{} copied to clipboard!", label),
            Instant::now() + Duration::from_secs(3),
        ));
    }

    fn set_banner(&mut self, msg: String, is_error: bool) {
        self.banner = Some((msg, Instant::now() + Duration::from_secs(4), is_error));
    }

    fn process_events(&mut self, ctx: &egui::Context) {
        let mut need_repaint = false;

        while let Some(event) = self.handle.try_recv_event() {
            need_repaint = true;
            match event {
                MessengerEvent::EngineReady {
                    my_nickname,
                    contacts,
                    ..
                } => {
                    self.edit_nickname = my_nickname;
                    self.contacts = contacts;
                }
                MessengerEvent::PeerConnected { peer_key, .. } => {
                    self.contacts = self.handle.get_contacts();
                    if self.active_peer.as_deref() == Some(&peer_key) {
                        self.set_banner("Connected to peer!".to_string(), false);
                    }
                }
                MessengerEvent::PeerDisconnected { peer_key } => {
                    self.typing_peers.remove(&peer_key);
                    self.contacts = self.handle.get_contacts();
                }
                MessengerEvent::PeerTyping {
                    peer_key,
                    is_typing,
                } => {
                    if is_typing {
                        self.typing_peers
                            .insert(peer_key, Instant::now() + Duration::from_secs(4));
                    } else {
                        self.typing_peers.remove(&peer_key);
                    }
                }
                MessengerEvent::MessageReceived { message } => {
                    if self.active_peer.as_deref() == Some(&message.conversation_peer) {
                        self.messages.push(message.clone());
                        let _ = self.handle.mark_as_read(&message.conversation_peer);
                        self.scroll_to_bottom = true;
                    }
                    self.contacts = self.handle.get_contacts();
                }
                MessengerEvent::MessageSent { message } => {
                    if self.active_peer.as_deref() == Some(&message.conversation_peer) {
                        self.messages.push(message);
                        self.scroll_to_bottom = true;
                    }
                    self.contacts = self.handle.get_contacts();
                }
                MessengerEvent::MessageStatusUpdated {
                    peer_key,
                    message_id,
                    status,
                } => {
                    if self.active_peer.as_deref() == Some(&peer_key) {
                        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
                            msg.status = status;
                        }
                    }
                }
                MessengerEvent::ContactListUpdated { contacts } => {
                    self.contacts = contacts;
                }
                MessengerEvent::Error { context, error } => {
                    self.set_banner(format!("{}: {}", context, error), true);
                }
                _ => {}
            }
        }

        // Clean expired typing indicators
        self.typing_peers
            .retain(|_, expire| *expire > Instant::now());

        if need_repaint {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for MessengerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        static FIRST_FRAME: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(true);
        if FIRST_FRAME.swap(false, std::sync::atomic::Ordering::Relaxed) {
            tracing::info!("First frame update called! Window is rendering.");
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        self.process_events(ctx);

        // Top Navigation Ribbon
        egui::TopBottomPanel::top("top_panel")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(22, 25, 33))
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Logo & App Name
                    ui.label(
                        RichText::new("⚡ Iroh P2P Messenger")
                            .size(17.0)
                            .strong()
                            .color(Color32::from_rgb(140, 180, 255)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Settings button
                        if ui.button(RichText::new("⚙ Settings").size(13.0)).clicked() {
                            self.show_settings_modal = true;
                        }

                        // My Shareable Ticket Button
                        if ui
                            .button(
                                RichText::new("📋 Copy My Ticket")
                                    .size(13.0)
                                    .color(Color32::from_rgb(220, 235, 255)),
                            )
                            .on_hover_text("Copy your full shareable ticket to share with friends")
                            .clicked()
                        {
                            self.copy_to_clipboard(
                                ctx,
                                self.handle.my_ticket.clone(),
                                "Shareable Ticket",
                            );
                        }

                        // Online identity pill
                        let short_id = if self.handle.my_node_id.len() > 8 {
                            &self.handle.my_node_id[..8]
                        } else {
                            &self.handle.my_node_id
                        };

                        ui.label(
                            RichText::new(format!("● {} ({})", self.handle.my_nickname, short_id))
                                .size(12.5)
                                .color(Color32::from_rgb(100, 220, 150)),
                        );
                    });
                });
            });

        // Banner notifications (e.g. copied, error, info)
        if let Some((msg, expire, is_error)) = &self.banner {
            if *expire > Instant::now() {
                egui::TopBottomPanel::top("banner_panel")
                    .frame(
                        egui::Frame::new()
                            .fill(if *is_error {
                                Color32::from_rgb(110, 30, 35)
                            } else {
                                Color32::from_rgb(30, 90, 60)
                            })
                            .inner_margin(egui::Margin::symmetric(16, 6)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(msg).color(Color32::WHITE).size(13.0));
                        });
                    });
            } else {
                self.banner = None;
            }
        }

        // Left Sidebar: Contacts and Add Peer
        egui::SidePanel::left("left_sidebar")
            .resizable(true)
            .default_width(300.0)
            .width_range(240.0..=420.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 23, 31))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                // Header section
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Chats").size(18.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("+ New Chat")
                                    .size(13.0)
                                    .color(Color32::from_rgb(130, 190, 255)),
                            )
                            .clicked()
                        {
                            self.show_add_peer_modal = true;
                        }
                    });
                });

                ui.add_space(8.0);

                // Search Bar
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    ui.add_sized(
                        [ui.available_width(), 24.0],
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Search contacts..."),
                    );
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Contacts List
                let filtered_contacts: Vec<Contact> = self
                    .contacts
                    .iter()
                    .filter(|c| {
                        if self.search_query.is_empty() {
                            true
                        } else {
                            let q = self.search_query.to_lowercase();
                            c.nickname.to_lowercase().contains(&q)
                                || c.public_key.to_lowercase().contains(&q)
                        }
                    })
                    .cloned()
                    .collect();

                if filtered_contacts.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.label(
                            RichText::new("No conversations yet")
                                .color(Color32::from_rgb(120, 130, 150))
                                .size(13.5),
                        );
                        ui.add_space(10.0);
                        if ui.button("Connect with a Friend").clicked() {
                            self.show_add_peer_modal = true;
                        }
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for contact in filtered_contacts {
                                let is_selected =
                                    self.active_peer.as_deref() == Some(&contact.public_key);
                                let is_connected =
                                    self.handle.is_peer_connected(&contact.public_key);

                                let bg_color = if is_selected {
                                    Color32::from_rgb(38, 48, 68)
                                } else {
                                    Color32::TRANSPARENT
                                };

                                let frame = egui::Frame::new()
                                    .fill(bg_color)
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::symmetric(10, 8));

                                let response = frame.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Avatar Circle with initial
                                        let initial = contact
                                            .nickname
                                            .chars()
                                            .next()
                                            .unwrap_or('?')
                                            .to_uppercase()
                                            .to_string();

                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(36.0, 36.0),
                                            egui::Sense::hover(),
                                        );

                                        let circle_color = if is_connected {
                                            Color32::from_rgb(45, 110, 85)
                                        } else {
                                            Color32::from_rgb(60, 65, 80)
                                        };

                                        ui.painter().circle_filled(
                                            rect.center(),
                                            18.0,
                                            circle_color,
                                        );

                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            initial,
                                            egui::FontId::proportional(16.0),
                                            Color32::WHITE,
                                        );

                                        // Online status badge dot
                                        let status_dot_pos = rect.center() + Vec2::new(11.0, 11.0);
                                        let dot_color = if is_connected {
                                            Color32::from_rgb(50, 220, 120)
                                        } else {
                                            Color32::from_rgb(110, 115, 130)
                                        };
                                        ui.painter().circle_filled(status_dot_pos, 4.5, dot_color);
                                        ui.painter().circle_stroke(
                                            status_dot_pos,
                                            4.5,
                                            Stroke::new(1.5_f32, Color32::from_rgb(20, 23, 31)),
                                        );

                                        // Name and preview
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(&contact.nickname)
                                                        .size(14.0)
                                                        .strong(),
                                                );

                                                if contact.unread_count > 0 {
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            let badge = RichText::new(format!(
                                                                " {} ",
                                                                contact.unread_count
                                                            ))
                                                            .size(11.0)
                                                            .color(Color32::WHITE)
                                                            .background_color(Color32::from_rgb(
                                                                60, 120, 240,
                                                            ));
                                                            ui.label(badge);
                                                        },
                                                    );
                                                }
                                            });

                                            let preview = contact
                                                .last_message_preview
                                                .as_deref()
                                                .unwrap_or("No messages yet");
                                            ui.label(
                                                RichText::new(preview)
                                                    .size(12.0)
                                                    .color(Color32::from_rgb(140, 150, 170)),
                                            );
                                        });
                                    });
                                });

                                if response.response.interact(egui::Sense::click()).clicked() {
                                    self.select_peer(contact.public_key.clone());
                                }

                                ui.add_space(2.0);
                            }
                        });
                }
            });

        // Center Panel: Chat conversation or Empty Welcome State
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(15, 17, 23))
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ctx, |ui| {
                if let Some(peer_key) = self.active_peer.clone() {
                    self.render_chat_view(ui, ctx, &peer_key);
                } else {
                    self.render_empty_welcome(ui, ctx);
                }
            });

        // Dialogs
        self.render_modals(ctx);
    }
}

impl MessengerApp {
    fn render_empty_welcome(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                RichText::new("🚀 Direct P2P Encrypted Messenger")
                    .size(24.0)
                    .strong()
                    .color(Color32::from_rgb(180, 210, 255)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Powered by Iroh QUIC networking. No central chat servers, no intermediaries.",
                )
                .size(14.0)
                .color(Color32::from_rgb(140, 150, 170)),
            );

            ui.add_space(32.0);

            // Ticket Card
            let card_frame = egui::Frame::new()
                .fill(Color32::from_rgb(24, 28, 38))
                .corner_radius(CornerRadius::same(12))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 70)))
                .inner_margin(egui::Margin::same(20));

            card_frame.show(ui, |ui| {
                ui.set_max_width(540.0);
                ui.label(
                    RichText::new("Your Shareable P2P Ticket")
                        .size(15.0)
                        .strong()
                        .color(Color32::from_rgb(130, 200, 255)),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Share this ticket with anyone. They can paste it to establish an instant direct QUIC connection with you through relays and NAT hole-punching:")
                        .size(12.5)
                        .color(Color32::from_rgb(160, 170, 190)),
                );
                ui.add_space(10.0);

                let ticket_box = egui::Frame::new()
                    .fill(Color32::from_rgb(16, 18, 25))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(10));

                ticket_box.show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&self.handle.my_ticket)
                                .monospace()
                                .size(11.5)
                                .color(Color32::from_rgb(130, 230, 170)),
                        )
                        .wrap(),
                    );
                });

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("📋 Copy Ticket to Clipboard")
                                .size(13.5)
                                .color(Color32::WHITE),
                        )
                        .clicked()
                    {
                        self.copy_to_clipboard(
                            ctx,
                            self.handle.my_ticket.clone(),
                            "Shareable Ticket",
                        );
                    }

                    if ui
                        .button(
                            RichText::new("+ Connect to a Friend")
                                .size(13.5)
                                .color(Color32::from_rgb(140, 190, 255)),
                        )
                        .clicked()
                    {
                        self.show_add_peer_modal = true;
                    }
                });
            });
        });
    }

    fn render_chat_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, peer_key: &str) {
        let contact = self
            .handle
            .get_contacts()
            .into_iter()
            .find(|c| c.public_key == peer_key);
        let contact_name = contact
            .as_ref()
            .map(|c| c.nickname.clone())
            .unwrap_or_else(|| {
                let short = if peer_key.len() > 8 {
                    &peer_key[..8]
                } else {
                    peer_key
                };
                format!("Peer-{}", short)
            });

        let is_connected = self.handle.is_peer_connected(peer_key);

        // Chat Header
        ui.horizontal(|ui| {
            // Peer avatar
            let initial = contact_name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();

            let (rect, _) = ui.allocate_exact_size(Vec2::new(32.0, 32.0), egui::Sense::hover());
            ui.painter().circle_filled(
                rect.center(),
                16.0,
                if is_connected {
                    Color32::from_rgb(40, 120, 90)
                } else {
                    Color32::from_rgb(50, 55, 70)
                },
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                initial,
                egui::FontId::proportional(15.0),
                Color32::WHITE,
            );

            ui.add_space(4.0);

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&contact_name).size(15.0).strong());

                    // Connection status badge
                    if is_connected {
                        ui.label(
                            RichText::new("● Connected (QUIC)")
                                .size(11.5)
                                .color(Color32::from_rgb(80, 225, 140)),
                        );
                    } else {
                        ui.label(
                            RichText::new("○ Offline")
                                .size(11.5)
                                .color(Color32::from_rgb(140, 145, 160)),
                        );
                    }
                });

                let short_key = if peer_key.len() > 16 {
                    format!("{}...{}", &peer_key[..8], &peer_key[peer_key.len() - 6..])
                } else {
                    peer_key.to_string()
                };

                ui.label(
                    RichText::new(short_key)
                        .monospace()
                        .size(11.0)
                        .color(Color32::from_rgb(120, 130, 150)),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !is_connected
                    && ui
                        .button(RichText::new("🔄 Reconnect").size(12.5))
                        .clicked()
                {
                    let ticket_to_use = contact
                        .as_ref()
                        .and_then(|c| c.ticket.clone())
                        .unwrap_or_else(|| peer_key.to_string());
                    let _ = self.handle.send_command(MessengerCommand::ConnectPeer {
                        ticket_or_id: ticket_to_use,
                        nickname: Some(contact_name.clone()),
                    });
                    self.set_banner(format!("Reconnecting to {}...", contact_name), false);
                }

                if ui.button(RichText::new("📋 Copy Key").size(12.5)).clicked() {
                    self.copy_to_clipboard(ctx, peer_key.to_string(), "Peer Public Key");
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Messages scroll area
        let available_height = ui.available_height() - 60.0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(available_height)
            .show(ui, |ui| {
                if self.messages.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("No messages in this chat yet.")
                                .color(Color32::from_rgb(120, 130, 150))
                                .size(13.5),
                        );
                        ui.label(
                            RichText::new(
                                "Say hello! Messages are delivered directly over P2P QUIC.",
                            )
                            .color(Color32::from_rgb(90, 100, 120))
                            .size(12.0),
                        );
                    });
                } else {
                    for msg in &self.messages {
                        self.render_message_bubble(ui, msg);
                        ui.add_space(6.0);
                    }
                }

                // Peer typing indicator
                if let Some(expire) = self.typing_peers.get(peer_key) {
                    if *expire > Instant::now() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("✏ {} is typing...", contact_name))
                                    .size(12.0)
                                    .color(Color32::from_rgb(130, 180, 255)),
                            );
                        });
                    }
                }

                if self.scroll_to_bottom {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    self.scroll_to_bottom = false;
                }
            });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // Message Input Field and Send Button
        ui.horizontal(|ui| {
            let text_edit = egui::TextEdit::singleline(&mut self.input_text)
                .hint_text("Type a message... (Press Enter to send)")
                .desired_width(ui.available_width() - 85.0);

            let response = ui.add(text_edit);

            // Detect typing to emit typing indicator to peer
            if response.changed() && is_connected {
                let should_send_typing = match self.last_typed_sent {
                    Some(last) => last.elapsed() > Duration::from_secs(2),
                    None => true,
                };
                if should_send_typing {
                    self.last_typed_sent = Some(Instant::now());
                    let _ = self.handle.send_command(MessengerCommand::SendTyping {
                        peer_key: peer_key.to_string(),
                        is_typing: true,
                    });
                }
            }

            let mut send_msg = false;
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                send_msg = true;
                response.request_focus();
            }

            let send_btn = ui.add_sized(
                [75.0, 28.0],
                egui::Button::new(
                    RichText::new("Send ⮞")
                        .size(13.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(45, 95, 200)),
            );

            if send_btn.clicked() {
                send_msg = true;
            }

            if send_msg {
                let content = self.input_text.trim().to_string();
                if !content.is_empty() {
                    let _ = self.handle.send_command(MessengerCommand::SendTextMessage {
                        peer_key: peer_key.to_string(),
                        content,
                    });
                    self.input_text.clear();
                    self.scroll_to_bottom = true;
                }
            }
        });
    }

    fn render_message_bubble(&self, ui: &mut egui::Ui, msg: &StoredMessage) {
        let is_outgoing = msg.direction == MessageDirection::Outgoing;

        let bubble_bg = if is_outgoing {
            Color32::from_rgb(35, 75, 160)
        } else {
            Color32::from_rgb(32, 36, 48)
        };

        let time_str = msg.timestamp.format("%H:%M").to_string();

        let status_str = match &msg.status {
            MessageStatus::Sending => " 🕒",
            MessageStatus::Sent => " ✓",
            MessageStatus::Delivered => " ✓✓",
            MessageStatus::Failed(_) => " ⚠",
        };

        let status_color = match &msg.status {
            MessageStatus::Delivered => Color32::from_rgb(100, 240, 160),
            MessageStatus::Failed(_) => Color32::from_rgb(255, 100, 100),
            _ => Color32::from_rgb(180, 190, 210),
        };

        let bubble_frame = egui::Frame::new()
            .fill(bubble_bg)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 8));

        if is_outgoing {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                bubble_frame.show(ui, |ui| {
                    ui.set_max_width(ui.available_width() * 0.75);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&msg.content).size(13.5).color(Color32::WHITE));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(status_str).size(11.0).color(status_color));
                            ui.label(
                                RichText::new(&time_str)
                                    .size(10.5)
                                    .color(Color32::from_rgb(170, 190, 230)),
                            );
                        });
                    });
                });
            });
        } else {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                bubble_frame.show(ui, |ui| {
                    ui.set_max_width(ui.available_width() * 0.75);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&msg.sender_name)
                                .size(11.5)
                                .strong()
                                .color(Color32::from_rgb(130, 190, 255)),
                        );
                        ui.add_space(2.0);
                        ui.label(RichText::new(&msg.content).size(13.5).color(Color32::WHITE));
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(&time_str)
                                .size(10.5)
                                .color(Color32::from_rgb(140, 150, 170)),
                        );
                    });
                });
            });
        }
    }

    fn render_modals(&mut self, ctx: &egui::Context) {
        // Modal: Add Contact / Connect
        if self.show_add_peer_modal {
            egui::Window::new("Connect with Peer")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([460.0, 240.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("Enter Peer's Shareable Ticket or Public Key:")
                            .size(13.0)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.add_sized(
                        [ui.available_width(), 60.0],
                        egui::TextEdit::multiline(&mut self.new_peer_ticket)
                            .hint_text("Paste 'iroh-msg:...' ticket or hex public key"),
                    );

                    ui.add_space(8.0);
                    ui.label(RichText::new("Friendly Contact Name (Optional):").size(13.0));
                    ui.add_space(4.0);
                    ui.add_sized(
                        [ui.available_width(), 26.0],
                        egui::TextEdit::singleline(&mut self.new_peer_nickname)
                            .hint_text("e.g. Alice"),
                    );

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("Connect & Chat")
                                    .size(13.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .clicked()
                        {
                            let ticket = self.new_peer_ticket.trim().to_string();
                            if !ticket.is_empty() {
                                let nick = if self.new_peer_nickname.trim().is_empty() {
                                    None
                                } else {
                                    Some(self.new_peer_nickname.trim().to_string())
                                };

                                let _ = self.handle.send_command(MessengerCommand::ConnectPeer {
                                    ticket_or_id: ticket.clone(),
                                    nickname: nick,
                                });

                                // Try to determine peer key to select chat immediately
                                if let Ok(addr) = message_core::decode_ticket(&ticket) {
                                    self.select_peer(addr.id.to_string());
                                }

                                self.new_peer_ticket.clear();
                                self.new_peer_nickname.clear();
                                self.show_add_peer_modal = false;
                                self.set_banner("Connecting to peer...".to_string(), false);
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            self.show_add_peer_modal = false;
                        }
                    });
                });
        }

        // Modal: Settings & Identity
        if self.show_settings_modal {
            egui::Window::new("Settings & My Identity")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([500.0, 320.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new("My Nickname:").strong());
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.edit_nickname);
                        if ui.button("Save").clicked() {
                            let nick = self.edit_nickname.trim().to_string();
                            if !nick.is_empty() {
                                let _ =
                                    self.handle.send_command(MessengerCommand::UpdateNickname {
                                        nickname: nick.clone(),
                                    });
                                self.handle.my_nickname = nick;
                                self.set_banner("Nickname updated!".to_string(), false);
                            }
                        }
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(RichText::new("My Public Key (Node ID):").strong());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&self.handle.my_node_id)
                                .monospace()
                                .size(11.5)
                                .color(Color32::from_rgb(130, 200, 255)),
                        );
                        if ui.button("📋 Copy").clicked() {
                            self.copy_to_clipboard(ctx, self.handle.my_node_id.clone(), "Node ID");
                        }
                    });

                    ui.add_space(10.0);
                    ui.label(RichText::new("Shareable Ticket:").strong());
                    let ticket_frame = egui::Frame::new()
                        .fill(Color32::from_rgb(18, 20, 26))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(egui::Margin::same(8));
                    ticket_frame.show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&self.handle.my_ticket)
                                    .monospace()
                                    .size(11.0)
                                    .color(Color32::from_rgb(140, 220, 160)),
                            )
                            .wrap(),
                        );
                    });

                    ui.add_space(6.0);
                    if ui.button("📋 Copy Ticket").clicked() {
                        self.copy_to_clipboard(
                            ctx,
                            self.handle.my_ticket.clone(),
                            "Shareable Ticket",
                        );
                    }

                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!("Data directory: {:?}", self.handle.data_dir))
                            .size(11.5)
                            .color(Color32::from_rgb(130, 140, 160)),
                    );

                    ui.add_space(14.0);
                    if ui.button("Close").clicked() {
                        self.show_settings_modal = false;
                    }
                });
        }
    }
}
