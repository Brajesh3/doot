use anyhow::{Context, Result};
use directories::ProjectDirs;
use iroh::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Identity {
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
    pub nickname: String,
    pub data_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    secret_key_hex: String,
    nickname: String,
}

impl Identity {
    pub fn load_or_create(
        custom_dir: Option<PathBuf>,
        default_nickname: Option<String>,
    ) -> Result<Self> {
        let data_dir = match custom_dir {
            Some(d) => d,
            None => {
                if let Some(proj_dirs) = ProjectDirs::from("com", "irohmessenger", "iroh-messenger")
                {
                    proj_dirs.data_dir().to_path_buf()
                } else {
                    PathBuf::from("./data")
                }
            }
        };

        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory at {:?}", data_dir))?;

        let id_file = data_dir.join("identity.json");
        if id_file.exists() {
            match Self::load_from_file(&id_file, data_dir.clone()) {
                Ok(id) => return Ok(id),
                Err(err) => {
                    tracing::warn!(
                        "Failed to load existing identity from {:?}: {}. Generating new.",
                        id_file,
                        err
                    );
                }
            }
        }

        let secret_key = SecretKey::generate();
        let public_key = secret_key.public();
        let hex_prefix = public_key.to_string();
        let short_id = if hex_prefix.len() > 6 {
            &hex_prefix[..6]
        } else {
            &hex_prefix
        };

        let nickname = default_nickname.unwrap_or_else(|| format!("User-{}", short_id));
        let identity = Self {
            secret_key,
            public_key,
            nickname,
            data_dir,
        };

        identity.save()?;
        Ok(identity)
    }

    fn load_from_file(path: &Path, data_dir: PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let stored: StoredIdentity = serde_json::from_str(&content)?;
        let secret_bytes =
            hex::decode(&stored.secret_key_hex).context("Invalid hex in stored secret key")?;
        let secret_array: [u8; 32] = secret_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Secret key must be 32 bytes"))?;
        let secret_key = SecretKey::from_bytes(&secret_array);
        let public_key = secret_key.public();

        Ok(Self {
            secret_key,
            public_key,
            nickname: stored.nickname,
            data_dir,
        })
    }

    pub fn save(&self) -> Result<()> {
        let id_file = self.data_dir.join("identity.json");
        let stored = StoredIdentity {
            secret_key_hex: hex::encode(self.secret_key.to_bytes()),
            nickname: self.nickname.clone(),
        };
        let json = serde_json::to_string_pretty(&stored)?;
        fs::write(&id_file, json)?;
        Ok(())
    }

    pub fn set_nickname(&mut self, new_nickname: String) -> Result<()> {
        self.nickname = new_nickname;
        self.save()
    }
}
