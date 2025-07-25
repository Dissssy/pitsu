use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc, Arc},
};

use pitsu_lib::{
    ChangeType, CreateRemoteRepository, FileUpload, Pitignore, RemoteRepository, ThisUser, UploadFile, User,
    UserWithAccess, VersionNumber,
};
use uuid::Uuid;

use crate::{
    config::{delete_request, delete_request_with_body, get_request, post_request, CONFIG, PUBLIC_URL},
    Repository,
};

pub struct RequestCache {
    this_user: Option<PendingRequest<Arc<ThisUser>>>,
    users: Option<PendingRequest<Vec<User>>>,
    upload: Option<PendingRequest<Uuid>>,
    download: Option<PendingRequest<Uuid>>,
    remote_version_number: Option<PendingRequest<Arc<VersionNumber>>>,
    remote_update_bytes: Option<PendingRequest<Arc<[u8]>>>,
    create_repository: Option<PendingRequest<Arc<RemoteRepository>>>,
    repositories: HashMap<Uuid, PendingRequest<Arc<RemoteRepository>>>,
    stored_repositories: HashMap<Uuid, PendingRequest<Option<Arc<Repository>>>>,
    user_action: Option<PendingRequest<Uuid>>,
    pub new_repository_name: String,
    pub new_repository_path: Option<PathBuf>,
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

impl<T> PendingRequest<T>
where
    T: std::fmt::Debug + Send + Sync + 'static,
{
    pub fn in_progress(&self) -> bool {
        matches!(self, PendingRequest::Pending(_))
    }
}

type PendingResponse<T> = Result<Option<T>, Arc<str>>;

impl RequestCache {
    pub fn new() -> Self {
        RequestCache {
            this_user: None,
            users: None,
            upload: None,
            download: None,
            remote_version_number: None,
            remote_update_bytes: None,
            repositories: HashMap::new(),
            stored_repositories: HashMap::new(),
            user_action: None,
            new_repository_name: String::new(),
            new_repository_path: None,
            create_repository: None,
        }
    }
    pub fn create_repository(&mut self, only_check: bool) -> PendingResponse<Arc<RemoteRepository>> {
        let new_state = match &self.create_repository {
            None => {
                if only_check {
                    return Ok(None);
                }
                let (sender, receiver) = mpsc::channel();
                let path = self.new_repository_path.clone();
                ehttp::fetch(
                    post_request(
                        &format!("{PUBLIC_URL}/api/repository"),
                        serde_json::to_value(CreateRemoteRepository {
                            name: self.new_repository_name.clone().into(),
                        })
                        .expect("Failed to serialize repository creation request"),
                    ),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to create repository: {e}"))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                                return;
                            }
                        };
                        if response.status != 201 {
                            sender
                                .send(Err(Arc::from(format!(
                                    "Failed to create repository: {}",
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
                                if let Some(path) = path {
                                    CONFIG.add_stored(repo.uuid, path).ok();
                                }
                                sender.send(Ok(Arc::new(repo))).unwrap_or_else(|e| {
                                    log::error!("Failed to send repository creation response: {e}");
                                });
                            }
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to parse repository: {e}"))))
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
                Err(mpsc::TryRecvError::Empty) => return Ok(None),
                Err(mpsc::TryRecvError::Disconnected) => {
                    PendingRequest::Response(Err(Arc::from("Request channel disconnected unexpectedly".to_string())))
                }
            },
            Some(PendingRequest::Response(ref result)) => return result.clone().map(Some),
        };
        self.create_repository = Some(new_state);
        Ok(None)
    }
    pub fn remote_version_number(&mut self) -> PendingResponse<Arc<VersionNumber>> {
        let new_state = match &self.remote_version_number {
            None => {
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(
                    get_request(&format!("{PUBLIC_URL}/api/local/version")),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to fetch commit hash: {e}"))))
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
                        // let version_number: Result<Arc<str>, Arc<str>> = match response.text() {
                        //     Some(text) => Ok(Arc::from(text.trim())),
                        //     None => Err(Arc::from("Failed to read response: No text found")),
                        // };
                        // match version_number {
                        //     Ok(version_number) => {
                        //         sender.send(Ok(version_number)).unwrap_or_else(|e| {
                        //             log::error!("Failed to send commit hash response: {e}");
                        //         });
                        //     }
                        //     Err(e) => {
                        //         sender
                        //             .send(Err(Arc::from(format!(
                        //                 "Failed to parse commit hash: {e}"
                        //             ))))
                        //             .unwrap_or_else(|e| {
                        //                 log::error!("Failed to send error response: {e}");
                        //             });
                        //     }
                        // }
                        let version_number = response.json::<VersionNumber>();
                        match version_number {
                            Ok(version_number) => {
                                sender.send(Ok(Arc::from(version_number))).unwrap_or_else(|e| {
                                    log::error!("Failed to send commit hash response: {e}");
                                });
                            }
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to parse commit hash: {e}"))))
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
                Err(mpsc::TryRecvError::Disconnected) => {
                    PendingRequest::Response(Err(Arc::from("Request channel disconnected unexpectedly".to_string())))
                }
            },
            Some(PendingRequest::Response(ref result)) => {
                return result.clone().map(Some);
            }
        };
        self.remote_version_number = Some(new_state);
        Ok(None)
    }
    pub fn remote_update_bytes(&mut self, only_check: bool) -> PendingResponse<Arc<[u8]>> {
        let new_state = match &self.remote_update_bytes {
            None => {
                if only_check {
                    return Ok(None);
                }
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(
                    get_request(&format!("{PUBLIC_URL}/api/local/update")),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to fetch update: {e}"))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                                return;
                            }
                        };
                        if response.status != 200 {
                            sender
                                .send(Err(Arc::from(format!("Failed to fetch update: {}", response.status))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                        let file = response.bytes;
                        sender.send(Ok(file.into())).unwrap_or_else(|e| {
                            log::error!("Failed to send update file: {e}");
                        });
                    },
                );
                PendingRequest::Pending(receiver)
            }
            Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
                Ok(result) => PendingRequest::Response(result),
                Err(mpsc::TryRecvError::Empty) => {
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    PendingRequest::Response(Err(Arc::from("Request channel disconnected unexpectedly".to_string())))
                }
            },
            Some(PendingRequest::Response(ref result)) => {
                return result.clone().map(Some);
            }
        };
        self.remote_update_bytes = Some(new_state);
        Ok(None)
    }
    pub fn this_user(&mut self) -> PendingResponse<Arc<ThisUser>> {
        let new_state = match &self.this_user {
            None => {
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(get_request(&format!("{PUBLIC_URL}/api/user")), move |response| {
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
                            .send(Err(Arc::from(format!("Failed to fetch user: {}", response.status))))
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
                });
                PendingRequest::Pending(receiver)
            }
            Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
                Ok(result) => PendingRequest::Response(result),
                Err(mpsc::TryRecvError::Empty) => {
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    PendingRequest::Response(Err(Arc::from("Request channel disconnected unexpectedly".to_string())))
                }
            },
            Some(PendingRequest::Response(ref result)) => {
                return result.clone().map(Some);
            }
        };
        self.this_user = Some(new_state);
        Ok(None)
    }
    pub fn all_users(&mut self) -> PendingResponse<Vec<User>> {
        let new_state = match &self.users {
            None => {
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(get_request(&format!("{PUBLIC_URL}/api/users")), move |response| {
                    let response = match response {
                        Ok(resp) => resp,
                        Err(e) => {
                            sender
                                .send(Err(Arc::from(format!("Failed to fetch users: {e}"))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                    };
                    if response.status != 200 {
                        sender
                            .send(Err(Arc::from(format!("Failed to fetch users: {}", response.status))))
                            .unwrap_or_else(|e| {
                                log::error!("Failed to send error response: {e}");
                            });
                        return;
                    }
                    let users: Result<Vec<User>, _> = response.json();
                    match users {
                        Ok(users) => {
                            sender.send(Ok(users)).unwrap_or_else(|e| {
                                log::error!("Failed to send users response: {e}");
                            });
                        }
                        Err(e) => {
                            sender
                                .send(Err(Arc::from(format!("Failed to parse users: {e}"))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                        }
                    }
                });
                PendingRequest::Pending(receiver)
            }
            Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
                Ok(result) => PendingRequest::Response(result),
                Err(mpsc::TryRecvError::Empty) => {
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    PendingRequest::Response(Err(Arc::from("Request channel disconnected unexpectedly".to_string())))
                }
            },
            Some(PendingRequest::Response(ref result)) => {
                return result.clone().map(Some);
            }
        };
        self.users = Some(new_state);
        Ok(None)
    }
    pub fn get_repository(&mut self, uuid: Uuid) -> PendingResponse<Arc<RemoteRepository>> {
        match self.repositories.entry(uuid) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                let (sender, receiver) = mpsc::channel();
                ehttp::fetch(get_request(&format!("{PUBLIC_URL}/{uuid}")), move |response| {
                    let response = match response {
                        Ok(resp) => resp,
                        Err(e) => {
                            sender
                                .send(Err(Arc::from(format!("Failed to fetch repository: {e}"))))
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
                                .send(Err(Arc::from(format!("Failed to parse repository: {e}"))))
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
                            // let mut diff = remote.files.diff(&repo.folder);
                            let diff = Arc::from(repo.folder.diff(&remote.files));
                            let pitignore = match Pitignore::from_repository(repo.path.clone()) {
                                Ok(pitignore) => pitignore,
                                Err(e) => {
                                    log::error!("Failed to get .pitignore for repository: {e}");
                                    sender
                                        .send(Err(Arc::from(format!("Failed to get .pitignore for repository: {e}"))))
                                        .unwrap_or_else(|e| {
                                            log::error!("Failed to send error response: {e}");
                                        });
                                    return;
                                }
                            };
                            let local_pitignore_diff = pitignore.apply_patterns(&diff);
                            let remote_pitignore_diff = remote.pitignore.apply_patterns(&diff);
                            let local = Repository {
                                local: repo,
                                // remote: Arc::clone(&remote),
                                local_pitignore_diff,
                                remote_pitignore_diff,
                                local_pitignore: Arc::from(pitignore),
                                remote_pitignore: Arc::from(remote.pitignore.clone()),
                            };
                            // println!("Loaded stored repository: {local:#?}");
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
                                .send(Err(Arc::from(format!("Failed to get stored repository: {e}"))))
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
    pub fn reload_repository(&mut self, uuid: Uuid) -> Result<(), Arc<str>> {
        // if self.upload or self.download are specifically and only IN PROGRESS, we should not reload
        if self.sync_in_progress() {
            return Err(Arc::from(
                "Cannot reload repository while sync is in progress".to_string(),
            ));
        }
        self.repositories.remove(&uuid);
        self.stored_repositories.remove(&uuid);
        self.upload = None;
        self.download = None;
        Ok(())
    }
    pub fn reload_this_user(&mut self) {
        self.this_user = None;
    }
    pub fn upload_files(&mut self, repo: Arc<Repository>, button_text: String) -> PendingResponse<Uuid> {
        if self.download.is_some() {
            return Ok(None);
        }
        generic_sync_request(self.sync_in_progress(), &mut self.upload, repo, true, button_text)
    }
    pub fn download_files(&mut self, repo: Arc<Repository>, button_text: String) -> PendingResponse<Uuid> {
        if self.upload.is_some() {
            return Ok(None);
        }
        generic_sync_request(self.sync_in_progress(), &mut self.download, repo, false, button_text)
    }
    pub fn sync_in_progress(&self) -> bool {
        self.upload_in_progress() || self.download_in_progress()
    }
    pub fn upload_in_progress(&self) -> bool {
        self.upload.as_ref().is_some_and(|req| req.in_progress())
    }
    pub fn download_in_progress(&self) -> bool {
        self.download.as_ref().is_some_and(|req| req.in_progress())
    }
    pub fn any_sync_response(&mut self) -> PendingResponse<Uuid> {
        // check either upload or download for a response, treat as if its a repeat call
        if let Some(upload) = &mut self.upload {
            // usual matching and try_recv logic
            match upload {
                PendingRequest::Pending(ref pending) => match pending.try_recv() {
                    Ok(result) => {
                        *upload = PendingRequest::Response(result);
                        return Ok(None);
                    }
                    Err(mpsc::TryRecvError::Empty) => return Ok(None),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        *upload = PendingRequest::Response(Err(Arc::from(
                            "Request channel disconnected unexpectedly".to_string(),
                        )));
                        return Ok(None);
                    }
                },
                PendingRequest::Response(ref result) => {
                    return result.clone().map(Some);
                }
            }
        } else if let Some(download) = &mut self.download {
            match download {
                PendingRequest::Pending(ref pending) => match pending.try_recv() {
                    Ok(result) => {
                        *download = PendingRequest::Response(result);
                        return Ok(None);
                    }
                    Err(mpsc::TryRecvError::Empty) => return Ok(None),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        *download = PendingRequest::Response(Err(Arc::from(
                            "Request channel disconnected unexpectedly".to_string(),
                        )));
                        return Ok(None);
                    }
                },
                PendingRequest::Response(ref result) => {
                    return result.clone().map(Some);
                }
            }
        }
        // if neither upload nor download have a response, return Ok(None)
        Ok(None)
    }
    pub fn set_user_access_level(&mut self, repository_uuid: Uuid, user: UserWithAccess) {
        // ehttp::fetch(
        //     post_request(
        //         &format!("{PUBLIC_URL}/{repository_uuid}/.pit/user/access"),
        //         serde_json::to_value(user).expect("Failed to serialize user"),
        //     ),
        //     move |response| {
        //         let response = match response {
        //             Ok(resp) => resp,
        //             Err(e) => {
        //                 log::error!("Failed to set user access level: {e}");
        //                 return;
        //             }
        //         };
        //         if response.status != 200 {
        //             log::error!("Failed to set user access level: {}", response.status);
        //             return;
        //         }
        //         log::info!("User access level set successfully");
        //     },
        // );
        match &mut self.user_action {
            None => {
                let (sender, receiver) = mpsc::channel();
                // std::thread::spawn(move || {
                ehttp::fetch(
                    post_request(
                        &format!("{PUBLIC_URL}/{repository_uuid}/.pit/user/access"),
                        serde_json::to_value(user).expect("Failed to serialize user"),
                    ),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to set user access level: {e}"))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                                return;
                            }
                        };
                        if response.status != 200 {
                            sender
                                .send(Err(Arc::from(format!(
                                    "Failed to set user access level: {}",
                                    response.status
                                ))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                        sender.send(Ok(repository_uuid)).unwrap_or_else(|e| {
                            log::error!("Failed to send user access level set response: {e}");
                        });
                    },
                );
                // });
                self.user_action = Some(PendingRequest::Pending(receiver));
            }
            Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
                Ok(result) => {
                    self.user_action = Some(PendingRequest::Response(result));
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.user_action = Some(PendingRequest::Response(Err(Arc::from(
                        "Request channel disconnected unexpectedly".to_string(),
                    ))));
                }
            },
            Some(PendingRequest::Response(_)) => {}
        }
    }
    pub fn resolve_user_action(&mut self) -> PendingResponse<Uuid> {
        if let Some(user_action) = &mut self.user_action {
            match user_action {
                PendingRequest::Pending(ref pending) => match pending.try_recv() {
                    Ok(result) => {
                        *user_action = PendingRequest::Response(result);
                        return Ok(None);
                    }
                    Err(mpsc::TryRecvError::Empty) => return Ok(None),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        *user_action = PendingRequest::Response(Err(Arc::from(
                            "Request channel disconnected unexpectedly".to_string(),
                        )));
                        return Ok(None);
                    }
                },
                PendingRequest::Response(ref result) => {
                    return result.clone().map(Some);
                }
            }
        }
        Ok(None)
    }
    pub fn delete_user_access_level(&mut self, repository_uuid: Uuid, user_uuid: Uuid) -> PendingResponse<Uuid> {
        match self.user_action {
            None => {
                let (sender, receiver) = mpsc::channel();
                self.user_action = Some(PendingRequest::Pending(receiver));
                ehttp::fetch(
                    delete_request_with_body(
                        &format!("{PUBLIC_URL}/{repository_uuid}/.pit/user/access"),
                        serde_json::to_value(user_uuid).expect("Failed to serialize user UUID"),
                    ),
                    move |response| {
                        let response = match response {
                            Ok(resp) => resp,
                            Err(e) => {
                                sender
                                    .send(Err(Arc::from(format!("Failed to remove user access level: {e}"))))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send error response: {e}");
                                    });
                                return;
                            }
                        };
                        if response.status != 200 {
                            sender
                                .send(Err(Arc::from(format!(
                                    "Failed to remove user access level: {}",
                                    response.status
                                ))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                        sender.send(Ok(repository_uuid)).unwrap_or_else(|e| {
                            log::error!("Failed to send user access level removed response: {e}");
                        });
                    },
                );
            }
            Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
                Ok(result) => {
                    self.user_action = Some(PendingRequest::Response(result));
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(None),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.user_action = Some(PendingRequest::Response(Err(Arc::from(
                        "Request channel disconnected unexpectedly".to_string(),
                    ))));
                    return Ok(None);
                }
            },
            Some(PendingRequest::Response(ref result)) => {
                return result.clone().map(Some);
            }
        }
        Ok(None)
    }

    pub fn reset_user_action(&mut self) {
        self.user_action = None;
    }
}

fn generic_sync_request(
    either_is_some: bool,
    request_storage: &mut Option<PendingRequest<Uuid>>,
    repository: Arc<Repository>,
    upload: bool,
    button_text: String,
) -> PendingResponse<Uuid> {
    if either_is_some {
        return Ok(None);
    }
    *request_storage = None;
    let uuid = repository.local.uuid;
    match *request_storage {
        None => {
            let (sender, receiver) = mpsc::channel();
            *request_storage = Some(PendingRequest::Pending(receiver));
            std::thread::spawn(move || {
                match crate::dialogue::rfd_confirm_response(&button_text) {
                    Ok(true) => {}
                    Ok(false) => {
                        if let Err(send_error) = sender.send(Err(Arc::from("Sync cancelled".to_string()))) {
                            log::error!("Failed to send sync cancellation response: {send_error}");
                        }
                        return;
                    }
                    Err(e) => {
                        log::error!("Failed to show confirmation dialog: {e}");
                        if let Err(send_error) =
                            sender.send(Err(Arc::from(format!("Failed to show confirmation dialog: {e}"))))
                        {
                            log::error!("Failed to send error response: {send_error}");
                        }
                        return;
                    }
                }
                if let Err(e) = sync_request(repository, upload) {
                    log::error!("Failed to sync files: {e}");
                    if let Err(send_error) = sender.send(Err(e)) {
                        log::error!("Failed to send sync error response: {send_error}");
                    }
                } else if let Err(send_error) = sender.send(Ok(uuid)) {
                    log::error!("Failed to send sync completion response: {send_error}");
                }
            });
            Ok(None)
        }
        Some(PendingRequest::Pending(ref pending)) => match pending.try_recv() {
            Ok(result) => {
                *request_storage = Some(PendingRequest::Response(result));
                Ok(None)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                *request_storage = Some(PendingRequest::Response(Err(Arc::from(
                    "Request channel disconnected unexpectedly".to_string(),
                ))));
                Ok(None)
            }
        },
        Some(PendingRequest::Response(ref result)) => result.clone().map(Some),
    }
}

fn sync_request(repository: Arc<Repository>, upload: bool) -> Result<(), Arc<str>> {
    let mut actions = Vec::new();
    let diffs = if upload {
        repository.local_pitignore_diff.iter()
    } else {
        repository.remote_pitignore_diff.iter()
    };
    for diff in diffs {
        match diff.change_type {
            ChangeType::Modified => {
                if upload {
                    actions.push(SyncAction {
                        action_type: ActionType::Upload,
                        full_path: diff.full_path.clone(),
                    });
                } else {
                    actions.push(SyncAction {
                        action_type: ActionType::Download,
                        full_path: diff.full_path.clone(),
                    });
                }
            }
            ChangeType::OnServer => {
                if upload {
                    actions.push(SyncAction {
                        action_type: ActionType::DeleteFromRemote,
                        full_path: diff.full_path.clone(),
                    });
                } else {
                    actions.push(SyncAction {
                        action_type: ActionType::Download,
                        full_path: diff.full_path.clone(),
                    });
                }
            }
            ChangeType::OnClient => {
                if upload {
                    actions.push(SyncAction {
                        action_type: ActionType::Upload,
                        full_path: diff.full_path.clone(),
                    });
                } else {
                    actions.push(SyncAction {
                        action_type: ActionType::DeleteFromDisk,
                        full_path: diff.full_path.clone(),
                    });
                }
            }
        }
    }
    let mut pending_batched_uploads = Vec::new();
    let url_prefix = format!("{PUBLIC_URL}/{}", repository.local.uuid);
    for action in actions {
        let mut local_path = repository.local.path.clone();
        local_path.push(action.full_path.strip_prefix("/").unwrap_or(&*action.full_path));
        let remote_path = format!(
            "{url_prefix}/{}",
            action.full_path.strip_prefix("/").unwrap_or(&*action.full_path)
        );
        if action.action_type == ActionType::Download {
            std::fs::create_dir_all(local_path.parent().unwrap())
                .map_err(|e| Arc::from(format!("Failed to create directory: {e}")))?;
        }
        // Delete if necessary
        if (action.action_type == ActionType::DeleteFromDisk || action.action_type == ActionType::Download)
            && local_path.exists()
        {
            if let Err(e) = std::fs::remove_file(&local_path) {
                log::warn!("Failed to delete file {}: {e}", local_path.display());
            }
        }
        if action.action_type == ActionType::DeleteFromDisk {
            let mut o_parent = local_path.parent();
            while let Some(parent) = o_parent {
                // ensure parent is somewhere UNDER the repository path E.G C:/repo is the parent of C:/repo/some/dir, never delete anything OUTSIDE of the repository
                if !parent.starts_with(&repository.local.path) {
                    log::warn!(
                        "Skipping deletion of parent directory {} as it is outside the repository path",
                        parent.display()
                    );
                    break;
                } else if parent == repository.local.path {
                    // if parent is the repository path, we don't delete it
                    break;
                }
                // if parent empty, remove it
                if parent
                    .read_dir()
                    .map_err(|e| Arc::from(format!("Failed to read directory: {e}")))?
                    .next()
                    .is_none()
                {
                    if let Err(e) = std::fs::remove_dir(parent) {
                        log::warn!("Failed to remove empty directory {}: {e}", parent.display());
                    }
                    o_parent = parent.parent();
                } else {
                    o_parent = None;
                }
            }
        }
        let (await_sender, await_receiver) = mpsc::channel::<Result<(), Arc<str>>>();
        match action.action_type {
            ActionType::DeleteFromDisk => {
                await_sender.send(Ok(())).unwrap_or_else(|e| {
                    log::error!("Failed to send delete completion: {e}");
                });
            }
            ActionType::Upload => {
                // Add to pending uploads
                pending_batched_uploads.push(
                    UploadFile::new(
                        action.full_path.clone(),
                        std::fs::read(&local_path).map_err(|e| Arc::from(format!("Failed to read file: {e}")))?,
                    )
                    .map_err(|e| Arc::from(format!("Failed to create upload file: {e}")))?,
                );
                if pending_batched_uploads.iter().map(|f| f.size()).sum::<usize>() as f64
                    > (pitsu_lib::MAX_UPLOAD_SIZE as f64) * 0.50
                {
                    let mut new_uploads = Vec::new();
                    std::mem::swap(&mut pending_batched_uploads, &mut new_uploads);
                    if let Err(e) = upload_batched_files(&url_prefix, FileUpload { files: new_uploads }) {
                        log::error!("Failed to upload files: {e}");
                        return Err(e);
                    }
                }
                if let Err(e) = await_sender.send(Ok(())) {
                    log::error!("Failed to send upload completion: {e}");
                }
            }
            ActionType::DeleteFromRemote => {
                ehttp::fetch(delete_request(&remote_path), move |response| {
                    log::error!("Deleted file: {remote_path}");
                    let response = match response {
                        Ok(resp) => resp,
                        Err(e) => {
                            await_sender
                                .send(Err(Arc::from(format!("Failed to delete file: {e}"))))
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to send error response: {e}");
                                });
                            return;
                        }
                    };
                    if response.status != 200 {
                        await_sender
                            .send(Err(Arc::from(format!("Failed to delete file: {}", response.status))))
                            .unwrap_or_else(|e| {
                                log::error!("Failed to send error response: {e}");
                            });
                        return;
                    }
                    await_sender.send(Ok(())).unwrap_or_else(|e| {
                        log::error!("Failed to send delete completion: {e}");
                    });
                });
            }
            ActionType::Download => ehttp::fetch(get_request(&remote_path), move |response| {
                let response = match response {
                    Ok(resp) => resp,
                    Err(e) => {
                        await_sender
                            .send(Err(Arc::from(format!("Failed to download file: {e}"))))
                            .unwrap_or_else(|e| {
                                log::error!("Failed to send error response: {e}");
                            });
                        return;
                    }
                };
                if response.status != 200 {
                    await_sender
                        .send(Err(Arc::from(format!("Failed to download file: {}", response.status))))
                        .unwrap_or_else(|e| {
                            log::error!("Failed to send error response: {e}");
                        });
                    return;
                }
                if let Err(e) = std::fs::write(&local_path, response.bytes) {
                    await_sender
                        .send(Err(Arc::from(format!("Failed to write to {local_path:?}: {e}"))))
                        .unwrap_or_else(|e| {
                            log::error!("Failed to send error response: {e}");
                        });
                    return;
                }
                await_sender.send(Ok(())).unwrap_or_else(|e| {
                    log::error!("Failed to send download completion: {e}");
                });
            }),
        }
        match await_receiver.recv() {
            Ok(Ok(())) => {
                // Successfully completed the action
            }
            Ok(Err(e)) => {
                log::error!("Failed to complete action: {e}");
                return Err(e);
            }
            Err(_) => {
                log::error!("Action channel disconnected unexpectedly");
                return Err(Arc::from("Action channel disconnected unexpectedly".to_string()));
            }
        }
    }
    if !pending_batched_uploads.is_empty() {
        if let Err(e) = upload_batched_files(
            &url_prefix,
            FileUpload {
                files: pending_batched_uploads,
            },
        ) {
            log::error!("Failed to upload remaining files: {e}");
            return Err(e);
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SyncAction {
    action_type: ActionType,
    full_path: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionType {
    DeleteFromDisk,
    DeleteFromRemote,
    Upload,
    Download,
}

fn upload_batched_files(url_prefix: &str, files: FileUpload) -> Result<(), Arc<str>> {
    if files.files.is_empty() {
        return Ok(());
    }
    let (sender, receiver) = mpsc::channel();
    ehttp::fetch(
        post_request(
            &format!("{url_prefix}/.pit/upload"),
            serde_json::to_value(&files).expect("Failed to serialize files"),
        ),
        move |response| {
            let response = match response {
                Ok(resp) => resp,
                Err(e) => {
                    sender
                        .send(Err(Arc::from(format!("Failed to upload files: {e}"))))
                        .unwrap_or_else(|e| {
                            log::error!("Failed to send error response: {e}");
                        });
                    return;
                }
            };
            if response.status != 200 {
                sender
                    .send(Err(Arc::from(format!("Failed to upload files: {}", response.status))))
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send error response: {e}");
                    });
                return;
            }
            sender.send(Ok(())).unwrap_or_else(|e| {
                log::error!("Failed to send upload completion: {e}");
            });
        },
    );
    receiver
        .recv()
        .map_err(|_| Arc::from("Upload channel disconnected unexpectedly".to_string()))?
}
