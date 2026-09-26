use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageDirection {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageStatus {
    Sending,
    Sent,
    Delivered,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: Uuid,
    pub conversation_peer: String, // String representation of PublicKey
    pub direction: MessageDirection,
    pub sender_name: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub public_key: String,
    pub nickname: String,
    pub ticket: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub unread_count: usize,
    #[serde(default)]
    pub last_message_preview: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct StoreData {
    contacts: HashMap<String, Contact>,
    messages: HashMap<String, Vec<StoredMessage>>,
}

pub struct Store {
    data_dir: PathBuf,
    contacts: HashMap<String, Contact>,
    messages: HashMap<String, Vec<StoredMessage>>,
}

impl Store {
    pub fn load(data_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&data_dir)?;
        let store_file = data_dir.join("store.json");

        if store_file.exists() {
            match Self::load_file(&store_file) {
                Ok(data) => {
                    return Ok(Self {
                        data_dir,
                        contacts: data.contacts,
                        messages: data.messages,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed reading store.json: {}. Creating fresh store.", e);
                }
            }
        }

        Ok(Self {
            data_dir,
            contacts: HashMap::new(),
            messages: HashMap::new(),
        })
    }

    fn load_file(path: &Path) -> Result<StoreData> {
        let text = fs::read_to_string(path)?;
        let data: StoreData = serde_json::from_str(&text)?;
        Ok(data)
    }

    pub fn save(&self) -> Result<()> {
        let store_file = self.data_dir.join("store.json");
        let data = StoreData {
            contacts: self.contacts.clone(),
            messages: self.messages.clone(),
        };
        let text = serde_json::to_string_pretty(&data)?;
        fs::write(&store_file, text)?;
        Ok(())
    }

    pub fn add_or_update_contact(&mut self, mut contact: Contact) -> Result<()> {
        if let Some(existing) = self.contacts.get(&contact.public_key) {
            if contact.ticket.is_none() {
                contact.ticket = existing.ticket.clone();
            }
            if contact.last_message_preview.is_none() {
                contact.last_message_preview = existing.last_message_preview.clone();
            }
        }
        self.contacts.insert(contact.public_key.clone(), contact);
        self.save()
    }

    pub fn update_contact_last_seen(&mut self, peer_key: &str) -> Result<()> {
        if let Some(c) = self.contacts.get_mut(peer_key) {
            c.last_seen = Some(Utc::now());
            self.save()?;
        }
        Ok(())
    }

    pub fn add_message(&mut self, msg: StoredMessage) -> Result<()> {
        let peer = msg.conversation_peer.clone();
        let preview = if msg.content.chars().count() > 40 {
            format!("{}...", msg.content.chars().take(40).collect::<String>())
        } else {
            msg.content.clone()
        };

        if let Some(contact) = self.contacts.get_mut(&peer) {
            contact.last_message_preview = Some(preview);
            if msg.direction == MessageDirection::Incoming {
                contact.unread_count += 1;
            }
        } else {
            // Auto-create contact entry if not existing
            let nickname = if msg.sender_name.is_empty() {
                let short = if peer.len() > 6 { &peer[..6] } else { &peer };
                format!("Peer-{}", short)
            } else {
                msg.sender_name.clone()
            };

            let contact = Contact {
                public_key: peer.clone(),
                nickname,
                ticket: None,
                last_seen: Some(Utc::now()),
                unread_count: if msg.direction == MessageDirection::Incoming {
                    1
                } else {
                    0
                },
                last_message_preview: Some(preview),
            };
            self.contacts.insert(peer.clone(), contact);
        }

        self.messages.entry(peer).or_default().push(msg);
        self.save()
    }

    pub fn update_message_status(
        &mut self,
        peer_key: &str,
        message_id: Uuid,
        status: MessageStatus,
    ) -> Result<()> {
        if let Some(msgs) = self.messages.get_mut(peer_key) {
            if let Some(msg) = msgs.iter_mut().find(|m| m.id == message_id) {
                msg.status = status;
                self.save()?;
            }
        }
        Ok(())
    }

    pub fn mark_as_read(&mut self, peer_key: &str) -> Result<()> {
        if let Some(contact) = self.contacts.get_mut(peer_key) {
            if contact.unread_count > 0 {
                contact.unread_count = 0;
                self.save()?;
            }
        }
        Ok(())
    }

    pub fn get_messages(&self, peer_key: &str) -> Vec<StoredMessage> {
        self.messages.get(peer_key).cloned().unwrap_or_default()
    }

    pub fn get_contacts(&self) -> Vec<Contact> {
        let mut list: Vec<Contact> = self.contacts.values().cloned().collect();
        // Sort contacts by last seen or nickname
        list.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        list
    }

    pub fn get_contact(&self, peer_key: &str) -> Option<Contact> {
        self.contacts.get(peer_key).cloned()
    }
}
