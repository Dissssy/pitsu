#![warn(clippy::todo)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{mpsc, Arc};

use colors_transform::Color;
use eframe::egui::{self, FontData, Id};
use pitsu_lib::{AccessLevel, ChangeType, Diff, RemoteRepository};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::{get_request, LocalRepository, CONFIG, MAX_PATH_LENGTH, PUBLIC_URL},
    pitignore::Pitignore,
};
mod cache;
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
                        .send(Err(Arc::from(format!("Failed to fetch update: {}", response.status))))
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
        let update = update_receiver.recv().expect("Failed to receive update file");
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
            let sort_states: SortStates = cc
                .storage
                .and_then(|storage| storage.get_string("sort_states"))
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "nerdfonts".into(),
                Arc::new({
                    let mut data = FontData::from_static(include_bytes!("../assets/SymbolsNerdFontMono-Regular.ttf"));
                    data.tweak.y_offset_factor = 0.0;
                    data
                }),
            );

            if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                font_keys.push("nerdfonts".into());
            }
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(App {
                ppp,
                sort: sort_states,
                ..Default::default()
            }))
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
    pitignore: Arc<Pitignore>,
}

pub struct App {
    long_running: cache::RequestCache,
    ppp: f32,
    state: AppState,
    state_stack: Vec<AppState>,
    sort: SortStates,
    edit_pitignore: Option<(Pitignore, EditState, bool)>,
    add_user_text: String,
    add_user_modal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Main,
    Settings,
    RepositoryDetails { uuid: Uuid, hover_state: HoverType },
    EditPitignore { uuid: Uuid },
}

