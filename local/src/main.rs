#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashMap,
    sync::{mpsc, Arc},
};

use eframe::{
    egui::{self, mutex::Mutex, FontData, Response},
    epaint,
};
use ehttp::Request;
use pitsu_lib::{AccessLevel, Diff, RemoteRepository, RootFolder, ThisUser};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::{
    config::{get_request, Pending, PUBLIC_URL},
    dialogue::{rfd_confirm_response, rfd_ok_dialogue},
};
mod config;
mod dialogue;
mod nerdfonts;
mod pitignore;
fn main() -> anyhow::Result<()> {
    config::setup();
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

struct Repository {
    local: RootFolder,
    remote: RemoteRepository,
    diff: Vec<Diff>,
    pitignore: pitignore::Pitignore,
}

pub struct App {
    cache: RequestCache,
    ppp: f32,
    state: AppState,
}

pub enum AppState {
    Main,
    RepositoryDetails { repository: Arc<RemoteRepository> },
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut new_state = self.header(
                ui,
                ctx,
                frame,
                match self.state {
                    AppState::Main => None,
                    AppState::RepositoryDetails { .. } => Some(AppState::Main),
                },
                match &self.state {
                    AppState::Main => Box::from(
                        |ui: &mut egui::Ui, ctx: &egui::Context, frame: &mut eframe::Frame| {
                            ui.label("Repositories");
                        },
                    ),
                    AppState::RepositoryDetails { repository } => {
                        let repository = Arc::clone(repository);
                        Box::from(
                            move |ui: &mut egui::Ui,
                                  ctx: &egui::Context,
                                  frame: &mut eframe::Frame| {
                                ui.label(format!("{}", repository.name));
                            },
                        )
                    }
                },
            );
            match &self.state {
                AppState::Main => {
                    if let Ok(Some(this)) = self.cache.this_user() {
                        let mut table = egui_extras::TableBuilder::new(ui)
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
                                match self.cache.get_repository(repo.uuid) {
                                    Ok(Some(repository)) => {
                                        body.row(20.0, |mut row| {
                                            row.col(|ui| {
                                                if ui
                                                    .add(
                                                        egui::Button::new(&*repository.name)
                                                            .wrap_mode(egui::TextWrapMode::Extend),
                                                    )
                                                    .clicked()
                                                {
                                                    new_state = Some(AppState::RepositoryDetails {
                                                        repository: Arc::clone(&repository),
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
                                    Ok(None) => {}
                                    Err(e) => {}
                                }
                            }
                        });
                    } else {
                        ui.label("Loading user data...");
                    }
                }
                AppState::RepositoryDetails { repository } => {
                    ui.label(format!("Repository: {}", repository.name));
                    ui.label(format!("UUID: {}", repository.uuid));
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
            cache: RequestCache::default(),
            ppp,
            state: AppState::Main,
        }
    }
    fn header(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        go_back: Option<AppState>,
        label: Box<dyn FnOnce(&mut egui::Ui, &egui::Context, &mut eframe::Frame)>,
    ) -> Option<AppState> {
        let username = match self.cache.this_user() {
            Ok(Some(this)) => Arc::clone(&this.user.username),
            Ok(None) => {
                ui.label("Loading user data...");
                return None;
            }
            Err(e) => {
                ui.label(format!("Error loading user data: {}", e));
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
            label(ui, ctx, frame);
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
                });
            });
        });
        ui.separator();
        new_state
    }
}

#[derive(Default)]
pub struct RequestCache {
    this_user: PendingRequest<ThisUser>,
    repositories: HashMap<Uuid, PendingRequest<RemoteRepository>>,
}

#[derive(Default, Debug)]
enum PendingRequest<T>
where
    T: std::fmt::Debug + Send + Sync + 'static,
{
    #[default]
    Unsent,
    Pending(mpsc::Receiver<Result<Arc<T>, Arc<str>>>),
    Response(Result<Arc<T>, Arc<str>>),
}

type PendingResponse<T> = Result<Option<Arc<T>>, Arc<str>>;

impl RequestCache {
    pub fn this_user(&mut self) -> PendingResponse<ThisUser> {
        let new_state = match &self.this_user {
            PendingRequest::Unsent => {
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
            PendingRequest::Pending(ref pending) => match pending.try_recv() {
                Ok(result) => PendingRequest::Response(result),
                Err(mpsc::TryRecvError::Empty) => {
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Disconnected) => PendingRequest::Response(Err(Arc::from(
                    "Request channel disconnected unexpectedly".to_string(),
                ))),
            },
            PendingRequest::Response(ref result) => {
                return result.clone().map(Some);
            }
        };
        self.this_user = new_state;
        Ok(None)
    }
    pub fn get_repository(&mut self, uuid: Uuid) -> PendingResponse<RemoteRepository> {
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
                PendingRequest::Unsent => {
                    let (sender, receiver) = mpsc::channel();
                    ehttp::fetch(
                        get_request(&format!("{PUBLIC_URL}/api/repository/{uuid}")),
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
}
