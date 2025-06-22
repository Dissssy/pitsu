pub use anyhow;
use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

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
        // Recursively collect all files in the folder structure
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
    // Count the number of files in the folder structure
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
        // Count the number of files in the folder structure
        recursive_count(&self.children)
    }

    // If the file is a folder, call this function again to get its children
    pub fn ingest_folder(root: &PathBuf) -> Result<Self> {
        let children: Vec<File> = std::fs::read_dir(root)?
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
                            println!("{hash} - {name}");
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
        Ok(RootFolder {
            size: children.iter().map(|f| f.size()).sum(),
            children,
        })
    }

    pub fn diff(&self, remote: &Self) -> Vec<Diff> {
        // Compare self with other, and return a vector of Diff objects
        let mut diffs = Vec::new();

        // first flatten the children into a single vector of RawFile
        let self_files = File::files(self.children.clone(), "".into());
        let other_files = File::files(remote.children.clone(), "".into());

        for file in &self_files {
            if !other_files.iter().any(|f| f.full_path == file.full_path) {
                diffs.push(Diff {
                    full_path: file.full_path.clone(),
                    change_type: ChangeType::Removed,
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
                    change_type: ChangeType::Added,
                });
            }
        }

        diffs
    }

    pub fn index_through(&self, path: &str) -> Result<Self> {
        // Return the folder structure at the given path
        // For example, the structure is
        // /git { /pitsu { /remote, /local } }
        // path = /git/pitsu
        // response should be { /remote, /local }
        // get this information from the self.children
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChangeType {
    Added,
    Removed,
    Modified,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_ingest_folder() -> Result<()> {
        // read in the root folder of "/git/pitsu/remote" and write it to /git/pitsu/test.json
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
        // ingest the folders at "/git/pitsu/local" and "/git/pitsu/remote" and diff them
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
        // Write the diffs to a file
        let diffs_json = serde_json::to_string_pretty(&diffs)?;
        let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diffs.json");
        fs::write(output_path, diffs_json)?;
        println!("Differences written to diffs.json");
        // Assert that there are differences
        assert!(
            !diffs.is_empty(),
            "No differences found, expected some diffs"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_same_diff() -> Result<()> {
        // ingest the folders at "/git/pitsu/local" and "/git/pitsu/local" and diff them
        let local_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or(anyhow::anyhow!("Failed to get parent directory"))?
            .join("local");
        let local_folder = RootFolder::ingest_folder(&local_path)?;
        let diffs = local_folder.diff(&local_folder);
        // Assert that there are no differences
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
