mod colors;
mod config;
use std::sync::Arc;

use anyhow::Result;
use config::CONFIG;
use iced::widget::{button, column, text, Column};
use iced::Task;
use pitsu_lib::{Diff, RemoteRepository, ThisUser};
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
    Sync(Uuid, Arc<[Diff]>),
    Synced(Uuid, Arc<Result<()>>),
}

impl App {
    pub fn view(&self) -> Column<Message> {
        match &self.state {
            StateMachine::Startup(StartupState::Pending) => Self::view_loading("Loading..."),
            StateMachine::Startup(StartupState::Loading) => {
                Self::view_loading("Loading user data...")
            }
            StateMachine::Startup(StartupState::Errored(err)) => Self::view_error(err),
            StateMachine::MainWindow { login_data } => self.view_main_window(login_data),
            StateMachine::RepositoryDetails {
                login_data: _,
                repository_uuid: _,
                repository,
                repository_diff,
                sync_status,
            } => self.view_repository_details(repository, repository_diff, sync_status),
            #[allow(unreachable_patterns)]
            e => column![text(format!("Current state: {e:?}")).size(30)],
        }
    }

    fn view_loading(msg: &str) -> Column<Message> {
        column![text(msg).size(30)]
    }

    fn view_error(err: &Arc<str>) -> Column<'static, Message> {
        column![text(format!("Error: {err}"))
            .color(*colors::ERROR_COLOR)
            .size(30)]
    }

    fn view_main_window(&self, login_data: &Arc<ThisUser>) -> Column<Message> {
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
        col
    }

    fn view_repository_details(
        &self,
        repository: &Pending<(Arc<RemoteRepository>, Option<Arc<StoredRepository>>)>, // Fixed generics syntax here
        repository_diff: &Pending<Arc<[Diff]>>,
        sync_status: &Pending<()>,
    ) -> Column<'static, Message> {
        match repository {
            Pending::Unsent => Self::view_loading("Fetching repository data..."),
            Pending::InProgress => Self::view_loading("Repository data is being fetched..."),
            Pending::Ready((repo, stored_repo)) => {
                let repo = repo.as_ref();
                // Add Back button at the top
                let mut col = column![
                    button(text("Back").size(20)).on_press(Message::ChangeState(
                        StateMachine::MainWindow {
                            login_data: self.get_login_data()
                        }
                    )),
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
                col = self.view_repository_diff(col, repo.uuid, repository_diff, sync_status);
                col
            }
            Pending::Errored(err) => Self::view_error(err),
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

    fn view_repository_diff(
        &self,
        mut col: Column<'static, Message>,
        repo_uuid: Uuid,
        repository_diff: &Pending<Arc<[Diff]>>,
        sync_status: &Pending<()>,
    ) -> Column<'static, Message> {
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
                        Pending::Unsent => col.push(
                            button(text("Sync Changes").size(30))
                                .on_press(Message::Sync(repo_uuid, diff.clone())),
                        ),
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
            Message::Sync(repository_uuid, diffs) => {
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
                            sync_diffs(client, repository_uuid, diffs),
                            move |result| Message::Synced(repository_uuid, Arc::new(result)),
                        );
                    }
                }
            }
            Message::Synced(repository_uuid, result) => {
                if let StateMachine::RepositoryDetails {
                    login_data: _,
                    repository_uuid: _,
                    repository: Pending::Ready((repo, _stored)),
                    repository_diff,
                    sync_status,
                } = &mut self.state
                {
                    if repo.uuid == repository_uuid {
                        *sync_status = match &*result {
                            Ok(_) => Pending::Ready(()),
                            Err(err) => Pending::Errored(err.to_string().into()),
                        };

                        *repository_diff = Pending::Ready(vec![].into());
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

async fn sync_diffs(client: Client, repository_uuid: Uuid, diffs: Arc<[Diff]>) -> Result<()> {
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

fn panic_hook(info: &std::panic::PanicHookInfo) {
    rfd::MessageDialog::new()
        .set_title("PITSU Panic")
        .set_description(info.to_string())
        .show();
}
