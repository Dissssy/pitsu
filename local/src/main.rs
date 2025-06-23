mod colors;
mod config;
use std::sync::Arc;

use anyhow::Result;
use config::CONFIG;
use iced::widget::{button, column, row, scrollable, text, text_editor, Column};
use iced::{Element, Task};
use pitsu_lib::{
    AccessLevel, CreateRemoteRepository, Diff, FileUpload, RemoteRepository, ThisUser, UploadFile,
};
use reqwest::Client;
use uuid::Uuid;

use crate::config::StoredRepository;

fn main() -> iced::Result {
    setup();

    iced::application(Title, App::update, App::view)
        .run_with(|| (App::default(), Task::done(Message::Start)))
}

struct Title;

impl iced::application::Title<App> for Title {
    fn title(&self, _state: &App) -> String {
        format!("PITSU - <{}>", CONFIG.username())
    }
}

fn setup() {
    std::panic::set_hook(Box::new(panic_hook));
    if CONFIG.api_key().is_empty() {
        log::error!("PITSU_API_KEY is not set. Please set it in your environment variables.");
        panic!("PITSU_API_KEY is not set. Please set it in your environment variables.");
    }
    if CONFIG.username().is_empty() {
        log::error!("PITSU_API_USERNAME is not set. Please set it in your environment variables.");
        panic!("PITSU_API_USERNAME is not set. Please set it in your environment variables.");
    }
    if CONFIG.public_url().is_empty() {
        log::warn!("PITSU_PUBLIC_URL is not set. Please set it in your environment variables.");
        panic!("PITSU_PUBLIC_URL is not set. Please set it in your environment variables.");
    }

    std::env::set_var("SEQ_API_KEY", env!("LOCAL_SEQ_API_KEY"));
    datalust_logger::init(&format!("PITSU <{}>", CONFIG.uuid()))
        .expect("Failed to initialize logger");
}

#[derive(Default)]
struct App {
    state: StateMachine,
    client: Client,
    text_editor: text_editor::Content,
}

#[derive(Debug, Clone)]
enum Message {
    Start,
    ChangeState(StateMachine),
    RepositoryReady(
        Uuid,
        Arc<Result<Arc<RemoteRepository>>>,
        Arc<Result<Option<Arc<StoredRepository>>>>,
    ),
    SelectFolder(Uuid),
    StoredRepositoryReady(Uuid, Arc<Result<Arc<StoredRepository>>>),
    SyncDown(Uuid, Arc<[Diff]>),
    SyncUp(Uuid, Arc<[Diff]>),
    Synced(Uuid, Arc<Result<()>>),
    Edit(text_editor::Action),
}

