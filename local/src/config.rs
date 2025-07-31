#![allow(unused)]
use anyhow::Result;
use core::panic;
use ehttp::{fetch, Request};
use lazy_static::lazy_static;
use pitsu_lib::{Pitignore, RootFolder, ThisUser, VersionNumber};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::dialogue;

pub fn setup() {
    std::panic::set_hook(Box::new(crate::dialogue::rfd_panic_dialogue));
    // let exe = std::env::current_exe().expect("Failed to get current executable path");
    // let binary_name = exe
    //     .file_name()
    //     .and_then(|s| s.to_str())
    //     .map(|s| s.to_string())
    //     .expect("Failed to convert executable name to string");
    // if let Some(api_key) = binary_name.strip_prefix("pitsu.") {
    //     let api_key = api_key.strip_suffix(".exe").unwrap_or(api_key);
    //     let api_key = api_key.split(' ').next().unwrap_or(api_key); // removes any accidental multi-file downloads (1) etc.
    //     log::info!("Extracted API key from binary name: {api_key}");
    //     let new_exe_name = exe.with_file_name("pitsu.exe");
    //     std::fs::rename(exe, new_exe_name).expect("Failed to rename executable");
    //     // std::env::set_var("PITSU_API_KEY", api_key.trim());
    //     unsafe {
    //         SET_API_KEY = Some(api_key.to_string());
    //     }
    // }
    std::env::set_var("SEQ_API_KEY", env!("LOCAL_SEQ_API_KEY"));
    std::env::set_var("SEQ_API_URL", env!("SEQ_API_URL"));
    datalust_logger::init(&format!("PITSU <{}>", CONFIG.uuid())).expect("Failed to initialize logger");
    if CONFIG.api_key().is_empty() {
        log::error!("PITSU_API_KEY is not set. Please try to download again.");
        panic!("PITSU_API_KEY is not set. Please try to download again.");
    }
    if CONFIG.public_url().is_empty() {
        log::warn!("PITSU_PUBLIC_URL is not set. Please try to download again.");
        panic!("PITSU_PUBLIC_URL is not set. Please try to download again.");
    }
}

pub static PUBLIC_URL: &str = env!("PITSU_PUBLIC_URL");
pub const MAX_PATH_LENGTH: usize = 32;
// pub const VERSION_NUMBER: &str = env!("VERSION_NUMBER");

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
    pub static ref VERSION_NUMBER: VersionNumber = {
        // serde_json::from_str(env!("VERSION_NUMBER")).unwrap_or_else(|err| {
        //     log::error!("Failed to parse version number: {err}");
        //     panic!("Failed to parse version number: {err}");
        // })
        // Parse version number from the exe manifest

        VersionNumber {
            major: env!("VERSION_MAJOR").parse().expect("Failed to parse major version"),
            minor: env!("VERSION_MINOR").parse().expect("Failed to parse minor version"),
            patch: env!("VERSION_PATCH").parse().expect("Failed to parse patch version"),
            folder_hash: env!("VERSION_HASH").parse().expect("Failed to parse folder hash"),
        }
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
    let config = Config {
        dir: config_path,
        user_info: UserInfo::get(config.api_key.clone()),

        config: Arc::new(Mutex::new(config)),
    };
    if let Err(e) = config.save() {
        log::error!("Failed to save configuration: {e}");
        panic!("Failed to save configuration: {e}");
    }
    Ok(config)
}

#[derive(Debug, Clone)]
pub struct Config {
    dir: PathBuf,
    user_info: Pending<UserInfo>,
    config: Arc<Mutex<ConfigV1>>,
}

