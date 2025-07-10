#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
};

use eframe::egui::{self, FontData};
use pitsu_lib::{AccessLevel, ChangeType, Diff, RemoteRepository, ThisUser};
use uuid::Uuid;

use crate::{
    config::{get_request, LocalRepository, CONFIG, MAX_PATH_LENGTH, PUBLIC_URL},
    pitignore::Pitignore,
};
mod config;
mod dialogue;
mod nerdfonts;
mod pitignore;

fn main() -> anyhow::Result<()> {
    config::setup();
    // if the program is run with the --update flag, we will update the application at this point. we need to get the old file location which will be "pitsu.exe" in the folder where the executable is located.
    if std::env::args().any(|arg| arg == "--update") {
        let this_exe = std::env::current_exe().expect("Failed to get current executable path");
        let new_exe = this_exe.with_file_name("pitsu.exe");
        let (update_sender, update_receiver) = mpsc::channel::<Result<Arc<[u8]>, Arc<str>>>();
        ehttp::fetch(
            get_request(&format!("{PUBLIC_URL}/api/local/update")),
            move |response| {
                let response = match response {
                    Ok(resp) => resp,
                    Err(e) => {
                        update_sender
                            .send(Err(Arc::from(format!("Failed to fetch update: {e}"))))
                            .unwrap_or_else(|e| {
                                log::error!("Failed to send error response: {e}");
                            });
                        return;
                    }
                };
                if response.status != 200 {
                    update_sender
                        .send(Err(Arc::from(format!(
                            "Failed to fetch update: {}",
                            response.status
                        ))))
                        .unwrap_or_else(|e| {
                            log::error!("Failed to send error response: {e}");
                        });
                    return;
                }
                let file = response.bytes;
                update_sender.send(Ok(file.into())).unwrap_or_else(|e| {
                    log::error!("Failed to send update file: {e}");
                });
            },
        );
        let update = update_receiver
            .recv()
            .expect("Failed to receive update file");
        let update = match update {
            Ok(file) => file,
            Err(e) => {
                log::error!("Failed to fetch update: {e}");
                return Err(anyhow::anyhow!("Failed to fetch update: {e}"));
            }
        };
        // Delete the "old" pitsu.exe if it exists
        if new_exe.exists() {
            std::fs::remove_file(&new_exe).expect("Failed to remove old pitsu.exe");
        }
        // Write the update to pitsu.exe
        std::fs::write(&new_exe, &*update).expect("Failed to write update file to pitsu.exe");
        // run pitsu.exe
        #[allow(clippy::zombie_processes)]
        std::process::Command::new(new_exe)
            .spawn()
            .expect("Failed to spawn new Pitsu process");
        return Ok(());
    } else {
        // If this is not an update, delete the temporary update file if it exists
        let temp_update_file = std::env::current_exe()
            .expect("Failed to get current executable path")
            .with_file_name("pitsu_old.exe");
        if temp_update_file.exists() {
            std::fs::remove_file(temp_update_file).expect("Failed to remove temporary update file");
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder {
            icon: Some(Arc::clone(&config::icons::WINDOW_ICON)),
            ..Default::default()
        },
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "Pitsu",
        native_options,
        Box::new(move |cc| {
            let ppp = cc
                .storage
                .and_then(|storage| {
                    storage
                        .get_string("pixels_per_point")
                        .and_then(|s| s.parse::<f32>().ok())
                })
                .unwrap_or(2.0);
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "nerdfonts".into(),
                Arc::new({
                    let mut data =
                        FontData::from_static(include_bytes!("../assets/nerdfonts_regular.ttf"));
                    data.tweak.y_offset_factor = 0.0;
                    data
                }),
            );

            if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                font_keys.push("nerdfonts".into());
            }
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(App::new(ppp)))
        }),
    ) {
        log::error!("Failed to run Pitsu: {e}");
        return Err(anyhow::anyhow!("Failed to run Pitsu: {e}"));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Repository {
    local: Arc<LocalRepository>,
    // remote: Arc<RemoteRepository>,
    diff: Arc<[Diff]>,
    pitignore: Arc<pitignore::Pitignore>,
}

pub struct App {
    cache: RequestCache,
    ppp: f32,
    state: AppState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Main,
    RepositoryDetails { uuid: Uuid },
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut new_state = self.header(ui, ctx, frame);
            match self.state {
                AppState::Main => {
                    if let Ok(Some(this)) = self.cache.this_user() {
                        let table = egui_extras::TableBuilder::new(ui)
                            .striped(true)
                            .resizable(false)
                            .column(egui_extras::Column::auto())
                            .column(egui_extras::Column::auto())
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.add(
                                        egui::Label::new("Repository")
                                            .wrap_mode(egui::TextWrapMode::Extend),
                                    );
                                });
                                header.col(|ui| {
                                    ui.add(
                                        egui::Label::new("Access Level")
                                            .wrap_mode(egui::TextWrapMode::Extend),
                                    );
                                });
                            });
                        table.body(|mut body| {
                            for (repo, access_level) in this
                                .owned_repositories
                                .iter()
                                .map(|r| (r, AccessLevel::Admin))
                                .chain(this.accessible_repositories.iter().map(|(r, al)| (r, *al)))
                            {
                                body.row(20.0, |mut row| {
                                    row.col(|ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(&*repo.name)
                                                    .wrap_mode(egui::TextWrapMode::Extend),
                                            )
                                            .clicked()
                                        {
                                            new_state = Some(AppState::RepositoryDetails {
                                                uuid: repo.uuid,
                                            });
                                        };
                                    });
                                    row.col(|ui| {
                                        ui.add(
                                            egui::Label::new(access_level.to_string())
                                                .wrap_mode(egui::TextWrapMode::Extend),
                                        );
                                    });
                                });
                            }
                        });
                    }
                }
                AppState::RepositoryDetails { uuid } => {
                    if let Ok(Some(repo)) = self.cache.get_repository(uuid) {
                        match self.cache.get_stored_repository(uuid, &repo) {
                            Ok(Some(Some(stored_repo))) => {
                                self.show_stored_repository_details(ui, &stored_repo);
                            }
                            Ok(Some(None)) => {
                                ui.label("This repository is not stored locally.");
                                if ui.button("Download Repository").clicked() {
                                    self.change_repository_path(uuid);
                                }
                            }
                            Ok(None) => {
                                ui.spinner();
                            }
                            Err(e) => {
                                ui.label(format!("Error fetching stored repository: {e}"));
                            }
                        }
                    } else {
                        ui.spinner();
                    }
                }
            }
            if let Some(new_state) = new_state {
                self.state = new_state;
            }
        });
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("pixels_per_point", self.ppp.to_string());
    }
}