impl App {
    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        match &self.state {
            StateMachine::Startup(StartupState::Pending) => Self::view_loading("Loading..."),
            StateMachine::Startup(StartupState::Loading) => {
                Self::view_loading("Loading user data...")
            }
            StateMachine::Startup(StartupState::Errored(err)) => Self::view_error(err.clone()),
            StateMachine::MainWindow { login_data } => self.view_main_window(login_data.clone()),
            StateMachine::RepositoryDetails {
                login_data: _,
                repository_uuid: _,
                repository,
                repository_diff,
                sync_status,
            } => self.view_repository_details(repository, repository_diff, sync_status),
            StateMachine::CreateRepository { login_data, upload } => match upload {
                Pending::Unsent => column![
                    text("Create New Repository").size(30),
                    text(format!("Logged in as: {}", login_data.user.username)).size(20),
                    text("Repository Name:").size(20),
                    text_editor(&self.text_editor)
                        .placeholder("Enter repository name")
                        .on_action(Message::Edit)
                        .size(20),
                    button(text("Create").size(20)).on_press(Message::Edit(
                        text_editor::Action::Edit(text_editor::Edit::Enter),
                    )),
                ]
                .into(),
                Pending::InProgress => Self::view_loading("Creating repository..."),
                Pending::Ready(_) => {
                    column![text("Repository created successfully!").size(30)].into()
                }
                Pending::Errored(err) => Self::view_error(err.clone()),
            },
            #[allow(unreachable_patterns)]
            e => column![text(format!("Current state: {e:?}")).size(30)].into(),
        }
    }

    fn view_loading<'a>(msg: &'a str) -> Element<'a, Message> {
        column![text(msg).size(30)].into()
    }

    fn view_error<'a>(err: Arc<str>) -> Element<'a, Message> {
        column![text(format!("Error: {err}"))
            .color(*colors::ERROR_COLOR)
            .size(30)]
        .into()
    }

    fn view_main_window<'a>(&'a self, login_data: Arc<ThisUser>) -> Element<'a, Message> {
        let mut col = Column::with_children(vec![text(format!(
            "Welcome, {}!",
            login_data.user.username
        ))
        .size(30)
        .into()]);
        for repository in &login_data.owned_repositories {
            col = col.push(
                button(text(format!("Repository (Owned): {}", repository.name))).on_press(
                    Message::ChangeState(StateMachine::RepositoryDetails {
                        login_data: login_data.clone(),
                        repository_uuid: repository.uuid,
                        repository: Pending::Unsent,
                        repository_diff: Pending::Unsent,
                        sync_status: Pending::Unsent,
                    }),
                ),
            );
        }
        for (repository, permission_level) in &login_data.accessible_repositories {
            col = col.push(
                button(text(format!(
                    "Repository ({}): {}",
                    permission_level, repository.name
                )))
                .on_press(Message::ChangeState(
                    StateMachine::RepositoryDetails {
                        login_data: login_data.clone(),
                        repository_uuid: repository.uuid,
                        repository: Pending::Unsent,
                        repository_diff: Pending::Unsent,
                        sync_status: Pending::Unsent,
                    },
                )),
            );
        }
        col = col.push(
            button(text("Create New Repository")).on_press(Message::ChangeState(
                StateMachine::CreateRepository {
                    login_data: login_data.clone(),
                    upload: Pending::Unsent,
                },
            )),
        );
        col.into()
    }

    fn view_repository_details<'a>(
        &'a self,
        repository: &'a Pending<(Arc<RemoteRepository>, Option<Arc<StoredRepository>>)>, // Fixed generics syntax here
        repository_diff: &'a Pending<Arc<[Diff]>>,
        sync_status: &'a Pending<()>,
    ) -> Element<'a, Message> {
        match repository {
            Pending::Unsent => Self::view_loading("Fetching repository data..."),
            Pending::InProgress => Self::view_loading("Repository data is being fetched..."),
            Pending::Ready((repo, stored_repo)) => {
                let repo = repo.as_ref();
                // Add Back button at the top
                let mut col = column![
                    row![
                        button(text("Back").size(20)).on_press(Message::ChangeState(
                            StateMachine::MainWindow {
                                login_data: self.get_login_data()
                            }
                        )),
                        button(text("Refresh").size(20)).on_press(Message::ChangeState(
                            StateMachine::RepositoryDetails {
                                login_data: self.get_login_data(),
                                repository_uuid: repo.uuid,
                                repository: Pending::Unsent,
                                repository_diff: Pending::Unsent,
                                sync_status: Pending::Unsent,
                            }
                        )),
                    ]
                    .spacing(10),
                    text(format!("Repository Details: {}", repo.name)).size(30),
                    text(format!("UUID: {}", repo.uuid)).size(20),
                    text(format!("Files: {}", repo.files.file_count())).size(20),
                    text(format!("Size: {}", readable_size(repo.files.size()))).size(20),
                ];
                col = match stored_repo {
                    Some(stored_repo) => col.push(
                        text(format!(
                            "Stored Repository: {}",
                            stored_repo.path.to_string_lossy()
                        ))
                        .size(30),
                    ),
                    None => col.push(
                        button(text("No stored repository found").size(30))
                            .on_press(Message::SelectFolder(repo.uuid)),
                    ),
                };
                col = self.view_repository_diff(
                    col,
                    repo.uuid,
                    repo.access_level,
                    repository_diff,
                    sync_status,
                );
                scrollable::Scrollable::new(col).into()
            }
            Pending::Errored(err) => Self::view_error(err.clone()),
        }
    }

    // Helper to get login_data for back navigation
    fn get_login_data(&self) -> Arc<ThisUser> {
        match &self.state {
            StateMachine::RepositoryDetails { login_data, .. } => login_data.clone(),
            StateMachine::MainWindow { login_data } => login_data.clone(),
            _ => panic!("Cannot get login data from current state"),
        }
    }

    fn view_repository_diff<'a>(
        &'a self,
        mut col: Column<'a, Message>,
        repo_uuid: Uuid,
        repo_access_level: AccessLevel,
        repository_diff: &'a Pending<Arc<[Diff]>>,
        sync_status: &'a Pending<()>,
    ) -> Column<'a, Message> {
        match repository_diff {
            Pending::Unsent => col.push(text("Repository diff is not yet fetched.").size(30)),
            Pending::InProgress => col.push(text("Repository diff is being fetched...").size(30)),
            Pending::Ready(diff) => {
                for diff in diff.iter() {
                    col = col.push(
                        text(format!(
                            "{} - {}",
                            match diff.change_type {
                                pitsu_lib::ChangeType::Added => "ADDED",
                                pitsu_lib::ChangeType::Modified => "MODIFIED",
                                pitsu_lib::ChangeType::Removed => "REMOVED",
                            },
                            diff.full_path,
                        ))
                        .color(match diff.change_type {
                            pitsu_lib::ChangeType::Added => *colors::ADDED_COLOR,
                            pitsu_lib::ChangeType::Modified => *colors::MODIFIED_COLOR,
                            pitsu_lib::ChangeType::Removed => *colors::REMOVED_COLOR,
                        })
                        .size(20),
                    );
                }
                if diff.is_empty() {
                    col.push(text("No changes detected.").size(30))
                } else {
                    match sync_status {
                        Pending::Unsent => {
                            col = col.push(
                                button(text("Sync Down").size(30))
                                    .on_press(Message::SyncDown(repo_uuid, diff.clone())),
                            );
                            if repo_access_level >= AccessLevel::Write {
                                col = col.push(
                                    button(text("Sync Up").size(30))
                                        .on_press(Message::SyncUp(repo_uuid, diff.clone())),
                                );
                            }
                            col
                        }
                        Pending::InProgress => col.push(text("Syncing changes...").size(30)),
                        Pending::Ready(_) => {
                            col.push(text("Changes synced successfully.").size(30))
                        }
                        Pending::Errored(err) => col.push(
                            text(format!("Error syncing changes: {err}"))
                                .color(*colors::ERROR_COLOR)
                                .size(30),
                        ),
                    }
                }
            }
            Pending::Errored(err) => {
                col.push(text(format!("Error fetching repository diff: {err}")).size(30))
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Start => {}
            Message::ChangeState(new_state) => {
                self.state = new_state;
            }
            Message::Edit(action) => {
                if let text_editor::Action::Edit(text_editor::Edit::Enter) = &action {
                    let string = self.text_editor.text();
                    if let StateMachine::CreateRepository { login_data, upload } = &mut self.state {
                        *upload = Pending::InProgress;
                        let login_data = login_data.clone();
                        let client = self.client.clone();
                        return Task::perform(
                            create_repository(client, string.into()),
                            move |result| match result {
                                Ok(login_data) => {
                                    Message::ChangeState(StateMachine::MainWindow { login_data })
                                }
                                Err(err) => Message::ChangeState(StateMachine::CreateRepository {
                                    login_data: login_data.clone(),
                                    upload: Pending::Errored(err.to_string().into()),
                                }),
                            },
                        );
                    }
                } else {
                    self.text_editor.perform(action);
                }
            }
            Message::RepositoryReady(repository_uuid, repository, stored_repository) => {
                if let StateMachine::RepositoryDetails {
                    login_data,
                    repository_uuid: uuid,
                    repository: _,
                    repository_diff,
                    sync_status: _,
                } = &mut self.state
                {
                    if *uuid == repository_uuid {
                        match &*repository {
                            Ok(repo) => {
                                let stored_repo = match &*stored_repository {
                                    Ok(Some(stored_repo)) => {
                                        let diff = stored_repo
                                            .folder
                                            .as_ref()
                                            .expect("Stored repository should have a folder")
                                            .diff(&repo.files);
                                        *repository_diff = Pending::Ready(diff.into());
                                        Some(stored_repo.clone())
                                    }
                                    Ok(None) => None,
                                    Err(err) => {
                                        log::error!("Failed to fetch stored repository for {repository_uuid}: {err}");
                                        None
                                    }
                                };
                                self.state = StateMachine::RepositoryDetails {
                                    login_data: login_data.clone(),
                                    repository_uuid: *uuid,
                                    repository: Pending::Ready((repo.clone(), stored_repo)),
                                    repository_diff: repository_diff.clone(),
                                    sync_status: Pending::Unsent,
                                };
                            }
                            Err(err) => {
                                log::error!("Failed to fetch repository {repository_uuid}: {err}");
                                self.state = StateMachine::RepositoryDetails {
                                    login_data: login_data.clone(),
                                    repository_uuid: *uuid,
                                    repository: Pending::Errored(err.to_string().into()),
                                    repository_diff: Pending::Unsent,
                                    sync_status: Pending::Unsent,
                                };
                            }
                        }
                    }
                }
            }
            Message::SelectFolder(repository_uuid) => {
                return Task::perform(select_folder(repository_uuid), move |result| {
                    Message::StoredRepositoryReady(repository_uuid, Arc::new(result))
                });
            }
            Message::StoredRepositoryReady(repository_uuid, stored_repo) => {
                if let StateMachine::RepositoryDetails {
                    login_data: _,
                    repository_uuid: _,
                    repository: Pending::Ready((repo, stored)),
                    repository_diff,
                    sync_status: _,
                } = &mut self.state
                {
                    if repo.uuid == repository_uuid {
                        *stored = match &*stored_repo {
                            Ok(stored_repo) => {
                                let diff = stored_repo
                                    .folder
                                    .as_ref()
                                    .expect("Stored repository should have a folder")
                                    .diff(&repo.files);

                                *repository_diff = Pending::Ready(diff.into());

                                Some(stored_repo.clone())
                            }
                            Err(err) => {
                                log::error!(
                                    "Failed to select folder for repository {repository_uuid}: {err}"
                                );
                                None
                            }
                        };
                    }
                }
            }
            Message::SyncDown(repository_uuid, diffs) => {
                if let StateMachine::RepositoryDetails {
                    login_data: _,
                    repository_uuid: _,
                    repository: Pending::Ready((repo, _stored)),
                    repository_diff: _,
                    sync_status,
                } = &mut self.state
                {
                    if repo.uuid == repository_uuid {
                        *sync_status = Pending::InProgress;
                        let client = self.client.clone();
                        return Task::perform(
                            sync_diffs_down(client, repository_uuid, diffs),
                            move |result| Message::Synced(repository_uuid, Arc::new(result)),
                        );
                    }
                }
            }
            Message::SyncUp(repository_uuid, diffs) => {
                if let StateMachine::RepositoryDetails {
                    login_data: _,
                    repository_uuid: _,
                    repository: Pending::Ready((repo, _stored)),
                    repository_diff: _,
                    sync_status,
                } = &mut self.state
                {
                    if repo.uuid == repository_uuid {
                        *sync_status = Pending::InProgress;
                        let client = self.client.clone();
                        return Task::perform(
                            sync_diffs_up(client, repository_uuid, diffs),
                            move |result| Message::Synced(repository_uuid, Arc::new(result)),
                        );
                    }
                }
            }
            Message::Synced(sync_uuid, result) => {
                if let StateMachine::RepositoryDetails {
                    login_data: _,
                    repository_uuid,
                    repository,
                    repository_diff,
                    sync_status,
                } = &mut self.state
                {
                    if repository.ready() && sync_uuid == *repository_uuid {
                        *sync_status = match &*result {
                            Ok(_) => Pending::Ready(()),
                            Err(err) => Pending::Errored(err.to_string().into()),
                        };

                        *repository_diff = Pending::Ready(vec![].into());
                        *repository = Pending::InProgress;
                        let client = self.client.clone();
                        let repository_uuid = *repository_uuid;
                        return Task::perform(
                            fetch_repository(client, repository_uuid),
                            move |result| {
                                let stored_repo = CONFIG.get_stored(repository_uuid);
                                Message::RepositoryReady(
                                    repository_uuid,
                                    Arc::new(result.map(Arc::new)),
                                    Arc::new(stored_repo),
                                )
                            },
                        );
                    }
                }
            }
        }
        match &mut self.state {
            StateMachine::Startup(StartupState::Pending) => {
                self.state = StateMachine::Startup(StartupState::Loading);

                let client = self.client.clone();
                Task::perform(fetch_user(client), |result| match result {
                    Ok(login_data) => Message::ChangeState(StateMachine::MainWindow {
                        login_data: Arc::new(login_data),
                    }),
                    Err(err) => Message::ChangeState(StateMachine::Startup(StartupState::Errored(
                        err.to_string().into(),
                    ))),
                })
            }
            StateMachine::RepositoryDetails {
                login_data: _,
                repository_uuid: _,
                repository,
                repository_diff: _,
                sync_status: _,
            } if *repository == Pending::Unsent => {
                let client = self.client.clone();
                *repository = Pending::InProgress;
                let repository_uuid = match &self.state {
                    StateMachine::RepositoryDetails {
                        repository_uuid, ..
                    } => *repository_uuid,
                    _ => unreachable!(),
                };
                Task::perform(fetch_repository(client, repository_uuid), move |result| {
                    let stored_repo = CONFIG.get_stored(repository_uuid);
                    Message::RepositoryReady(
                        repository_uuid,
                        Arc::new(result.map(Arc::new)),
                        Arc::new(stored_repo),
                    )
                })
            }
            _ => Task::none(),
        }
    }
}