impl Config {
    fn new(dir: PathBuf) -> Self {
        let config = ConfigV1::default();
        Config {
            dir,
            user_info: UserInfo::get(config.api_key.clone()),
            config: Arc::new(Mutex::new(config)),
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
        match self.config.lock() {
            Ok(config) => config.api_key.clone(),
            Err(e) => {
                log::error!("Failed to lock config: {e}");
                panic!("Failed to lock config: {}", e);
            }
        }
    }
    pub fn username(&self) -> Arc<str> {
        self.user_info
            .wait_ready()
            .expect("Failed to wait for user info")
            .username
            .clone()
    }
    pub fn uuid(&self) -> Uuid {
        self.user_info.wait_ready().expect("Failed to wait for user info").uuid
    }
    pub fn public_url(&self) -> &'static str {
        PUBLIC_URL
    }
    pub fn get_stored(&self, uuid: Uuid) -> Result<Option<Arc<LocalRepository>>> {
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
        let overrides = {
            let config = self
                .config
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock config: {}", e))?;
            match config.stored_repositories.get(&uuid) {
                Some(repo) => repo.overrides.clone(),
                None => Pitignore::default(),
            }
        };
        let local_repo = LocalRepository {
            uuid,
            path,
            folder: root_folder,
            overrides,
        };
        Ok(Some(Arc::new(local_repo)))
    }
    pub fn add_stored(&self, uuid: Uuid, path: PathBuf) -> Result<()> {
        let stored_repo = Arc::new(StoredRepository {
            uuid,
            path,
            overrides: Pitignore::default(),
        });
        {
            let mut config = self
                .config
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock config: {}", e))?;
            config.stored_repositories.insert(uuid, stored_repo.clone());
        }
        self.save()?;
        log::info!("Stored repository added: {}", stored_repo.path.display());
        Ok(())
    }
    pub fn skip_confirmation(&self) -> bool {
        let config = self
            .config
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock config: {}", e))
            .expect("Failed to lock config");
        config.skip_confirmation
    }
    pub fn set_skip_confirmation(&self, skip: bool) {
        {
            let mut config = self.config.lock().expect("Failed to lock config");
            config.skip_confirmation = skip;
        }
        if let Err(e) = self.save() {
            log::error!("Failed to save configuration after toggling skip confirmation: {e}");
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigV1 {
    #[serde(default = "get_api_key")]
    #[serde(skip_serializing_if = "arc_str_empty")]
    api_key: Arc<str>,
    stored_repositories: HashMap<Uuid, Arc<StoredRepository>>,
    #[serde(default)]
    skip_confirmation: bool,
}

impl Default for ConfigV1 {
    fn default() -> Self {
        ConfigV1 {
            api_key: get_api_key(),
            stored_repositories: HashMap::new(),
            skip_confirmation: false,
        }
    }
}

fn arc_str_empty(s: &Arc<str>) -> bool {
    s.is_empty()
}

// fn get_api_key() -> Arc<str> {
//     let resp = crate::dialogue::get_api_key().expect("Failed to get API key from user");
//     resp.trim().to_string().into()
// }

pub static mut SET_API_KEY: Option<String> = None;

#[inline(never)]
fn get_api_key() -> Arc<str> {
    // Arc::from(env!("PITSU_API_KEY_PLACEHOLDER"))
    // if let Some(val) = unsafe {
    //     #[allow(static_mut_refs)]
    //     SET_API_KEY.clone()
    // } {
    //     log::info!("Using API key from environment variable");
    //     Arc::from(val)
    // } else {
    //     log::info!("No API key found in binary name");
    //     dialogue::get_api_key()
    //         .expect("Failed to get API key from user")
    //         .trim()
    //         .to_string()
    //         .into()
    // }
    "________________________________PITSU_API_KEY_PLACEHOLDER________________________________"
        .trim_matches('_')
        .to_string()
        .into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRepository {
    uuid: Uuid,
    path: PathBuf,
    #[serde(
        deserialize_with = "rename_deserialize_pitignore",
        serialize_with = "rename_serialize_pitignore"
    )]
    overrides: Pitignore,
}

// when serialized, i want overrides: Pitignore, to be flattened to overrides: Vec<pitignore.patterns>
fn rename_serialize_pitignore<S>(pitignore: &Pitignore, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_newtype_struct("overrides", &pitignore.patterns)
}

fn rename_deserialize_pitignore<'de, D>(deserializer: D) -> Result<Pitignore, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let patterns: Vec<(usize, pitsu_lib::PitignorePattern)> = serde::Deserialize::deserialize(deserializer)?;
    Ok(Pitignore { patterns })
}

#[derive(Debug, Clone)]
pub struct LocalRepository {
    pub uuid: Uuid,
    pub path: PathBuf,
    pub overrides: Pitignore,
    pub folder: RootFolder,
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    uuid: Uuid,
    username: Arc<str>,
}

impl<'de> Deserialize<'de> for UserInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = ThisUser::deserialize(deserializer)?;
        Ok(UserInfo {
            uuid: inner.user.uuid,
            username: inner.user.username,
        })
    }
}

impl UserInfo {
    pub fn get(api_key: Arc<str>) -> Pending<Self> {
        let mut request = Request::get(format!("{PUBLIC_URL}/api/user"));
        request
            .headers
            .insert("Authorization", format!("Bearer {}", api_key.as_ref()));
        Pending::<UserInfo>::new(request)
    }
}

type TResult<T> = Result<Arc<T>, Arc<anyhow::Error>>;
#[derive(Debug, Clone)]
pub struct Pending<T: Debug + DeserializeOwned + Send + Sync + 'static> {
    channel: Arc<Mutex<std::sync::mpsc::Receiver<TResult<T>>>>,
    value: Arc<Mutex<Option<TResult<T>>>>,
}

