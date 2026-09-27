use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64;
use iroh::{EndpointAddr, PublicKey};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

pub const DOOT_CHAT_ALPN: &[u8] = b"/doot/chat/1.0.0";
pub const ALPN_NAME: &[u8] = DOOT_CHAT_ALPN;
pub const LEGACY_ALPN_NAME: &[u8] = b"/iroh-messenger/1.0.0";
const TICKET_PREFIX: &str = "iroh-msg:";
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10 MB

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WireMessage {
    Text {
        id: Uuid,
        sender_name: String,
        timestamp_millis: i64,
        content: String,
    },
    FileOffer {
        id: Uuid,
        sender_name: String,
        timestamp_millis: i64,
        file_name: String,
        file_size: u64,
        mime_type: String,
        blake3_hash: String,
        is_directory: bool,
    },
    FileStreamStart {
        file_id: Uuid,
    },
    Ack {
        message_id: Uuid,
    },
    Typing {
        is_typing: bool,
    },
    Ping,
    Pong,
}

impl WireMessage {
    pub fn message_id(&self) -> Option<Uuid> {
        match self {
            Self::Text { id, .. } => Some(*id),
            Self::FileOffer { id, .. } => Some(*id),
            Self::FileStreamStart { file_id } => Some(*file_id),
            Self::Ack { message_id } => Some(*message_id),
            _ => None,
        }
    }
}

pub async fn send_wire_message<W>(writer: &mut W, msg: &WireMessage) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let data = serde_json::to_vec(msg)?;
    let len = data.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn recv_wire_message<R>(reader: &mut R) -> Result<WireMessage>
where
    R: AsyncReadExt + Unpin,
{
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len > MAX_MESSAGE_SIZE {
        anyhow::bail!("Message size {} exceeds limit {}", len, MAX_MESSAGE_SIZE);
    }
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await?;
    let msg: WireMessage = serde_json::from_slice(&data)?;
    Ok(msg)
}

/// Encodes an `EndpointAddr` into a clean, shareable ticket string.
pub fn encode_ticket(addr: &EndpointAddr) -> Result<String> {
    let json = serde_json::to_vec(addr)?;
    let b64 = BASE64.encode(json);
    Ok(format!("{}{}", TICKET_PREFIX, b64))
}

/// Decodes a shareable ticket, base64 payload, JSON, or raw Public Key into an `EndpointAddr`.
pub fn decode_ticket(input: &str) -> Result<EndpointAddr> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Input ticket/address is empty");
    }

    // Case 1: Prefixed ticket string
    if let Some(rest) = trimmed.strip_prefix(TICKET_PREFIX) {
        let json_bytes = BASE64.decode(rest).context("Invalid Base64 in ticket")?;
        let addr: EndpointAddr = serde_json::from_slice(&json_bytes)
            .context("Invalid endpoint address payload in ticket")?;
        return Ok(addr);
    }

    // Case 2: Raw JSON
    if trimmed.starts_with('{')
        && let Ok(addr) = serde_json::from_str::<EndpointAddr>(trimmed)
    {
        return Ok(addr);
    }

    // Case 3: Raw Base64 without prefix
    if let Ok(json_bytes) = BASE64.decode(trimmed)
        && let Ok(addr) = serde_json::from_slice::<EndpointAddr>(&json_bytes)
    {
        return Ok(addr);
    }

    // Case 4: Raw PublicKey string (e.g. Hex or z-base32)
    if let Ok(pk) = PublicKey::from_str(trimmed) {
        return Ok(EndpointAddr::from(pk));
    }

    anyhow::bail!(
        "Unable to parse ticket: expected 'iroh-msg:<base64>', JSON EndpointAddr, or valid Public Key"
    )
}