#[derive(Debug, Clone)]
enum StateMachine {
    Startup(StartupState),
    MainWindow {
        login_data: Arc<ThisUser>,
    },
    CreateRepository {
        login_data: Arc<ThisUser>,
        upload: Pending<()>,
    },
    RepositoryDetails {
        login_data: Arc<ThisUser>,
        repository_uuid: Uuid,
        repository: Pending<(Arc<RemoteRepository>, Option<Arc<StoredRepository>>)>,
        repository_diff: Pending<Arc<[Diff]>>,
        sync_status: Pending<()>,
    },
}

impl Default for StateMachine {
    fn default() -> Self {
        StateMachine::Startup(StartupState::Pending)
    }
}

#[derive(Debug, Clone)]
enum StartupState {
    Pending,
    Loading,
    Errored(Arc<str>),
}

#[derive(Debug, Clone)]
enum Pending<T> {
    Unsent,
    InProgress,
    Ready(T),
    Errored(Arc<str>),
}

impl<T> Pending<T> {
    pub fn ready(&self) -> bool {
        matches!(self, Pending::Ready(_))
    }
}

impl<T> PartialEq for Pending<T> {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Pending::Unsent, Pending::Unsent)
                | (Pending::InProgress, Pending::InProgress)
                | (Pending::Ready(_), Pending::Ready(_))
                | (Pending::Errored(_), Pending::Errored(_))
        )
    }
}