pub enum EditState {
    None,
    AddingPattern { pattern: String, negated: bool },
    EditingPattern { index: usize },
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut new_state = self.header(ui, ctx, frame);
            match self.state {
                AppState::Main => {
                    if let Ok(Some(this)) = self.long_running.this_user() {
                        let table = egui_extras::TableBuilder::new(ui)
                            .striped(false)
                            .resizable(false)
                            .column(egui_extras::Column::auto())
                            .column(egui_extras::Column::auto())
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.add(egui::Label::new("Repository").extend());
                                });
                                header.col(|ui| {
                                    ui.add(egui::Label::new("Access Level").extend());
                                });
                            });
                        table.body(|mut body| {
                            // for (repo, access_level) in this
                            //     .owned_repositories
                            //     .iter()
                            //     .map(|r| (r, AccessLevel::Owner))
                            //     .chain(this.accessible_repositories.iter().map(|(r, al)| (r, *al)))
                            for repo in this
                                .owned_repositories
                                .iter()
                                .chain(this.accessible_repositories.iter())
                            {
                                body.row(20.0, |mut row| {
                                    row.col(|ui| {
                                        if ui
                                            .add(egui::Button::new(&*repo.name).wrap_mode(egui::TextWrapMode::Extend))
                                            .clicked()
                                        {
                                            new_state = Some(AppState::RepositoryDetails {
                                                uuid: repo.uuid,
                                                hover_state: HoverType::None,
                                            });
                                        };
                                    });
                                    row.col(|ui| {
                                        ui.add(egui::Label::new(repo.access_level.to_string()).extend());
                                    });
                                });
                            }
                        });
                    }
                }
                AppState::Settings => {
                    todo!("Settings page not implemented yet");
                }
                AppState::RepositoryDetails { uuid, hover_state } => {
                    if let Ok(Some(repo)) = self.long_running.get_repository(uuid) {
                        match self.long_running.get_stored_repository(uuid, &repo) {
                            Ok(Some(Some(stored_repo))) => {
                                self.show_stored_repository_details(ui, &stored_repo, hover_state, &mut new_state);
                            }
                            Ok(Some(None)) => {
                                ui.label("This repository is not stored locally.");
                                if ui
                                    .add_enabled(
                                        !self.long_running.sync_in_progress(),
                                        egui::Button::new("Download Repository"),
                                    )
                                    .clicked()
                                {
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
                AppState::EditPitignore { uuid } => {
                    ui.with_layout(
                        egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                        |ui| {
                            self.pitignore_editor(ui, uuid, &mut new_state);
                        },
                    );
                }
            }
            if let Some(new_state) = new_state {
                self.state = new_state;
            }
        });
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("pixels_per_point", self.ppp.to_string());
        storage.set_string(
            "sort_states",
            serde_json::to_string(&self.sort).expect("Failed to serialize sort states"),
        );
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    fn new() -> Self {
        App {
            long_running: cache::RequestCache::new(),
            ppp: 1.0,
            state: AppState::RepositoryDetails {
                uuid: Uuid::parse_str("33e704f7-f804-49ed-98ab-b2b940a2cdd5").expect("Invalid UUID"),
                hover_state: HoverType::None,
            },
            state_stack: Vec::new(),
            edit_pitignore: None,
            sort: SortStates::default(),
            add_user_text: String::new(),
            add_user_modal: true,
        }
    }
    fn header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, _frame: &mut eframe::Frame) -> Option<AppState> {
        if self.add_user_modal {
            let modal = egui::Modal::new(Id::new("add_user_modal")).show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.label("Add User to Repository");
                });
                ui.text_edit_singleline(&mut self.add_user_text);
                match self.long_running.all_users() {
                    Ok(Some(users)) => {
                        for user in users {
                            ui.label(format!("{}", user.username));
                        }
                    }
                    Ok(None) => {
                        ui.spinner();
                    }
                    Err(e) => {
                        ui.label(format!("Error fetching users: {e}"));
                    }
                }
            });
            if modal.backdrop_response.clicked() {
                self.add_user_modal = false;
                self.add_user_text.clear();
            }
        }
        if let Ok(Some(uuid)) = self.long_running.any_sync_response() {
            self.long_running
                .reload_repository(uuid)
                .expect("Failed to reload repository");
        }
        let go_back = self.state_stack.pop().or(match self.state {
            AppState::Main => None,
            AppState::Settings => Some(AppState::Main),
            AppState::RepositoryDetails { .. } => Some(AppState::Main),
            AppState::EditPitignore { uuid, .. } => Some(AppState::RepositoryDetails {
                uuid,
                hover_state: HoverType::None,
            }),
        });
        let username = match self.long_running.this_user() {
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
                .add_enabled(go_back.is_some(), egui::Button::new(nerdfonts::UNDO_VARIANT))
                .clicked()
            {
                new_state = go_back;
            }
            let mut new_hover_state = HoverType::None;
            match self.state {
                AppState::Main => {
                    ui.label("Repositories");
                }
                AppState::Settings => {
                    ui.label("Settings");
                }
                AppState::RepositoryDetails { uuid, hover_state } => {
                    new_hover_state = hover_state;
                    if let Some(repo) = self.long_running.get_repository(uuid).unwrap_or(None) {
                        ui.label(format!("{}", repo.name));
                        if ui
                            .menu_button(nerdfonts::ACCOUNT, |ui| {
                                let is_admin = repo.access_level >= AccessLevel::Admin;
                                let table = {
                                    let mut table = egui_extras::TableBuilder::new(ui)
                                        .striped(false)
                                        .resizable(false)
                                        .column(egui_extras::Column::auto())
                                        .column(egui_extras::Column::auto())
                                        .id_salt("repository_access");
                                    if is_admin {
                                        table = table.column(egui_extras::Column::auto());
                                    }
                                    table.header(20.0, |mut header| {
                                        if is_admin {
                                            header.col(|ui| {
                                                ui.add(egui::Label::new(nerdfonts::ACCOUNT).extend());
                                            });
                                        }
                                        header.col(|ui| {
                                            ui.add(egui::Label::new("Name").extend());
                                        });
                                        header.col(|ui| {
                                            ui.add(egui::Label::new("Access").extend());
                                        });
                                    })
                                };
                                table.body(|mut body| {
                                    for user in &repo.users {
                                        body.row(20.0, |mut row| {
                                            if is_admin {
                                                row.col(|ui: &mut egui::Ui| {
                                                    if ui
                                                        .button(nerdfonts::ACCOUNT_MINUS)
                                                        .on_hover_text("Remove user")
                                                        .clicked()
                                                    {
                                                        todo!("Remove user functionality not implemented yet");
                                                    }
                                                });
                                            }
                                            row.col(|ui| {
                                                ui.add(egui::Label::new(&*user.user.username).extend());
                                            });
                                            row.col(|ui| {
                                                ui.add(egui::Label::new(format!("{:?}", user.access_level)).extend());
                                            });
                                        });
                                    }
                                });
                                ui.centered_and_justified(|ui| {
                                    ui.set_height(20.0);
                                    if ui
                                        .button(nerdfonts::ACCOUNT_PLUS)
                                        .on_hover_text("Add user to repository")
                                        .clicked()
                                    {
                                        self.add_user_modal = true;
                                    };
                                });
                            })
                            .response
                            .clicked()
                        {
                            self.add_user_text.clear();
                        };
                        if let Some(stored) = self
                            .long_running
                            .get_stored_repository(uuid, &repo)
                            .ok()
                            .flatten()
                            .flatten()
                        {
                            // Show refresh button
                            if ui
                                .add_enabled(
                                    !self.long_running.sync_in_progress(),
                                    egui::Button::new(nerdfonts::REFRESH),
                                )
                                .clicked()
                            {
                                self.long_running
                                    .reload_repository(uuid)
                                    .expect("Failed to reload repository after changing path");
                            }
                            if !stored.diff.is_empty() {
                                if repo.access_level >= AccessLevel::Write {
                                    let hover_text = {
                                        let mut text = String::from("Clicking this will:\n");
                                        let number_to_upload = stored
                                            .diff
                                            .iter()
                                            .filter(|d| {
                                                d.change_type == ChangeType::OnClient
                                                    || d.change_type == ChangeType::Modified
                                            })
                                            .count();
                                        if number_to_upload > 0 {
                                            text.push_str(&format!(" - Upload {number_to_upload} changes\n",));
                                        }
                                        let num_to_del = stored
                                            .diff
                                            .iter()
                                            .filter(|d| d.change_type == ChangeType::OnServer)
                                            .count();
                                        if num_to_del > 0 {
                                            text.push_str(&format!(" - Delete {num_to_del} files from server\n",));
                                        }
                                        text.trim()
                                            .replace(" 1 changes", " 1 change")
                                            .replace(" 1 files", " 1 file")
                                            .to_string()
                                    };
                                    let upload = if self.long_running.upload_in_progress() {
                                        ui.add_enabled(!self.long_running.sync_in_progress(), egui::Spinner::new())
                                    } else {
                                        ui.add_enabled(
                                            !self.long_running.sync_in_progress(),
                                            egui::Button::new(
                                                egui::RichText::new(nerdfonts::UPLOAD).color(egui::Color32::YELLOW),
                                            ),
                                        )
                                        .on_hover_text(&hover_text)
                                    };
                                    if upload.clicked() {
                                        if let Err(e) = self.long_running.upload_files(Arc::clone(&stored), hover_text)
                                        {
                                            panic!("Failed to upload files: {e}");
                                        }
                                    }
                                    if upload.hovered() {
                                        new_hover_state = HoverType::SyncUp;
                                    } else if hover_state == HoverType::SyncUp {
                                        new_hover_state = HoverType::None;
                                    }
                                }
                                if repo.access_level >= AccessLevel::Read {
                                    let hover_text = {
                                        let mut text = String::from("Clicking this will:\n");
                                        let number_to_download = stored
                                            .diff
                                            .iter()
                                            .filter(|d| {
                                                d.change_type == ChangeType::OnServer
                                                    || d.change_type == ChangeType::Modified
                                            })
                                            .count();
                                        if number_to_download > 0 {
                                            text.push_str(&format!(" - Download {number_to_download} changes\n",));
                                        }
                                        let number_to_delete_from_client = stored
                                            .diff
                                            .iter()
                                            .filter(|d| d.change_type == ChangeType::OnClient)
                                            .count();
                                        if number_to_delete_from_client > 0 {
                                            text.push_str(&format!(
                                                " - Delete {number_to_delete_from_client} files from client\n",
                                            ));
                                        }
                                        let size = repo.size as i64 - stored.local.folder.size() as i64;
                                        let sign = if size >= 0 { "+" } else { "-" };
                                        text.push_str(&format!(
                                            " - Size change: {}{}",
                                            sign,
                                            readable_size_and_color(size.unsigned_abs()).0
                                        ));
                                        text.trim()
                                            .replace(" 1 changes", " 1 change")
                                            .replace(" 1 files", " 1 file")
                                            .to_string()
                                    };
                                    let download = if self.long_running.download_in_progress() {
                                        ui.add_enabled(!self.long_running.sync_in_progress(), egui::Spinner::new())
                                    } else {
                                        ui.add_enabled(
                                            !self.long_running.sync_in_progress(),
                                            egui::Button::new(
                                                egui::RichText::new(nerdfonts::DOWNLOAD).color(egui::Color32::GREEN),
                                            ),
                                        )
                                        .on_hover_text(&hover_text)
                                    };
                                    if download.clicked() {
                                        if let Err(e) =
                                            self.long_running.download_files(Arc::clone(&stored), hover_text)
                                        {
                                            panic!("Failed to download files: {e}");
                                        }
                                    }
                                    if download.hovered() {
                                        new_hover_state = HoverType::SyncDown;
                                    } else if hover_state == HoverType::SyncDown {
                                        new_hover_state = HoverType::None;
                                    }
                                }
                            }
                        }
                    } else {
                        ui.spinner();
                    }
                }
                AppState::EditPitignore { uuid } => {
                    if let Ok(Some(repo)) = self.long_running.get_repository(uuid) {
                        ui.label(format!("Editing .pitignore for {}", repo.name));
                        if let Some((_, _, dirty)) = self.edit_pitignore {
                            if dirty {
                                ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(nerdfonts::SAVE).color(egui::Color32::LIGHT_GREEN),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Extend),
                                )
                                .on_hover_text("Save changes to .pitignore");
                            }
                        }
                    } else {
                        // NOTE TO SELF TOO TIRED TO KEEP GOING BUT CHANGE THIS EDITING SHIT TO JUST PULL DIRECTLY OFF OF THE MUTABLE PITIGNORE INSTEAD OF ALL THIS FANCY SHIT LOL
                        ui.spinner();
                    }
                }
            }
            if let AppState::RepositoryDetails { hover_state, .. } = &mut self.state {
                std::mem::swap(hover_state, &mut new_hover_state);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                ui.menu_button(&*username, |ui| {
                    ui.label(format!("UI Scale: {:.2}x", self.ppp));
                    let slider = ui.add(egui::Slider::new(&mut self.ppp, 1.0..=4.0).show_value(false));
                    if slider.drag_stopped() {
                        ctx.set_pixels_per_point(self.ppp);
                    }
                    if slider.changed() && (self.ppp - self.ppp.round()).abs() < 0.1 {
                        self.ppp = self.ppp.round();
                    }
                    ui.add(egui::Label::new(format!("Version: {}", *config::VERSION_NUMBER)).extend());
                    if let Ok(Some(hash)) = self.long_running.remote_version_number() {
                        ui.add(egui::Label::new(format!("Remote Version: {hash}")).extend());
                    }
                });
                self.update_app_button(ui);
            });
        });
        ui.separator();
        new_state
    }

    fn update_app_button(&mut self, ui: &mut egui::Ui) {
        if let Ok(Some(version_number)) = self.long_running.remote_version_number() {
            if *version_number != *config::VERSION_NUMBER
                && ui
                    .button(
                        egui::RichText::new(nerdfonts::UPDATE).color(if config::VERSION_NUMBER.is_dev() {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::GREEN
                        }),
                    )
                    .on_hover_text("Update Pitsu to the latest version")
                    .clicked()
            {
                // Copy the executable to pitsu_old.exe
                let this_exe = std::env::current_exe().expect("Failed to get current executable path");
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

    fn show_stored_repository_details(
        &mut self,
        ui: &mut egui::Ui,
        stored_repo: &Repository,
        hover_state: HoverType,
        new_state: &mut Option<AppState>,
    ) {
        ui.with_layout(
            egui::Layout::left_to_right(egui::Align::LEFT)
                .with_main_justify(true)
                .with_cross_justify(true),
            |ui| {
                ui.vertical(|ui| {
                    self.repository_info(ui, stored_repo);
                    // if !stored_repo.pitignore.patterns.is_empty() {
                    //     // ui.separator();
                    self.repository_pitignore(ui, stored_repo, new_state);
                    // }
                });
                let local_empty = stored_repo.local.folder.is_empty();
                if !stored_repo.diff.is_empty() {
                    ui.with_layout(
                        egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(local_empty),
                        |ui| {
                            self.repository_diff(ui, stored_repo, hover_state, local_empty);
                        },
                    );
                }
                if !local_empty {
                    ui.with_layout(
                        egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                        |ui| {
                            self.repository_local_files(ui, stored_repo, hover_state);
                        },
                    );
                }
            },
        );
    }
    fn repository_info(&mut self, ui: &mut egui::Ui, stored_repo: &Repository) {
        let display_path = stored_repo.local.path.display().to_string();
        ui.menu_button(
            if display_path.len() > MAX_PATH_LENGTH {
                format!("Path: ...{}", &display_path[display_path.len() - MAX_PATH_LENGTH + 3..])
            } else {
                format!("Path: {display_path}")
            },
            |ui| {
                // ui.label(format!("Full Path: {display_path}"));
                ui.add(egui::Label::new(format!("Full Path: {}", stored_repo.local.path.display())).extend())
                    .on_hover_text("This is the full path to the repository on your local machine.");
                if ui
                    .button("Open in File Explorer")
                    .on_hover_text("Open the repository folder in your file explorer.")
                    .clicked()
                {
                    open::that(&stored_repo.local.path).unwrap_or_else(|e| {
                        log::error!("Failed to open repository path: {e}");
                    });
                    ui.close();
                };
                if ui
                    .add_enabled(!self.long_running.sync_in_progress(), egui::Button::new("Change path"))
                    .on_hover_text("Change the local repository path.")
                    .clicked()
                {
                    self.change_repository_path(stored_repo.local.uuid);
                    ui.close();
                }
            },
        );
    }
    fn repository_pitignore(&mut self, ui: &mut egui::Ui, stored_repo: &Repository, new_state: &mut Option<AppState>) {
        if !stored_repo.pitignore.patterns.is_empty() {
            let table = egui_extras::TableBuilder::new(ui)
                .striped(false)
                .resizable(false)
                .id_salt("pitignore_patterns")
                .column(egui_extras::Column::auto())
                .column(egui_extras::Column::auto())
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        // ui.add(egui::Label::new(nerdfonts::UPLOAD).extend());
                        if ui.button(nerdfonts::EDIT).on_hover_text("Edit .pitignore").clicked() {
                            self.edit_pitignore = Some(((*stored_repo.pitignore).clone(), EditState::None, false));
                            *new_state = Some(AppState::EditPitignore {
                                uuid: stored_repo.local.uuid,
                            });
                        }
                    });
                    header.col(|ui| {
                        ui.add(egui::Label::new("Pitignore").extend());
                    });
                });
            table.body(|mut body| {
                for (_index, pattern) in &stored_repo.pitignore.patterns {
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(if pattern.negated {
                                    egui::RichText::new(nerdfonts::CHECK).color(egui::Color32::LIGHT_GREEN)
                                } else {
                                    egui::RichText::new(nerdfonts::BLOCKED).color(egui::Color32::LIGHT_RED)
                                })
                                .extend(),
                            );
                        });
                        row.col(|ui| {
                            ui.add(egui::Label::new(&*pattern.pattern).extend());
                        });
                    });
                }
            });
        } else {
            ui.horizontal(|ui| {
                if ui.button(nerdfonts::EDIT).on_hover_text("Edit .pitignore").clicked() {
                    *new_state = Some(AppState::EditPitignore {
                        uuid: stored_repo.local.uuid,
                    });
                }
                ui.label("Pitignore");
            });
        }
    }
    fn repository_diff(
        &mut self,
        ui: &mut egui::Ui,
        stored_repo: &Repository,
        hover_state: HoverType,
        local_empty: bool,
    ) {
        // ui.label("Differences:");
        // for diff in stored_repo.diff.iter() {
        //     let label = match diff.change_type {
        //         ChangeType::Added => format!("Added: {}", diff.full_path),
        //         ChangeType::Removed => format!("Removed: {}", diff.full_path),
        //         ChangeType::Modified => format!("Modified: {}", diff.full_path),
        //     };
        //     ui.label(label);
        // }
        // let mut sort_now = false;
        let table = egui_extras::TableBuilder::new(ui)
            .striped(false)
            .resizable(false)
            .id_salt("repository_diff")
            .column(egui_extras::Column::auto())
            .column(egui_extras::Column::auto())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    // ui.add(egui::Label::new(nerdfonts::LOCATION).extend());
                    if ui
                        .button(egui::RichText::new(match self.sort.diff {
                            DiffSort::OnClient => nerdfonts::SORT_BOOL_ASCENDING,
                            DiffSort::OnServer => nerdfonts::SORT_BOOL_DESCENDING,
                            _ => nerdfonts::LOCATION,
                        }))
                        .clicked()
                    {
                        self.sort.diff = match self.sort.diff {
                            DiffSort::OnClient => DiffSort::OnServer,
                            DiffSort::OnServer => DiffSort::OnClient,
                            _ => DiffSort::OnServer,
                        };
                        // sort_now = true;
                    }
                });
                header.col(|ui| {
                    // ui.add(egui::Label::new("Full Path").extend());
                    if ui
                        .button(egui::RichText::new(
                            format!(
                                "Full Path {}",
                                match self.sort.diff {
                                    DiffSort::Alphabetical => nerdfonts::SORT_ALPHABETICAL_ASCENDING,
                                    DiffSort::AlphabeticalReverse => nerdfonts::SORT_ALPHABETICAL_DESCENDING,
                                    _ => "",
                                }
                            )
                            .trim(),
                        ))
                        .clicked()
                    {
                        self.sort.diff = match self.sort.diff {
                            DiffSort::Alphabetical => DiffSort::AlphabeticalReverse,
                            DiffSort::AlphabeticalReverse => DiffSort::Alphabetical,
                            _ => DiffSort::Alphabetical,
                        };
                        // sort_now = true;
                    }
                });
            });
        // if sort_now {
        let mut diffs = stored_repo.diff.iter().cloned().collect::<Vec<_>>();
        match self.sort.diff {
            DiffSort::OnClient => {
                diffs.sort_by(|a, b| match (a.change_type, b.change_type) {
                    (ChangeType::OnClient, ChangeType::OnServer) => std::cmp::Ordering::Less,
                    (ChangeType::OnServer, ChangeType::OnClient) => std::cmp::Ordering::Greater,
                    _ => a.full_path.to_lowercase().cmp(&b.full_path.to_lowercase()),
                });
            }
            DiffSort::OnServer => {
                diffs.sort_by(|a, b| match (a.change_type, b.change_type) {
                    (ChangeType::OnServer, ChangeType::OnClient) => std::cmp::Ordering::Less,
                    (ChangeType::OnClient, ChangeType::OnServer) => std::cmp::Ordering::Greater,
                    _ => a.full_path.to_lowercase().cmp(&b.full_path.to_lowercase()),
                });
            }
            DiffSort::Alphabetical => {
                diffs.sort_by(|a, b| a.full_path.to_lowercase().cmp(&b.full_path.to_lowercase()));
            }
            DiffSort::AlphabeticalReverse => {
                diffs.sort_by(|a, b| b.full_path.to_lowercase().cmp(&a.full_path.to_lowercase()));
            }
        }
        // }
        table.body(|mut body| {
            if local_empty {
                let ui = body.ui_mut();
                ui.set_width(ui.available_width());
            }
            let download = egui::RichText::new(nerdfonts::DOWNLOAD).color(egui::Color32::GREEN);
            let upload = egui::RichText::new(nerdfonts::UPLOAD).color(egui::Color32::YELLOW);
            for diff in diffs.iter() {
                body.row(20.0, |mut row| {
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(match diff.change_type {
                                ChangeType::OnClient => match hover_state {
                                    HoverType::SyncUp => upload.clone(),
                                    HoverType::SyncDown => {
                                        egui::RichText::new(nerdfonts::TRASH).color(egui::Color32::RED)
                                    }
                                    HoverType::None => {
                                        egui::RichText::new(nerdfonts::HOME).color(egui::Color32::ORANGE)
                                    }
                                },
                                ChangeType::OnServer => match hover_state {
                                    HoverType::SyncUp => {
                                        egui::RichText::new(nerdfonts::TRASH).color(egui::Color32::RED)
                                    }
                                    HoverType::SyncDown => download.clone(),
                                    HoverType::None => {
                                        egui::RichText::new(nerdfonts::SERVER).color(egui::Color32::GOLD)
                                    }
                                },
                                ChangeType::Modified => match hover_state {
                                    HoverType::SyncUp => upload.clone(),
                                    HoverType::SyncDown => download.clone(),
                                    HoverType::None => egui::RichText::new(nerdfonts::EDIT).color(egui::Color32::CYAN),
                                },
                            })
                            .extend(),
                        );
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(format!("{}  ", diff.full_path)).extend());
                    });
                });
            }
        });
    }
    fn repository_local_files(&mut self, ui: &mut egui::Ui, stored_repo: &Repository, hover_state: HoverType) {
        let table = egui_extras::TableBuilder::new(ui)
            .striped(false)
            .resizable(false)
            .id_salt("repository_local_files")
            .column(egui_extras::Column::auto())
            .column(egui_extras::Column::auto())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    // ui.add(egui::Label::new("Size").extend());
                    if ui
                        .button(egui::RichText::new(
                            format!(
                                "Size {}",
                                match self.sort.local_files {
                                    LocalSort::Size => nerdfonts::SORT_NUMERIC_ASCENDING,
                                    LocalSort::SizeReverse => nerdfonts::SORT_NUMERIC_DESCENDING,
                                    _ => "",
                                }
                            )
                            .trim(),
                        ))
                        .clicked()
                    {
                        self.sort.local_files = match self.sort.local_files {
                            LocalSort::Size => LocalSort::SizeReverse,
                            LocalSort::SizeReverse => LocalSort::Size,
                            _ => LocalSort::Size,
                        };
                    }
                });
                header.col(|ui| {
                    // ui.add(egui::Label::new("File Name").extend());
                    if ui
                        .button(egui::RichText::new(format!(
                            "File Name {}",
                            match self.sort.local_files {
                                LocalSort::Name => nerdfonts::SORT_ALPHABETICAL_ASCENDING,
                                LocalSort::NameReverse => nerdfonts::SORT_ALPHABETICAL_DESCENDING,
                                _ => "",
                            }
                        )))
                        .clicked()
                    {
                        self.sort.local_files = match self.sort.local_files {
                            LocalSort::Name => LocalSort::NameReverse,
                            LocalSort::NameReverse => LocalSort::Name,
                            _ => LocalSort::Name,
                        };
                    }
                });
            });
        // if sort_now {
        let mut files = stored_repo.local.folder.files();
        match self.sort.local_files {
            LocalSort::Size => {
                files.sort_by(|(_, size_a), (_, size_b)| size_a.cmp(size_b));
            }
            LocalSort::SizeReverse => {
                files.sort_by(|(_, size_a), (_, size_b)| size_b.cmp(size_a));
            }
            LocalSort::Name => {
                files.sort_by(|(name_a, _), (name_b, _)| name_a.to_lowercase().cmp(&name_b.to_lowercase()));
            }
            LocalSort::NameReverse => {
                files.sort_by(|(name_a, _), (name_b, _)| name_b.to_lowercase().cmp(&name_a.to_lowercase()));
            }
        }
        // }

        table.body(|mut body| {
            let ui = body.ui_mut();
            ui.set_width(ui.available_width());
            for (name, size) in files {
                body.row(20.0, |mut row| {
                    let (size, color) = readable_size_and_color(size);
                    let will_be_deleted = {
                        if hover_state == HoverType::SyncDown {
                            stored_repo
                                .diff
                                .iter()
                                .any(|d| d.full_path == name && d.change_type == ChangeType::OnClient)
                        } else {
                            false
                        }
                    };
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&*size).color(if will_be_deleted {
                                egui::Color32::RED
                            } else {
                                color
                            }))
                            .extend(),
                        );
                    });
                    row.col(|ui| {
                        // ui.add(egui::Label::new(&*name).extend());
                        if will_be_deleted {
                            ui.add(
                                egui::Label::new(egui::RichText::new(&*name).color(egui::Color32::RED).strikethrough())
                                    .extend(),
                            );
                        } else {
                            ui.add(egui::Label::new(&*name).extend());
                        }
                    });
                });
            }
        });
    }

    fn change_repository_path(&mut self, uuid: Uuid) {
        let path = rfd::FileDialog::new()
            .set_title("Select Repository Storage Location")
            .pick_folder();
        if let Some(path) = path {
            if let Err(e) = CONFIG.add_stored(uuid, path) {
                dialogue::rfd_ok_dialogue(&format!("Failed to store repository:\n{e}")).ok();
            }
            self.long_running
                .reload_repository(uuid)
                .expect("Failed to reload repository after changing path");
        }
    }

    fn pitignore_editor(&mut self, ui: &mut egui::Ui, uuid: Uuid, new_state: &mut Option<AppState>) {
        match self.edit_pitignore {
            Some((ref mut pitignore, ref mut edit, ref mut dirty)) => {
                let table = egui_extras::TableBuilder::new(ui)
                    .column(egui_extras::Column::auto())
                    .column(egui_extras::Column::auto())
                    .column(egui_extras::Column::auto())
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.add(egui::Label::new(nerdfonts::TRASH).extend());
                        });
                        header.col(|ui| {
                            ui.add(egui::Label::new(nerdfonts::DASH).extend());
                        });
                        header.col(|ui| {
                            ui.add(egui::Label::new("Pattern").extend());
                        });
                    });
                let mut new_edit_state = None;
                match edit {
                    EditState::None => {
                        table.body(|mut body| {
                            pitignore.patterns.retain_mut(|(index, p)| {
                                let mut delete = false;
                                body.row(20.0, |mut row| {
                                    row.col(|ui| {
                                        delete = ui
                                            .add(egui::Button::new(
                                                egui::RichText::new(nerdfonts::TRASH).color(egui::Color32::RED),
                                            ))
                                            .on_hover_text("Delete this pattern")
                                            .clicked();
                                    });
                                    row.col(|ui| {
                                        let flip = if p.negated {
                                            ui.add(
                                                egui::Button::new(
                                                    egui::RichText::new(nerdfonts::CHECK)
                                                        .color(egui::Color32::LIGHT_GREEN),
                                                )
                                                .wrap_mode(egui::TextWrapMode::Extend),
                                            )
                                            .clicked()
                                        } else {
                                            ui.add(
                                                egui::Button::new(
                                                    egui::RichText::new(nerdfonts::BLOCKED)
                                                        .color(egui::Color32::LIGHT_RED),
                                                )
                                                .wrap_mode(egui::TextWrapMode::Extend),
                                            )
                                            .clicked()
                                        };
                                        if flip {
                                            *dirty = true;
                                            p.negated = !p.negated;
                                        }
                                    });
                                    row.col(|ui| {
                                        if ui
                                            .add(egui::Button::new(&*p.pattern).wrap_mode(egui::TextWrapMode::Extend))
                                            .clicked()
                                        {
                                            *dirty = true;
                                            new_edit_state = Some(EditState::EditingPattern { index: *index });
                                        }
                                    });
                                });
                                if delete {
                                    *dirty = true;
                                }
                                !delete
                            });
                        });
                    }
                    EditState::AddingPattern { pattern, negated } => {
                        todo!()
                    }
                    EditState::EditingPattern { index } => {
                        table.body(|mut body| {
                            pitignore.patterns.retain_mut(|(tindex, p)| {
                                let mut delete = false;
                                body.row(20.0, |mut row| {
                                    row.col(|ui| {
                                        delete = ui
                                            .add_enabled(
                                                index == tindex,
                                                egui::Button::new(
                                                    egui::RichText::new(nerdfonts::TRASH).color(egui::Color32::RED),
                                                ),
                                            )
                                            .on_hover_text("Delete this pattern")
                                            .clicked();
                                    });
                                    row.col(|ui| {
                                        let flip = if p.negated {
                                            ui.add_enabled(
                                                index == tindex,
                                                egui::Button::new(
                                                    egui::RichText::new(nerdfonts::CHECK)
                                                        .color(egui::Color32::LIGHT_GREEN),
                                                )
                                                .wrap_mode(egui::TextWrapMode::Extend),
                                            )
                                            .clicked()
                                        } else {
                                            ui.add_enabled(
                                                index == tindex,
                                                egui::Button::new(
                                                    egui::RichText::new(nerdfonts::BLOCKED)
                                                        .color(egui::Color32::LIGHT_RED),
                                                )
                                                .wrap_mode(egui::TextWrapMode::Extend),
                                            )
                                            .clicked()
                                        };
                                        if flip {
                                            *dirty = true;
                                            p.negated = !p.negated;
                                        }
                                    });
                                    row.col(|ui| {
                                        if ui
                                            .add_enabled(
                                                index == tindex,
                                                egui::Button::new(&*p.pattern).wrap_mode(egui::TextWrapMode::Extend),
                                            )
                                            .clicked()
                                        {
                                            *dirty = true;
                                            new_edit_state = Some(EditState::EditingPattern { index: *index });
                                        }
                                    });
                                });
                                if delete {
                                    *dirty = true;
                                    new_edit_state = Some(EditState::None);
                                }
                                !delete
                            });
                        });
                    }
                }
                // if *dirty {
                //     ui.add(
                //         egui::Button::new(
                //             egui::RichText::new(nerdfonts::SAVE).color(egui::Color32::LIGHT_GREEN),
                //         )
                //         .wrap_mode(egui::TextWrapMode::Extend),
                //     )
                //     .on_hover_text("Save changes to .pitignore");
                // }
                if let Some(new_edit_state) = new_edit_state {
                    *edit = new_edit_state;
                }
            }
            None => {
                *new_state = Some(AppState::RepositoryDetails {
                    uuid,
                    hover_state: HoverType::None,
                });
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverType {
    None,
    SyncUp,
    SyncDown,
}

const SIZES: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

fn readable_size_and_color(bytes: u64) -> (Arc<str>, egui::Color32) {
    // the color returned should be a rainbow gradient, with red being 1B, orange being 1KB, yellow being 1MB, green being 1GB, blue being 1TB, and purple being 1PB
    let mut size = bytes as f32;
    let mut index = 0;
    while size >= 1024.0 && index < SIZES.len() - 1 {
        size /= 1024.0;
        index += 1;
    }
    // let hsl = colors_transform::Hsl::from(
    //     360.0 - ((((index as f32 / (SIZES.len() - 1) as f32) * 360.0) + 240.0) % 360.0),
    //     100.0,
    //     50.0,
    // );
    // like above but with smooth transition based on size / 1024.0
    let hsl = colors_transform::Hsl::from(
        360.0 - ((((index as f32 + size / 1024.0) / (SIZES.len() - 1) as f32) * 360.0 + 240.0) % 360.0),
        100.0,
        50.0,
    );

    let rgb = hsl.to_rgb();

    (
        Arc::from(format!(
            "{} {}",
            if size >= 100.0 {
                format!("{size:.0}")
            } else if size >= 10.0 {
                format!("{size:.1}")
            } else {
                format!("{size:.2}")
            }
            .trim_end_matches('0')
            .trim_end_matches('.'),
            SIZES[index]
        )),
        egui::Color32::from_rgb(
            rgb.get_red().round() as u8,
            rgb.get_green().round() as u8,
            rgb.get_blue().round() as u8,
        ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct SortStates {
    diff: DiffSort,
    local_files: LocalSort,
}

impl Default for SortStates {
    fn default() -> Self {
        Self {
            diff: DiffSort::Alphabetical,
            local_files: LocalSort::SizeReverse,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffSort {
    OnClient, // Onclient, then Modified, then OnServer
    // Modified,            // Modified, then OnClient, then OnServer
    OnServer,            // OnServer, then Modified, then OnClient
    Alphabetical,        // Sort alphabetically by full path
    AlphabeticalReverse, // Sort alphabetically by full path in reverse
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalSort {
    Name,        // Sort by name
    NameReverse, // Sort by name in reverse
    Size,        // Sort by size
    SizeReverse, // Sort by size in reverse
}
