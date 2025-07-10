pub use anyhow;
use anyhow::Result;
use base64::Engine as _;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    io::{Read as _, Write as _},
    path::PathBuf,
    sync::Arc,
};
use uuid::Uuid;

pub const MAX_UPLOAD_SIZE: usize = 1024 * 1024 * 1024;

lazy_static::lazy_static!(
    static ref ENGINE: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
        &base64::alphabet::URL_SAFE,
        base64::engine::general_purpose::NO_PAD,
    );
    static ref COMPRESSION: flate2::Compression = flate2::Compression::best();
);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum File {
    Folder {
        name: Arc<str>,
        children: Vec<File>,
        size: u64,
    },
    File {
        name: Arc<str>,
        hash: Arc<str>,
        size: u64,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RootFolder {
    #[serde(default)]
    children: Vec<File>,
    #[serde(default)]
    size: u64,
}

impl File {
    pub fn name(&self) -> Arc<str> {
        match self {
            File::Folder { name, .. } => name.clone(),
            File::File { name, .. } => name.clone(),
        }
    }
    pub fn size(&self) -> u64 {
        match self {
            File::Folder { size, .. } => *size,
            File::File { size, .. } => *size,
        }
    }
    fn files(files: Vec<Self>, path_so_far: String) -> Vec<RawFile> {
        let mut result = Vec::new();
        for file in files {
            match file {
                File::Folder {
                    name,
                    children,
                    size: _,
                } => {
                    let new_path = format!("{path_so_far}/{name}");
                    result.extend(Self::files(children, new_path));
                }
                File::File { name, hash, size } => {
                    result.push(RawFile {
                        full_path: format!("{path_so_far}/{name}").into(),
                        hash,
                        size,
                    });
                }
            }
        }
        result
    }
}

fn recursive_count(files: &[File]) -> usize {
    files
        .par_iter()
        .fold(
            || 0,
            |acc, file| match file {
                File::Folder { children, .. } => acc + recursive_count(children),
                File::File { .. } => acc + 1,
            },
        )
        .sum()
}

impl RootFolder {
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn file_count(&self) -> usize {
        recursive_count(&self.children)
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    pub fn ingest_folder(root: &PathBuf) -> Result<Self> {
        let mut children: Vec<File> = std::fs::read_dir(root)?
            .par_bridge()
            .filter_map(|entry| match entry {
                Ok(entry) => {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                        match Self::ingest_folder(&path) {
                            Ok(folder) => Some(File::Folder {
                                name: name.into(),
                                size: folder.children.iter().map(|f| f.size()).sum(),
                                children: folder.children,
                            }),
                            Err(_) => None,
                        }
                    } else {
                        let hash = std::fs::read(&path).ok().map(|data| {
                            use sha2::{Digest, Sha256};
                            let mut hasher = Sha256::new();
                            hasher.update(data);
                            format!("{:x}", hasher.finalize()).into()
                        });
                        if let Some(hash) = hash {
                            // println!("{hash} - {name}");
                            Some(File::File {
                                name: name.into(),
                                hash,
                                size: entry.metadata().ok()?.len(),
                            })
                        } else {
                            None
                        }
                    }
                }
                Err(_) => None,
            })
            .collect();
        children.sort_by_key(|a| a.name());
        Ok(RootFolder {
            size: children.iter().map(|f| f.size()).sum(),
            children,
        })
    }

    pub fn diff(&self, server: &Self) -> Vec<Diff> {
        let mut diffs = Vec::new();

        let self_files = File::files(self.children.clone(), "".into());
        let other_files = File::files(server.children.clone(), "".into());

        for file in &self_files {
            if !other_files.iter().any(|f| f.full_path == file.full_path) {
                diffs.push(Diff {
                    full_path: file.full_path.clone(),
                    change_type: ChangeType::OnClient,
                });
            } else if other_files
                .iter()
                .find(|f| f.full_path == file.full_path)
                .is_some_and(|f| f.hash != file.hash)
            {
                diffs.push(Diff {
                    full_path: file.full_path.clone(),
                    change_type: ChangeType::Modified,
                });
            }
        }

        for file in &other_files {
            if !self_files.iter().any(|f| f.full_path == file.full_path) {
                diffs.push(Diff {
                    full_path: file.full_path.clone(),
                    change_type: ChangeType::OnServer,
                });
            }
        }

        diffs
    }

    pub fn index_through(&self, path: &str) -> Result<Self> {
        let mut current_folder = self.clone();
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for part in parts {
            let next_folder = current_folder.children.iter().find_map(|file| {
                if let File::Folder {
                    name,
                    children,
                    size,
                } = file
                {
                    if &**name == part {
                        Some(RootFolder {
                            children: children.clone(),
                            size: *size,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            current_folder = match next_folder {
                Some(folder) => folder,
                None => return Err(anyhow::anyhow!("Path not found")),
            };
        }
        Ok(current_folder)
    }
    // pub fn iter_files<'a>(&'a self) -> FileIter<'a> {
    //     FileIter {
    //         stack: self.children.iter().collect(),
    //     }
    // }
    pub fn files(&self) -> Vec<(Arc<str>, u64)> {
        recursive_flatten(&self.children, "".into())
    }
}

fn recursive_flatten(files: &[File], path_so_far: String) -> Vec<(Arc<str>, u64)> {
    let mut result = Vec::new();
    for file in files {
        match file {
            File::Folder { name, children, .. } => {
                let new_path = format!("{path_so_far}/{name}");
                result.extend(recursive_flatten(children, new_path));
            }
            File::File {
                name,
                hash: _,
                size,
            } => {
                result.push((format!("{path_so_far}/{name}").into(), *size));
            }
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RawFile {
    full_path: Arc<str>,
    hash: Arc<str>,
    size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Diff {
    pub full_path: Arc<str>,
    pub change_type: ChangeType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    OnServer,
    OnClient,
    Modified,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_ingest_folder() -> Result<()> {
        let root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or(anyhow::anyhow!("Failed to get parent directory"))?
            .join("remote");
        let root_folder = RootFolder::ingest_folder(&root_path)?;
        let json = serde_json::to_string_pretty(&root_folder)?;
        let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test.json");
        fs::write(output_path, json)?;
        println!("Ingested folder structure written to test.json");

        Ok(())
    }

    #[tokio::test]
    async fn test_diff() -> Result<()> {
        let local_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or(anyhow::anyhow!("Failed to get parent directory"))?
            .join("local");
        let remote_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or(anyhow::anyhow!("Failed to get parent directory"))?
            .join("remote");
        let local_folder = RootFolder::ingest_folder(&local_path)?;
        let remote_folder = RootFolder::ingest_folder(&remote_path)?;
        let diffs = local_folder.diff(&remote_folder);

        let diffs_json = serde_json::to_string_pretty(&diffs)?;
        let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diffs.json");
        fs::write(output_path, diffs_json)?;
        println!("Differences written to diffs.json");

        assert!(
            !diffs.is_empty(),
            "No differences found, expected some diffs"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_same_diff() -> Result<()> {
        let local_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or(anyhow::anyhow!("Failed to get parent directory"))?
            .join("local");
        let local_folder = RootFolder::ingest_folder(&local_path)?;
        let diffs = local_folder.diff(&local_folder);

        assert!(diffs.is_empty(), "Expected no differences, but found some");
        println!("No differences found as expected.");
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteRepository {
    pub uuid: Uuid,
    pub name: Arc<str>,
    pub files: RootFolder,
    pub access_level: AccessLevel,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateRemoteRepository {
    pub name: Arc<str>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimpleRemoteRepository {
    pub uuid: Uuid,
    pub name: Arc<str>,
    pub size: u64,
    pub file_count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub uuid: Uuid,
    pub username: Arc<str>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThisUser {
    pub user: User,
    pub owned_repositories: Vec<SimpleRemoteRepository>,
    pub accessible_repositories: Vec<(SimpleRemoteRepository, AccessLevel)>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessLevel {
    None = 0,
    Read = 1,
    Write = 2,
    Admin = 3,
}

impl TryFrom<&str> for AccessLevel {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        Ok(match s {
            "N" => AccessLevel::None,
            "R" => AccessLevel::Read,
            "W" => AccessLevel::Write,
            "RW+" => AccessLevel::Admin,
            _ => return Err(anyhow::anyhow!("Invalid access level: {}", s)),
        })
    }
}

impl std::fmt::Display for AccessLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level_str = match self {
            AccessLevel::None => "N",
            AccessLevel::Read => "R",
            AccessLevel::Write => "W",
            AccessLevel::Admin => "RW+",
        };
        write!(f, "{level_str}")
    }
}

impl TryFrom<String> for AccessLevel {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self> {
        Self::try_from(s.as_str())
    }
}

impl std::cmp::Ord for AccessLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl std::cmp::PartialOrd for AccessLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserWithAccess {
    #[serde(flatten)]
    pub user: User,
    pub access_level: AccessLevel,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetAccess {
    pub user: Uuid,
    pub access_level: AccessLevel,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateRemoteRepository {
    pub name: Arc<str>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileUpload {
    pub files: Vec<UploadFile>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadFile {
    pub path: Arc<str>,
    bytes: Arc<[u8]>,
    #[serde(skip)]
    decoded: Option<Arc<[u8]>>,
}

impl UploadFile {
    pub fn new(path: Arc<str>, raw_bytes: Vec<u8>) -> Result<Self> {
        let bytes = compress(&raw_bytes)?;
        Ok(Self {
            path,
            bytes,
            decoded: None,
        })
    }
    pub fn get_bytes(&mut self) -> Result<Arc<[u8]>> {
        if let Some(decoded) = &self.decoded {
            return Ok(decoded.clone());
        }
        let decoded = decompress(&self.bytes)?;
        self.decoded = Some(decoded.clone());
        Ok(decoded)
    }
    pub fn size(&self) -> usize {
        let bytes = self.bytes.len();
        let decoded = self.decoded.as_ref().map_or(0, |d| d.len());
        let path = self.path.len();
        r#"{"path": "",bytes: ""}"#.len() + bytes + decoded + path
    }
}

fn compress(data: &[u8]) -> Result<Arc<[u8]>> {
    let mut compressed_data = Vec::new();
    let mut encoder = flate2::write::GzEncoder::new(&mut compressed_data, *COMPRESSION);
    encoder.write_all(data)?;
    encoder.finish()?;
    Ok(compressed_data.into())
}

fn decompress(compressed: &Arc<[u8]>) -> Result<Arc<[u8]>> {
    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
    let mut decompressed_data = Vec::new();
    decoder
        .read_to_end(&mut decompressed_data)
        .map_err(|e| anyhow::anyhow!("Failed to decompress data: {}", e))?;
    Ok(decompressed_data.into())
}

pub fn encode_base64(data: &[u8]) -> String {
    ENGINE.encode(data)
}

pub fn decode_base64(data: &str) -> Result<Vec<u8>> {
    ENGINE
        .decode(data)
        .map_err(|e| anyhow::anyhow!("Failed to decode base64 data: {}", e))
}

pub fn encode_string_base64(data: &str) -> String {
    encode_base64(data.as_bytes())
}

pub fn decode_string_base64(data: &str) -> Result<String> {
    let bytes = decode_base64(data)?;
    String::from_utf8(bytes).map_err(|e| anyhow::anyhow!("Failed to decode base64 string: {}", e))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionNumber {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub folder_hash: String,
}

impl std::fmt::Display for VersionNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl VersionNumber {
    pub fn new(path: &PathBuf) -> Result<Self> {
        let cargo_toml_path = path.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            return Err(anyhow::anyhow!("Cargo.toml not found"));
        }
        let mut file = std::fs::File::open(cargo_toml_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let mut major = 0;
        let mut minor = 0;
        let mut patch = 0;
        for line in contents.lines() {
            if line.starts_with("version = ") {
                let version = line.split('=').nth(1).unwrap().trim();
                let parts: Vec<&str> = version.split('.').collect();
                if parts.len() >= 3 {
                    major = parts[0].parse()?;
                    minor = parts[1].parse()?;
                    patch = parts[2].parse()?;
                }
            }
        }
        if major == 0 && minor == 0 && patch == 0 {
            return Err(anyhow::anyhow!("Version not found in Cargo.toml"));
        }
        // Get a hash of the folder contents
        let mut hasher = sha2::Sha256::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let mut file = std::fs::File::open(entry.path())?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                hasher.update(&buffer);
            }
        }
        let folder_hash = hasher.finalize().to_vec();
        let folder_hash = base64::engine::general_purpose::STANDARD.encode(folder_hash);
        Ok(VersionNumber {
            major,
            minor,
            patch,
            folder_hash,
        })
    }
    pub fn is_dev(&self) -> bool {
        self.folder_hash == "dev"
    }
}

impl std::cmp::Ord for VersionNumber {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.major > other.major {
            return std::cmp::Ordering::Greater;
        } else if self.major < other.major {
            return std::cmp::Ordering::Less;
        }
        if self.minor > other.minor {
            return std::cmp::Ordering::Greater;
        } else if self.minor < other.minor {
            return std::cmp::Ordering::Less;
        }
        if self.patch > other.patch {
            return std::cmp::Ordering::Greater;
        } else if self.patch < other.patch {
            return std::cmp::Ordering::Less;
        }
        std::cmp::Ordering::Equal
    }
}

impl std::cmp::PartialOrd for VersionNumber {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for VersionNumber {}

impl PartialEq for VersionNumber {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch == other.patch
    }
}