impl<T> Eq for Pending<T> {}

async fn fetch_user(client: Client) -> Result<ThisUser> {
    let response = client
        .get(format!("{}/api/user", CONFIG.public_url()))
        .header("Authorization", format!("Bearer {}", CONFIG.api_key()))
        .send()
        .await?;

    if response.status().is_success() {
        let user = response.json().await?;
        Ok(user)
    } else {
        Err(anyhow::anyhow!("Failed to fetch user"))
    }
}

async fn fetch_repository(client: Client, repository_uuid: Uuid) -> Result<RemoteRepository> {
    let response = client
        .get(format!("{}/{}", CONFIG.public_url(), repository_uuid))
        .header("Authorization", format!("Bearer {}", CONFIG.api_key()))
        .send()
        .await?;

    if response.status().is_success() {
        let repository = response.json().await?;
        Ok(repository)
    } else {
        Err(anyhow::anyhow!("Failed to fetch repository"))
    }
}

const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

fn readable_size(size: u64) -> String {
    let mut size = size;
    let mut unit_index = 0;

    while size >= 1024 && unit_index < UNITS.len() - 1 {
        size /= 1024;
        unit_index += 1;
    }

    format!("{} {}", size, UNITS[unit_index])
}

async fn select_folder(repository_uuid: Uuid) -> Result<Arc<StoredRepository>> {
    let path = rfd::FileDialog::new()
        .set_title("Select Repository Folder")
        .pick_folder()
        .ok_or_else(|| anyhow::anyhow!("No folder selected"))?;
    let stored_repo = CONFIG
        .add_stored(repository_uuid, path)
        .map_err(|e| anyhow::anyhow!("Failed to add stored repository: {}", e))?;
    Ok(stored_repo)
}

