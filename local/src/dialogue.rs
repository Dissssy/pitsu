use std::sync::Arc;

use anyhow::Result;
use clipboard_rs::Clipboard as _;
use eframe::{
    egui::{self, mutex::Mutex, ViewportBuilder},
    NativeOptions,
};

use crate::config::UserInfo;

pub fn rfd_confirm_response(query: &str) -> Result<bool> {
    let response = rfd::MessageDialog::new()
        .set_title("Confirmation")
        .set_description(query)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();
    match response {
        rfd::MessageDialogResult::Yes => Ok(true),
        rfd::MessageDialogResult::No => Ok(false),
        _ => Err(anyhow::anyhow!("No response provided")),
    }
}

pub fn rfd_ok_dialogue(query: &str) -> Result<()> {
    rfd::MessageDialog::new()
        .set_title("Information")
        .set_description(query)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    Ok(())
}

#[allow(unused)]
pub fn get_api_key() -> Result<Arc<str>> {
    let api_key = Arc::new(Mutex::new(String::new()));
    let user_info = Arc::new(Mutex::new(None));
    let api_query = Arc::new(Mutex::new(None));
    {
        let api_key = Arc::clone(&api_key);
        let api_query = Arc::clone(&api_query);
        let user_info = Arc::clone(&user_info);
        let interval = std::time::Duration::from_millis(100);
        let mut last_check = std::time::Instant::now();
        let mut error = None;
        eframe::run_simple_native(
            "Enter API Key",
            NativeOptions {
                viewport: ViewportBuilder::default()
                    .with_icon(Arc::clone(&crate::config::icons::WINDOW_ICON))
                    .with_resizable(false)
                    .with_inner_size(egui::vec2(360.0, 36.0)),
                ..Default::default()
            },
            move |ctx, _frame| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let query_is_some = {
                        let api_query = api_query.lock();
                        api_query.is_some()
                    };
                    let mut api_key = api_key.lock();
                    ui.horizontal(|ui| {
                        ui.add_enabled(!query_is_some, egui::TextEdit::singleline(&mut *api_key));
                        if ui.add_enabled(!query_is_some, egui::Button::new("Submit")).clicked() {
                            let mut query = api_query.lock();
                            *query = Some(UserInfo::get(api_key.clone().into()));
                        }
                        if query_is_some {
                            ui.spinner();
                        }
                        if last_check.elapsed() >= interval {
                            last_check = std::time::Instant::now();
                            let mut query = api_query.lock();
                            if let Some(pending) = &mut *query {
                                match pending.try_ready() {
                                    Ok(Some(info)) => {
                                        let mut user_info_lock = user_info.lock();
                                        *user_info_lock = Some(info);
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                    Err(e) => {
                                        error = Some(format!("Failed to get API key: {e}"));
                                        *query = None;
                                    }
                                    Ok(None) => {}
                                }
                            }
                        }
                    });
                    if let Some(e) = &error {
                        ui.label(egui::RichText::new(e).color(egui::Color32::RED));
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        360.0 + (if query_is_some { 24.0 } else { 0.0 }),
                        36.0 + (if error.is_some() { 14.0 } else { 0.0 }),
                    )));
                });
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to run API key input popup: {}", e))?;
    }
    let api_key = api_key.lock().clone();
    let user_info = user_info.lock().clone();
    if api_key.is_empty() {
        return Err(anyhow::anyhow!("No API key provided"));
    }
    if user_info.is_none() {
        return Err(anyhow::anyhow!("No user info provided"));
    }
    Ok(Arc::from(api_key))
}

pub fn rfd_panic_dialogue(info: &std::panic::PanicHookInfo) {
    log::error!(
        "{} PANIC: {} at {}",
        crate::config::CONFIG.username(),
        info.payload().downcast_ref::<&str>().unwrap_or(&"No payload"),
        info.location().map_or("unknown location".into(), |l| l.to_string())
    );
    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .unwrap_or(&"NO PANIC PAYLOAD PROVIDED")
        .to_string();
    let location = info.location().map_or("unknown location".into(), |l| l.to_string());
    let text = format!("PITSU has encountered a panic:\n{payload}\n\nLocation:\n{location}");

    let result = rfd::MessageDialog::new()
        // .set_buttons(rfd::MessageButtons::OkCancelCustom(
        //     String::from("Copy"),
        //     String::from("Ignore"),
        // ))
        .set_buttons(rfd::MessageButtons::YesNo)
        .set_title("Panic Occurred")
        .set_description(format!("{text}\n\nWould you like to copy this text to your clipboard?"))
        .show();

    if result == rfd::MessageDialogResult::Yes {
        let ctx = clipboard_rs::ClipboardContext::new().expect("Failed to create clipboard context");
        if let Err(e) = ctx.set_text(text.clone()) {
            eprintln!("Failed to copy text to clipboard: {e}");
        }
    }
}

// pub mod in_thread {
//     use std::sync::mpsc::Receiver;

//     use super::*;

//     pub fn confirm_response(query: &str) -> Receiver<Result<bool>> {
//         let (sender, receiver) = std::sync::mpsc::channel();
//         let query = query.to_string();
//         std::thread::spawn(move || {
//             sender
//                 .send(super::rfd_confirm_response(&query))
//                 .unwrap_or_else(|e| {
//                     eprintln!("Failed to send confirmation response: {e}");
//                 });
//         });
//         receiver
//     }
// }