impl<T: Debug + DeserializeOwned + Send + Sync + 'static> Pending<T> {
    pub fn new(request: Request) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<TResult<T>>();
        fetch(request, move |response| {
            if let Err(err) = response {
                if let Err(e) = sender.send(Err(Arc::new(anyhow::anyhow!(err)))) {
                    log::error!("Failed to send error through channel: {e}");
                }
            } else if let Ok(response) = response {
                if !(200..=299).contains(&response.status) {
                    if let Err(e) = sender.send(Err(Arc::new(anyhow::anyhow!(
                        "Request failed with status: {}",
                        response.status
                    )))) {
                        log::error!("Failed to send error through channel: {e}");
                    }
                } else if let Ok(value) = response.json::<T>() {
                    if let Err(e) = sender.send(Ok(Arc::new(value))) {
                        log::error!("Failed to send value through channel: {e}");
                    }
                } else if let Err(e) = sender.send(Err(Arc::new(anyhow::anyhow!("Failed to parse response as JSON")))) {
                    log::error!("Failed to send error through channel: {e}");
                }
            }
        });
        Pending {
            channel: Arc::new(Mutex::new(receiver)),
            value: Arc::new(Mutex::new(None)),
        }
    }

    fn get_cached(&self) -> Result<Option<Arc<T>>, Arc<anyhow::Error>> {
        if let Some(value) = &*self.value.lock().map_err(|_| anyhow::anyhow!("Mutex lock failed"))? {
            match value.clone() {
                Ok(arc_value) => Ok(Some(arc_value.clone())),
                Err(err) => Err(err),
            }
        } else {
            Ok(None)
        }
    }

    fn set_cached(&self, value: Result<Arc<T>, Arc<anyhow::Error>>) -> Result<(), Arc<anyhow::Error>> {
        let mut value_lock = self
            .value
            .lock()
            .map_err(|_| Arc::new(anyhow::anyhow!("Mutex lock failed")))?;
        *value_lock = Some(value);
        Ok(())
    }

    pub fn try_ready(&mut self) -> Result<Option<Arc<T>>, Arc<anyhow::Error>> {
        if let Some(value) = self.get_cached()? {
            return Ok(Some(value));
        }
        let channel = self
            .channel
            .lock()
            .map_err(|_| Arc::new(anyhow::anyhow!("Failed to lock channel mutex")))?;
        match channel.try_recv() {
            Ok(result) => {
                self.set_cached(result)?;
                self.get_cached()
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(Arc::new(anyhow::anyhow!("Channel disconnected"))),
        }
    }

    pub fn wait_ready(&self) -> Result<Arc<T>, Arc<anyhow::Error>> {
        if let Some(value) = self.get_cached()? {
            return Ok(value);
        }
        let channel = self
            .channel
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock channel mutex"))?;
        match channel.recv() {
            Ok(result) => {
                self.set_cached(result)?;
                self.get_cached()?
                    .ok_or_else(|| Arc::new(anyhow::anyhow!("No value available after waiting")))
            }
            Err(e) => {
                log::error!("Failed to receive from channel: {e}");
                Err(Arc::new(anyhow::anyhow!("Channel disconnected")))
            }
        }
    }
}

pub mod icons {
    use std::sync::Arc;

    use eframe::egui::{include_image, IconData, ImageSource};
    use image::ImageFormat;
    use lazy_static::lazy_static;
    use resvg::{tiny_skia::Pixmap, usvg::Transform};

    include_flate::flate!(static SVG: str from "assets/p51-03.svg");
    static IMAGE_SIZE: u32 = 1024;
    lazy_static! {
        pub static ref WINDOW_ICON: Arc<IconData> = {
            let svg = resvg::usvg::Tree::from_str(&SVG, &resvg::usvg::Options::default()).expect("Failed to parse SVG");

            let mut pixmap = Pixmap::new(IMAGE_SIZE, IMAGE_SIZE).expect("Failed to create pixmap");
            resvg::render(&svg, Transform::default(), &mut pixmap.as_mut());
            Arc::new(IconData {
                rgba: pixmap.data().to_vec(),
                width: IMAGE_SIZE,
                height: IMAGE_SIZE,
            })
        };
    }
}

pub fn get_request(url: &str) -> Request {
    let mut request = Request::get(url);
    request
        .headers
        .insert("Authorization", format!("Bearer {}", CONFIG.api_key()));
    request
}

pub fn post_request(url: &str, body: Value) -> Request {
    let mut request: Request = Request::json(url, &body).expect("Failed to create JSON request");
    request
        .headers
        .insert("Authorization", format!("Bearer {}", CONFIG.api_key()));
    request.method = "POST".to_string();
    request
}

pub fn delete_request(url: &str) -> Request {
    let mut request = Request::get(url);
    request.method = "DELETE".to_string();
    request
        .headers
        .insert("Authorization", format!("Bearer {}", CONFIG.api_key()));
    request
}

pub fn delete_request_with_body(url: &str, body: Value) -> Request {
    let mut request = Request::json(url, &body).expect("Failed to create JSON request");
    request.method = "DELETE".to_string();
    request
        .headers
        .insert("Authorization", format!("Bearer {}", CONFIG.api_key()));
    request
}