async fn sync_diffs_down(client: Client, repository_uuid: Uuid, diffs: Arc<[Diff]>) -> Result<()> {
    let repository_path = CONFIG
        .get_stored(repository_uuid)?
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No stored repository found for UUID: {}", repository_uuid))?
        .path
        .clone();
    for diff in diffs.iter() {
        let full_path = match diff.full_path.strip_prefix('/') {
            Some(stripped) => stripped,
            None => &diff.full_path,
        };
        let local_path = repository_path.join(full_path);
        match diff.change_type {
            pitsu_lib::ChangeType::Added | pitsu_lib::ChangeType::Modified => {
                if let Some(parent) = local_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let response = client
                    .get(format!(
                        "{}/{}/{}",
                        CONFIG.public_url(),
                        repository_uuid,
                        full_path
                    ))
                    .header("Authorization", format!("Bearer {}", CONFIG.api_key()))
                    .send()
                    .await?;
                if response.status().is_success() {
                    let content = response.bytes().await?;
                    std::fs::write(&local_path, content)?;
                } else {
                    return Err(anyhow::anyhow!(
                        "Failed to fetch file {}: {}",
                        full_path,
                        response.status()
                    ));
                }
            }
            pitsu_lib::ChangeType::Removed => {
                if local_path.exists() {
                    std::fs::remove_file(&local_path)?;
                }
            }
        }
    }
    Ok(())
}

