#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{mpsc, Arc};

use colors_transform::Color;
use eframe::egui::{self, FontData};
use pitsu_lib::{AccessLevel, ChangeType, Diff};
use uuid::Uuid;

use crate::config::{get_request, LocalRepository, CONFIG, MAX_PATH_LENGTH, PUBLIC_URL};
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
                    let mut data = FontData::from_static(include_bytes!(
                        "../assets/SymbolsNerdFontMono-Regular.ttf"
                    ));
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
    cache: cache::RequestCache,
    ppp: f32,
    state: AppState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Main,
    RepositoryDetails { uuid: Uuid, hover_state: HoverType },
    EditPitignore { uuid: Uuid },
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut new_state = self.header(ui, ctx, frame);
            match self.state {
                AppState::Main => {
                    if let Ok(Some(this)) = self.cache.this_user() {
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
                                                hover_state: HoverType::None,
                                            });
                                        };
                                    });
                                    row.col(|ui| {
                                        ui.add(egui::Label::new(access_level.to_string()).extend());
                                    });
                                });
                            }
                        });
                    }
                }
                AppState::RepositoryDetails { uuid, hover_state } => {
                    if let Ok(Some(repo)) = self.cache.get_repository(uuid) {
                        match self.cache.get_stored_repository(uuid, &repo) {
                            Ok(Some(Some(stored_repo))) => {
                                self.show_stored_repository_details(
                                    ui,
                                    &stored_repo,
                                    hover_state,
                                    &mut new_state,
                                );
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
                AppState::EditPitignore { uuid } => {
                    if let Ok(Some(repo)) = self.cache.get_repository(uuid) {
                        ui.label(format!("Edit pitignore for {}", repo.name));
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
            cache: cache::RequestCache::new(),
            ppp,
            state: AppState::RepositoryDetails {
                uuid: Uuid::parse_str("33e704f7-f804-49ed-98ab-b2b940a2cdd5")
                    .expect("Invalid UUID"),
                hover_state: HoverType::None,
            },
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
            AppState::EditPitignore { uuid } => Some(AppState::RepositoryDetails {
                uuid,
                hover_state: HoverType::None,
            }),
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
            let mut new_hover_state = HoverType::None;
            match self.state {
                AppState::Main => {
                    ui.label("Repositories");
                }
                AppState::RepositoryDetails { uuid, hover_state } => {
                    new_hover_state = hover_state;
                    if let Some(repo) = self.cache.get_repository(uuid).unwrap_or(None) {
                        ui.label(format!("{}", repo.name));
                        if let Some(stored) = self
                            .cache
                            .get_stored_repository(uuid, &repo)
                            .ok()
                            .flatten()
                            .flatten()
                        {
                            // Show refresh button
                            if ui.button(nerdfonts::REFRESH).clicked() {
                                self.cache.reload_repository(uuid);
                            }
                            if !stored.diff.is_empty() {
                                if repo.access_level >= AccessLevel::Write {
                                    let upload = ui
                                        .button(
                                            egui::RichText::new(nerdfonts::UPLOAD)
                                                .color(egui::Color32::YELLOW),
                                        )
                                        .on_hover_text({
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
                                                text.push_str(&format!(
                                                    " - Upload {number_to_upload} changes\n",
                                                ));
                                            }
                                            let number_to_delete_from_server = stored
                                                .diff
                                                .iter()
                                                .filter(|d| d.change_type == ChangeType::OnServer)
                                                .count();
                                            if number_to_delete_from_server > 0 {
                                                text.push_str(&format!(
                                                    " - Delete {number_to_delete_from_server} files from server\n",
                                                ));
                                            }
                                            text.trim().to_string()
                                        });
                                    if upload.clicked() {
                                        todo!("Upload repository changes");
                                    }
                                    if upload.hovered() {
                                        new_hover_state = HoverType::SyncUp;
                                    } else if hover_state == HoverType::SyncUp {
                                        new_hover_state = HoverType::None;
                                    }
                                }
                                if repo.access_level >= AccessLevel::Read {
                                    let download = ui
                                        .button(
                                            egui::RichText::new(nerdfonts::DOWNLOAD)
                                                .color(egui::Color32::GREEN),
                                        )
                                        .on_hover_text({
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
                                                text.push_str(&format!(
                                                    " - Download {number_to_download} changes\n",
                                                ));
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
                                            text.trim().to_string()
                                        });
                                    if download.clicked() {
                                        todo!("Download repository changes");
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
                    if let Ok(Some(repo)) = self.cache.get_repository(uuid) {
                        ui.label(format!("Edit pitignore for {}", repo.name));
                    } else {
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
                    let slider =
                        ui.add(egui::Slider::new(&mut self.ppp, 1.0..=4.0).show_value(false));
                    if slider.drag_stopped() {
                        ctx.set_pixels_per_point(self.ppp);
                    }
                    if slider.changed() && (self.ppp - self.ppp.round()).abs() < 0.1 {
                        self.ppp = self.ppp.round();
                    }
                    ui.add(
                        egui::Label::new(format!("Version: {}", *config::VERSION_NUMBER))
                            .extend(),
                    );
                    if let Ok(Some(hash)) = self.cache.remote_version_number() {
                        ui.add(
                            egui::Label::new(format!("Remote Version: {hash}"))
                                .extend(),
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
        if let Ok(Some(version_number)) = self.cache.remote_version_number() {
            if *version_number != *config::VERSION_NUMBER
                && ui
                    .button(egui::RichText::new(nerdfonts::UPDATE).color(
                        if config::VERSION_NUMBER.is_dev() {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::GREEN
                        },
                    ))
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
                            self.repository_local_files(ui, stored_repo);
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
                        .extend(),
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
                    ui.close();
                };
                if ui
                    .button("Change path")
                    .on_hover_text("Change the path to the repository on your local machine.")
                    .clicked()
                {
                    self.change_repository_path(stored_repo.local.uuid);
                    ui.close();
                }
            },
        );
    }
    fn repository_pitignore(
        &self,
        ui: &mut egui::Ui,
        stored_repo: &Repository,
        new_state: &mut Option<AppState>,
    ) {
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
                        if ui
                            .button(nerdfonts::EDIT)
                            .on_hover_text("Edit .pitignore")
                            .clicked()
                        {
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
                for pattern in &stored_repo.pitignore.patterns {
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(if pattern.negated {
                                    egui::RichText::new(nerdfonts::CHECK)
                                        .color(egui::Color32::LIGHT_GREEN)
                                } else {
                                    egui::RichText::new(nerdfonts::BLOCKED)
                                        .color(egui::Color32::LIGHT_RED)
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
                if ui
                    .button(nerdfonts::EDIT)
                    .on_hover_text("Edit .pitignore")
                    .clicked()
                {
                    *new_state = Some(AppState::EditPitignore {
                        uuid: stored_repo.local.uuid,
                    });
                }
                ui.label("Pitignore");
            });
        }
    }
    fn repository_diff(
        &self,
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

        let table = egui_extras::TableBuilder::new(ui)
            .striped(false)
            .resizable(false)
            .id_salt("repository_diff")
            .column(egui_extras::Column::auto())
            .column(egui_extras::Column::auto())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.add(egui::Label::new(nerdfonts::LOCATION).extend());
                });
                header.col(|ui| {
                    ui.add(egui::Label::new("Full Path").extend());
                });
            });
        table.body(|mut body| {
            if local_empty {
                let ui = body.ui_mut();
                ui.set_width(ui.available_width());
            }
            let download = egui::RichText::new(nerdfonts::DOWNLOAD).color(egui::Color32::GREEN);
            let upload = egui::RichText::new(nerdfonts::UPLOAD).color(egui::Color32::YELLOW);
            for diff in stored_repo.diff.iter() {
                body.row(20.0, |mut row| {
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(match diff.change_type {
                                ChangeType::OnClient => match hover_state {
                                    HoverType::SyncUp => upload.clone(),
                                    HoverType::SyncDown => egui::RichText::new(nerdfonts::TRASH)
                                        .color(egui::Color32::RED),
                                    HoverType::None => egui::RichText::new(nerdfonts::HOME)
                                        .color(egui::Color32::ORANGE),
                                },
                                ChangeType::OnServer => match hover_state {
                                    HoverType::SyncUp => egui::RichText::new(nerdfonts::TRASH)
                                        .color(egui::Color32::RED),
                                    HoverType::SyncDown => download.clone(),
                                    HoverType::None => egui::RichText::new(nerdfonts::SERVER)
                                        .color(egui::Color32::GOLD),
                                },
                                ChangeType::Modified => match hover_state {
                                    HoverType::SyncUp => upload.clone(),
                                    HoverType::SyncDown => download.clone(),
                                    HoverType::None => egui::RichText::new(nerdfonts::EDIT)
                                        .color(egui::Color32::CYAN),
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
    fn repository_local_files(&self, ui: &mut egui::Ui, stored_repo: &Repository) {
        let table = egui_extras::TableBuilder::new(ui)
            .striped(false)
            .resizable(false)
            .id_salt("repository_local_files")
            .column(egui_extras::Column::auto())
            .column(egui_extras::Column::auto())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.add(egui::Label::new("Size").extend());
                });
                header.col(|ui| {
                    ui.add(egui::Label::new("File Name").extend());
                });
            });
        table.body(|mut body| {
            let ui = body.ui_mut();
            ui.set_width(ui.available_width());
            for (name, size) in stored_repo.local.folder.files() {
                body.row(20.0, |mut row| {
                    let (size, color) = readable_size_and_color(size);
                    row.col(|ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&*size).color(color)).extend());
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(&*name).extend());
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
            self.cache.reload_repository(uuid);
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
    let hsl = colors_transform::Hsl::from(
        360.0 - ((((index as f32 / (SIZES.len() - 1) as f32) * 360.0) + 240.0) % 360.0),
        100.0,
        50.0,
    );
    let rgb = hsl.to_rgb();

    (
        Arc::from(format!(
            "{} {}",
            format!("{size:.3}")
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
