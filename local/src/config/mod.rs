#![allow(unused)]
use anyhow::Result;
use lazy_static::lazy_static;
use pitsu_lib::{RootFolder, ThisUser};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub static PUBLIC_URL: &str = env!("PITSU_PUBLIC_URL");

lazy_static! {
    static ref CONFIG_DIR: PathBuf = {
        let mut path = {
            let path = dirs::config_dir();
            match path {
                Some(p) => p,
                None => {
                    log::error!("Could not find config directory");
                    panic!("Could not find config directory");
                }
            }
        };
        path.push("pitsu");
        path
    };
    pub static ref CONFIG: Config = load_config().unwrap_or_else(|err| {
        log::error!("Failed to load configuration: {err}");
        panic!("Failed to load configuration: {err}");
    });
}

fn load_config() -> Result<Config> {
    let config_path = CONFIG_DIR.join("config.toml");
    if !config_path.exists() {
        log::warn!("Configuration file not found at: {}", config_path.display());
        if let Err(e) = std::fs::create_dir_all(CONFIG_DIR.as_path()) {
            log::error!("Failed to create configuration directory: {e}");
            panic!("Failed to create configuration directory: {e}");
        }
        let config = Config::new(config_path.clone());
        if let Err(e) = config.save() {
            log::error!("Failed to create default configuration file: {e}");
            panic!("Failed to create default configuration file: {e}");
        }
        return Ok(config);
    }
    log::debug!("Loading configuration from: {}", config_path.display());
    let config_str = std::fs::read_to_string(&config_path)?;
    let config = ConfigVersion::load(&config_str)?;
    log::debug!("Configuration loaded successfully");
    Ok(Config {
        dir: config_path,
        user_info: UserInfo::get(config.api_key_override.clone())?,
        config: Arc::new(Mutex::new(config)),
    })
}

#[derive(Debug, Clone)]
pub struct Config {
    dir: PathBuf,
    user_info: UserInfo,
    config: Arc<Mutex<ConfigV1>>,
}

impl Config {
    fn new(dir: PathBuf) -> Self {
        Config {
            dir,
            config: Arc::new(Mutex::new(ConfigV1::default())),
            user_info: UserInfo::default(),
        }
    }
    fn save(&self) -> Result<()> {
        let config_str = toml::to_string(
            &*self
                .config
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock config: {}", e))?,
        )?;
        std::fs::write(&self.dir, config_str)?;
        Ok(())
    }
    pub fn api_key(&self) -> Arc<str> {
        self.user_info.api_key.clone()
    }
    pub fn username(&self) -> Arc<str> {
        self.user_info.username.clone()
    }
    pub fn uuid(&self) -> Uuid {
        self.user_info.uuid
    }
    pub fn public_url(&self) -> &'static str {
        PUBLIC_URL
    }
    pub fn get_stored(&self, uuid: Uuid) -> Result<Option<Arc<StoredRepository>>> {
        let path = {
            let config = self
                .config
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock config: {}", e))?;
            match config.stored_repositories.get(&uuid) {
                Some(repo) => repo.path.clone(),
                None => return Ok(None),
            }
        };

        if !path.exists() {
            return Err(anyhow::anyhow!(
                "Stored repository path does not exist: {}",
                path.display()
            ));
        }
        let root_folder = RootFolder::ingest_folder(&path)?;
        let mut config = self
            .config
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock config: {}", e))?;
        if let Some(repo) = config.stored_repositories.get_mut(&uuid) {
            let mut new = Arc::new(StoredRepository {
                uuid,
                path: repo.path.clone(),
                folder: Some(root_folder.clone()),
            });
            std::mem::swap(&mut new, repo);
            Ok(Some(repo.clone()))
        } else {
            log::warn!("Stored repository with UUID {uuid} not found in config");
            Ok(None)
        }
    }
    pub fn add_stored(&self, uuid: Uuid, path: PathBuf) -> Result<Arc<StoredRepository>> {
        let stored_repo = Arc::new(StoredRepository {
            uuid,
            folder: Some(RootFolder::ingest_folder(&path)?),
            path,
        });
        {
            let mut config = self
                .config
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock config: {}", e))?;
            config.stored_repositories.insert(uuid, stored_repo.clone());
        }
        self.save()?;
        Ok(stored_repo)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ConfigVersion {
    V1(ConfigV1),
}

impl ConfigVersion {
    fn load(config_str: &str) -> Result<ConfigV1> {
        let config: ConfigVersion = toml::from_str(config_str)?;

        match config {
            ConfigVersion::V1(v1) => Ok(v1),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConfigV1 {
    api_key_override: Option<Arc<str>>,
    stored_repositories: HashMap<Uuid, Arc<StoredRepository>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredRepository {
    uuid: Uuid,
    pub path: PathBuf,
    #[serde(skip)]
    pub folder: Option<RootFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserInfo {
    uuid: Uuid,
    api_key: Arc<str>,
    username: Arc<str>,
}

impl UserInfo {
    pub fn get(override_key: Option<Arc<str>>) -> Result<Self> {
        let api_key = {
            if let Some(key) = override_key {
                key
            } else {
                Arc::from(env!("PITSU_API_KEY"))
            }
        };
        let user = fetch_user(api_key.clone())?;
        Ok(Self {
            uuid: user.user.uuid,
            username: user.user.username,
            api_key,
        })
    }
}

impl Default for UserInfo {
    fn default() -> Self {
        Self::get(None).expect("Failed to get default user info")
    }
}

fn fetch_user(api_key: Arc<str>) -> Result<ThisUser> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("{PUBLIC_URL}/api/user"))
        .header("Authorization", format!("Bearer {api_key}"))
        .send()?;

    if response.status().is_success() {
        let user = response.json()?;
        Ok(user)
    } else {
        Err(anyhow::anyhow!("Failed to fetch user"))
    }
}