async fn sync_diffs_up(client: Client, repository_uuid: Uuid, diffs: Arc<[Diff]>) -> Result<()> {
    let repository_path = CONFIG
        .get_stored(repository_uuid)?
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No stored repository found for UUID: {}", repository_uuid))?
        .path
        .clone();
    let mut pending_uploads = Vec::new();
    let mut pending_upload_size = 0;
    for diff in diffs.iter() {
        let full_path = match diff.full_path.strip_prefix('/') {
            Some(stripped) => stripped,
            None => &diff.full_path,
        };
        let local_path = repository_path.join(full_path);
        match diff.change_type {
            pitsu_lib::ChangeType::Removed | pitsu_lib::ChangeType::Modified => {
                // File is missing from server or is different, upload it
                let file_data = std::fs::read(&local_path).map_err(|e| {
                    anyhow::anyhow!("Failed to read file {}: {}", local_path.display(), e)
                })?;
                // let url = format!("{}/{}/{}", CONFIG.public_url(), repository_uuid, full_path);
                // upload_file(&client, &file_data, &url).await?;
                let file = UploadFile::new(diff.full_path.clone(), file_data)?;
                let file_size = file.size();
                if ((pending_upload_size + file_size) as f32)
                    < ((pitsu_lib::MAX_UPLOAD_SIZE as f32) * 0.1)
                {
                    pending_upload_size += file.size();
                    pending_uploads.push(file);
                    if pending_upload_size as f32 > ((pitsu_lib::MAX_UPLOAD_SIZE as f32) * 0.05) {
                        let mut uploads = Vec::new();
                        std::mem::swap(&mut uploads, &mut pending_uploads);
                        upload_files(&client, uploads, repository_uuid).await?;
                        pending_upload_size = 0;
                    }
                } else {
                    let mut uploads = vec![file];
                    std::mem::swap(&mut uploads, &mut pending_uploads);
                    upload_files(&client, uploads, repository_uuid).await?;
                    pending_upload_size = file_size;
                }
            }
            pitsu_lib::ChangeType::Added => {
                // File on server exists but is not in local repository, delete the remote file
                let url = format!("{}/{}/{}", CONFIG.public_url(), repository_uuid, full_path);
                delete_file(&client, &url).await?;
            }
        }
    }
    if !pending_uploads.is_empty() {
        upload_files(&client, pending_uploads, repository_uuid).await?;
    }
    Ok(())
}