impl App {
    fn new(ppp: f32) -> Self {
        App {
            cache: RequestCache::new(),
            ppp,
            state: AppState::Main,
        }
    }
    fn header(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) -> Option<AppState> {
        let go_back = match self.state {
            AppState::Main => None,
            AppState::RepositoryDetails { .. } => Some(AppState::Main),
        };
        let username = match self.cache.this_user() {
            Ok(Some(this)) => Arc::clone(&this.user.username),
            Ok(None) => {
                return None;
            }
            Err(e) => {
                ui.label(format!("Error loading user data: {e}"));
                return None;
            }
        };
        let mut new_state = None;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    go_back.is_some(),
                    egui::Button::new(nerdfonts::UNDO_VARIANT),
                )
                .clicked()
            {
                new_state = go_back;
            }
            match self.state {
                AppState::Main => {
                    ui.label("Repositories");
                }
                AppState::RepositoryDetails { uuid } => {
                    if let Some(repo) = self.cache.get_repository(uuid).unwrap_or(None) {
                        ui.label(format!("{}", repo.name));
                        if self
                            .cache
                            .get_stored_repository(uuid, &repo)
                            .ok()
                            .flatten()
                            .flatten()
                            .is_some()
                        {
                            // Show refresh button
                            if ui.button(nerdfonts::REFRESH).clicked() {
                                self.cache.reload_repository(uuid);
                            }
                        }
                    } else {
                        ui.spinner();
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                ui.menu_button(&*username, |ui| {
                    ui.label(format!("UI Scale: {:.2}x", self.ppp));
                    let slider =
                        ui.add(egui::Slider::new(&mut self.ppp, 1.0..=4.0).show_value(false));
                    if slider.drag_stopped() {
                        ctx.set_pixels_per_point(self.ppp);
                    }
                    if slider.changed() && (self.ppp - self.ppp.round()).abs() < 0.1 {
                        self.ppp = self.ppp.round();
                    }
                    ui.add(
                        egui::Label::new(format!("Version: {}", config::COMMIT_HASH))
                            .wrap_mode(egui::TextWrapMode::Extend),
                    );
                    if let Ok(Some(hash)) = self.cache.remote_commit_hash() {
                        ui.add(
                            egui::Label::new(format!("Remote Version: {hash}"))
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    }
                });
                self.update_app_button(ui);
            });
        });
        ui.separator();
        new_state
    }

    fn update_app_button(&mut self, ui: &mut egui::Ui) {
        if let Ok(Some(commit_hash)) = self.cache.remote_commit_hash() {
            if &*commit_hash != config::COMMIT_HASH
                && ui
                    .button(nerdfonts::UPDATE)
                    .on_hover_text("Update Pitsu to the latest version")
                    .clicked()
            {
                // Copy the executable to pitsu_old.exe
                let this_exe =
                    std::env::current_exe().expect("Failed to get current executable path");
                let old_exe = this_exe.with_file_name("pitsu_old.exe");
                std::fs::copy(&this_exe, &old_exe).unwrap_or_else(|e| {
                    log::error!("Failed to create backup copy: {e}");
                    0
                });
                // Run that with --update
                std::process::Command::new(&this_exe)
                    .arg("--update")
                    .spawn()
                    .expect("Failed to spawn update process");
                std::process::exit(0);
            }
        } else {
            ui.spinner();
        }
    }

    fn show_stored_repository_details(&mut self, ui: &mut egui::Ui, stored_repo: &Repository) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                self.repository_info(ui, stored_repo);
                if !stored_repo.pitignore.is_empty() {
                    ui.separator();
                    self.repository_pitignore(ui, stored_repo);
                }
            });
            if !stored_repo.diff.is_empty() {
                ui.separator();
                ui.vertical(|ui| {
                    self.repository_diff(ui, stored_repo);
                });
            }
        });
    }
    fn repository_info(&mut self, ui: &mut egui::Ui, stored_repo: &Repository) {
        let display_path = stored_repo.local.path.display().to_string();
        ui.menu_button(
            if display_path.len() > MAX_PATH_LENGTH {
                format!(
                    "Path: ...{}",
                    &display_path[display_path.len() - MAX_PATH_LENGTH + 3..]
                )
            } else {
                format!("Path: {display_path}")
            },
            |ui| {
                // ui.label(format!("Full Path: {display_path}"));
                ui.add(
                    egui::Label::new(format!("Full Path: {}", stored_repo.local.path.display()))
                        .wrap_mode(egui::TextWrapMode::Extend),
                )
                .on_hover_text("This is the full path to the repository on your local machine.");
                if ui
                    .button("Open in File Explorer")
                    .on_hover_text("Open the repository folder in your file explorer.")
                    .clicked()
                {
                    open::that(&stored_repo.local.path).unwrap_or_else(|e| {
                        log::error!("Failed to open repository path: {e}");
                    });
                    ui.close_menu();
                };
                if ui
                    .button("Change path")
                    .on_hover_text("Change the path to the repository on your local machine.")
                    .clicked()
                {
                    self.change_repository_path(stored_repo.local.uuid);
                    ui.close_menu();
                }
            },
        );
    }
    fn repository_pitignore(&self, ui: &mut egui::Ui, stored_repo: &Repository) {
        ui.label("Pitignore Patterns:");
        for pattern in &stored_repo.pitignore.patterns {
            let label = if pattern.negated {
                format!("!{}", pattern.pattern)
            } else {
                pattern.pattern.to_string()
            };
            ui.label(label);
        }
    }
    fn repository_diff(&self, ui: &mut egui::Ui, stored_repo: &Repository) {
        ui.label("Differences:");
        for diff in stored_repo.diff.iter() {
            let label = match diff.change_type {
                ChangeType::Added => format!("Added: {}", diff.full_path),
                ChangeType::Removed => format!("Removed: {}", diff.full_path),
                ChangeType::Modified => format!("Modified: {}", diff.full_path),
            };
            ui.label(label);
        }
    }

    fn change_repository_path(&mut self, uuid: Uuid) {
        let path = rfd::FileDialog::new()
            .set_title("Select Repository Storage Location")
            .pick_folder();
        if let Some(path) = path {
            if let Err(e) = CONFIG.add_stored(uuid, path) {
                dialogue::rfd_ok_dialogue(&format!("Failed to store repository:\n{e}")).ok();
            }
            self.cache.reload_repository(uuid);
        }
    }
}

pub struct RequestCache {
    remote_commit_hash: Option<PendingRequest<Arc<str>>>,
    update: Option<PendingRequest<Arc<[u8]>>>,
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
            update: None,
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
                            // let diff = remote.files.diff(&repo.folder);
                            let diff = repo.folder.diff(&remote.files);
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
