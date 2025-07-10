use std::{
    collections::HashMap,
    sync::{mpsc, Arc},
};

use pitsu_lib::{RemoteRepository, ThisUser};
use uuid::Uuid;

use crate::{
    config::{get_request, CONFIG, PUBLIC_URL},
    pitignore::Pitignore,
    Repository,
};

pub struct RequestCache {
    remote_commit_hash: Option<PendingRequest<Arc<str>>>,
    this_user: Option<PendingRequest<Arc<ThisUser>>>,
    repositories: HashMap<Uuid, PendingRequest<Arc<RemoteRepository>>>,
    stored_repositories: HashMap<Uuid, PendingRequest<Option<Arc<Repository>>>>,
}

#[derive(Debug)]
enum PendingRequest<T>
where
    T: std::fmt::Debug + Send + Sync + 'static,
{
    // #[default]
    // Unsent,
    Pending(mpsc::Receiver<Result<T, Arc<str>>>),
    Response(Result<T, Arc<str>>),
}

type PendingResponse<T> = Result<Option<T>, Arc<str>>;

impl RequestCache {
    pub fn new() -> Self {
        RequestCache {
            remote_commit_hash: None,
            this_user: None,
            repositories: HashMap::new(),
            stored_repositories: HashMap::new(),
        }
    }
    pub fn remote_commit_hash(&mut self) -> PendingResponse<Arc<str>> {
        let new_state = match &self.remote_commit_hash {
            None => {
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(
                    get_request(&format!("{PUBLIC_URL}/api/local/version")),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!(
                                        "Failed to fetch commit hash: {e}"
                                    ))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                                return;
                            }
                        };
                        if response.status != 200 {
                            sender
                                .send(Err(Arc::from(format!(
                                    "Failed to fetch commit hash: {}",
                                    response.status
                                ))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                        let commit_hash: Result<Arc<str>, Arc<str>> = match response.text() {
                            Some(text) => Ok(Arc::from(text.trim())),
                            None => Err(Arc::from("Failed to read response: No text found")),
                        };
                        match commit_hash {
                            Ok(commit_hash) => {
                                sender.send(Ok(commit_hash)).unwrap_or_else(|e| {
                                    log::error!("Failed to send commit hash response: {e}");
                                });
                            }
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!(
                                        "Failed to parse commit hash: {e}"
                                    ))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                            }
                        }
                    },
                );
                PendingRequest::Pending(receiver)
            }
            Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
                Ok(result) => PendingRequest::Response(result),
                Err(mpsc::TryRecvError::Empty) => {
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Disconnected) => PendingRequest::Response(Err(Arc::from(
                    "Request channel disconnected unexpectedly".to_string(),
                ))),
            },
            Some(PendingRequest::Response(ref result)) => {
                return result.clone().map(Some);
            }
        };
        self.remote_commit_hash = Some(new_state);
        Ok(None)
    }
    pub fn this_user(&mut self) -> PendingResponse<Arc<ThisUser>> {
        let new_state = match &self.this_user {
            None => {
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(
                    get_request(&format!("{PUBLIC_URL}/api/user")),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to fetch user: {e}"))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                                return;
                            }
                        };
                        if response.status != 200 {
                            sender
                                .send(Err(Arc::from(format!(
                                    "Failed to fetch user: {}",
                                    response.status
                                ))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                        let user: Result<ThisUser, _> = response.json();
                        match user {
                            Ok(user) => {
                                sender.send(Ok(Arc::new(user))).unwrap_or_else(|e| {
                                    log::error!("Failed to send user response: {e}");
                                });
                            }
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to parse user: {e}"))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                            }
                        }
                    },
                );
                PendingRequest::Pending(receiver)
            }
            Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
                Ok(result) => PendingRequest::Response(result),
                Err(mpsc::TryRecvError::Empty) => {
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Disconnected) => PendingRequest::Response(Err(Arc::from(
                    "Request channel disconnected unexpectedly".to_string(),
                ))),
            },
            Some(PendingRequest::Response(ref result)) => {
                return result.clone().map(Some);
            }
        };
        self.this_user = Some(new_state);
        Ok(None)
    }
    pub fn get_repository(&mut self, uuid: Uuid) -> PendingResponse<Arc<RemoteRepository>> {
        match self.repositories.entry(uuid) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(
                    get_request(&format!("{PUBLIC_URL}/{uuid}")),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!(
                                        "Failed to fetch repository: {e}"
                                    ))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                                return;
                            }
                        };
                        if response.status != 200 {
                            sender
                                .send(Err(Arc::from(format!(
                                    "Failed to fetch repository: {}",
                                    response.status
                                ))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                        let repo: Result<RemoteRepository, _> = response.json();
                        match repo {
                            Ok(repo) => {
                                sender.send(Ok(Arc::new(repo))).unwrap_or_else(|e| {
                                    log::error!("Failed to send repository response: {e}");
                                });
                            }
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!(
                                        "Failed to parse repository: {e}"
                                    ))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                            }
                        }
                    },
                );
                entry.insert(PendingRequest::Pending(receiver));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => match entry.get_mut() {
                PendingRequest::Pending(receiver) => match receiver.try_recv() {
                    Ok(result) => {
                        entry.insert(PendingRequest::Response(result));
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        return Ok(None);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        entry.insert(PendingRequest::Response(Err(Arc::from(
                            "Request channel disconnected unexpectedly".to_string(),
                        ))));
                    }
                },
                PendingRequest::Response(result) => {
                    return result.clone().map(Some);
                }
            },
        };
        Ok(None)
    }
    pub fn get_stored_repository(
        &mut self,
        uuid: Uuid,
        remote: &Arc<RemoteRepository>,
    ) -> PendingResponse<Option<Arc<Repository>>> {
        match self.stored_repositories.entry(uuid) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                let (sender, receiver) = mpsc::channel();
                let remote = Arc::clone(remote);
                std::thread::spawn(move || {
                    let stored_repo = CONFIG.get_stored(uuid);
                    match stored_repo {
                        Ok(Some(repo)) => {
                            let diff = remote.files.diff(&repo.folder);
                            // let diff = repo.folder.diff(&remote.files);
                            let pitignore = match Pitignore::from_repository(repo.path.clone()) {
                                Ok(pitignore) => pitignore,
                                Err(e) => {
                                    log::error!("Failed to get .pitignore for repository: {e}");
                                    sender
                                        .send(Err(Arc::from(format!(
                                            "Failed to get .pitignore for repository: {e}"
                                        ))))
                                        .unwrap_or_else(|e| {
                                            log::error!("Failed to send error response: {e}");
                                        });
                                    return;
                                }
                            };
                            let local = Repository {
                                local: repo,
                                // remote: Arc::clone(&remote),
                                diff: Arc::from(diff),
                                pitignore: Arc::from(pitignore),
                            };
                            sender.send(Ok(Some(Arc::from(local)))).unwrap_or_else(|e| {
                                log::error!("Failed to send stored repository response: {e}");
                            });
                        }
                        Ok(None) => {
                            sender.send(Ok(None)).unwrap_or_else(|e| {
                                log::error!("Failed to send empty stored repository response: {e}");
                            });
                        }
                        Err(e) => {
                            sender
                                .send(Err(Arc::from(format!(
                                    "Failed to get stored repository: {e}"
                                ))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                        }
                    }
                });
                entry.insert(PendingRequest::Pending(receiver));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => match entry.get_mut() {
                PendingRequest::Pending(receiver) => match receiver.try_recv() {
                    Ok(result) => {
                        entry.insert(PendingRequest::Response(result));
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        return Ok(None);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        entry.insert(PendingRequest::Response(Err(Arc::from(
                            "Request channel disconnected unexpectedly".to_string(),
                        ))));
                    }
                },
                PendingRequest::Response(result) => {
                    return result.clone().map(Some);
                }
            },
        };
        Ok(None)
    }
    pub fn reload_repository(&mut self, uuid: Uuid) {
        self.repositories.remove(&uuid);
        self.stored_repositories.remove(&uuid);
    }
}

impl Default for RequestCache {
    fn default() -> Self {
        Self::new()
    }
}