fn panic_hook(info: &std::panic::PanicHookInfo) {
    rfd::MessageDialog::new()
        .set_title("PITSU Panic")
        .set_description(info.to_string())
        .show();
}

// curl -X POST -H "Authorization Bearer <token>" -F "file=@/path/to/file" https://pit.p51.nl/{uuid}/{path}
async fn upload_files(client: &Client, files: Vec<UploadFile>, uuid: Uuid) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    log::info!("Uploading {} files to repository {}", files.len(), uuid);
    for file in &files {
        log::debug!(
            "Preparing to upload file: {} ({} bytes)",
            file.path,
            file.size()
        );
    }
    let response = client
        .post(format!("{}/{}/.pit/upload", CONFIG.public_url(), uuid))
        .header("Authorization", format!("Bearer {}", CONFIG.api_key()))
        // .multipart(
        //     reqwest::multipart::Form::new()
        //         // .part("file", reqwest::multipart::Part::bytes(file_bytes.to_vec())),
        //         .file("file", file_path),
        // )
        .json(&FileUpload { files })
        .send()
        .await?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| "No response text".to_string());
        log::error!("Failed to upload files: {status} - {text}");
        Err(anyhow::anyhow!(
            "Failed to upload file: {} - {}",
            status,
            text
        ))
    }
}

// curl -X DELETE -H "Authorization Bearer <token>" https://pit.p51.nl/{uuid}/{path}
async fn delete_file(client: &Client, url: &str) -> Result<()> {
    let response = client
        .delete(url)
        .header("Authorization", format!("Bearer {}", CONFIG.api_key()))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to delete file: {}",
            response.status()
        ))
    }
}

async fn create_repository(client: Client, name: Arc<str>) -> Result<Arc<ThisUser>> {
    let response = client
        .post(format!("{}/api/repository", CONFIG.public_url()))
        .header("Authorization", format!("Bearer {}", CONFIG.api_key()))
        .json(&CreateRemoteRepository { name })
        .send()
        .await?;

    if response.status().is_success() {
        // get updated user data
        let user = fetch_user(client).await?;
        Ok(Arc::new(user))
    } else {
        Err(anyhow::anyhow!("Failed to create repository"))
    }
}
