// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};
use tauri_plugin_clipboard::Clipboard;
use tokio_util::sync::CancellationToken;
// use tauri_plugin_log::{Target, TargetKind};

use apicize_lib::{
    models::{apicize::ApicizeExecution, ApicizeSettings, Workspace},
    oauth2_client_tokens::{clear_all_oauth2_tokens, clear_oauth2_token},
};
use tauri::{Emitter, Manager, State};

use std::sync::{Mutex, OnceLock};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        // .plugin(
        //     tauri_plugin_log::Builder::new()
        //         .targets([
        //             Target::new(TargetKind::Stdout),
        //             // Target::new(TargetKind::LogDir { file_name: None }),
        //             // Target::new(TargetKind::Webview),                ])
        //         ])
        //         .build(),
        // )
        // .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            save_workspace,
            open_settings,
            save_settings,
            run_request,
            cancel_request,
            clear_cached_authorization,
            // get_environment_variables,
            is_release_mode,
            get_clipboard_image_base64,
        ])
        .setup(|app| {
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

                let ctrl_n_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyN);
                let ctrl_o_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyO);
                let ctrl_s_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyS);
                let ctrl_shift_s_shortcut =
                    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyS);

                let handle = app.handle().clone();

                let shortcut_builder = tauri_plugin_global_shortcut::Builder::new();
                app.handle().plugin(
                    shortcut_builder
                        .with_handler(move |_app, shortcut, _event| {
                            let focused = _app
                                .get_webview_window("main")
                                .unwrap()
                                .is_focused()
                                .unwrap();
                            if focused {
                                if shortcut == &ctrl_n_shortcut {
                                    handle.emit("shortcut", "new").unwrap()
                                } else if shortcut == &ctrl_o_shortcut {
                                    handle.emit("shortcut", "open").unwrap()
                                } else if shortcut == &ctrl_s_shortcut {
                                    handle.emit("shortcut", "save").unwrap()
                                } else if shortcut == &ctrl_shift_s_shortcut {
                                    handle.emit("shortcut", "saveAs").unwrap()
                                }
                            }
                        })
                        .build(),
                )?;

                app.global_shortcut().register(ctrl_n_shortcut)?;
                app.global_shortcut().register(ctrl_o_shortcut)?;
                app.global_shortcut().register(ctrl_s_shortcut)?;
                app.global_shortcut().register(ctrl_shift_s_shortcut)?;
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running Apicize");
}

#[tauri::command]
async fn open_workspace(path: String) -> Result<Workspace, String> {
    match Workspace::open_from_file(&PathBuf::from(path), None) {
        Ok(workspace) => {
            clear_all_oauth2_tokens().await;
            Ok(workspace)
        }
        Err(err) => Err(format!("{}", err.error)),
    }
}

#[tauri::command]
fn save_workspace(workspace: Workspace, path: String) -> Result<(), String> {
    match workspace.save(&PathBuf::from(path)) {
        Ok(..) => Ok(()),
        Err(err) => Err(format!("{}", err.error)),
    }
}

#[tauri::command]
async fn open_settings() -> Result<ApicizeSettings, String> {
    match ApicizeSettings::open() {
        Ok(result) => {
            clear_all_oauth2_tokens().await;
            Ok(result.data)
        }
        Err(err) => Err(format!("{}", err.error)),
    }
}

#[tauri::command]
async fn save_settings(settings: ApicizeSettings) -> Result<(), String> {
    match settings.save() {
        Ok(..) => Ok(()),
        Err(err) => Err(format!("{}", err.error)),
    }
}

fn cancellation_tokens() -> &'static Mutex<HashMap<String, CancellationToken>> {
    static TOKENS: OnceLock<Mutex<HashMap<String, CancellationToken>>> = OnceLock::new();
    TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
async fn run_request(workspace: Workspace, request_id: String) -> Result<ApicizeExecution, String> {

    let arc_workspace = Arc::new(workspace);
    let arc_test_started = Arc::new(Instant::now());
    let cancellation = CancellationToken::new();
    {
        cancellation_tokens()
            .lock()
            .unwrap()
            .insert(request_id.clone(), cancellation.clone());
    }

    let response = Workspace::run(
        arc_workspace.clone(),
        &request_id,
        Some(cancellation),
        arc_test_started,
    )
    .await;

    cancellation_tokens().lock().unwrap().remove(&request_id);

    match response {
        Ok(result) => Ok(result),
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
async fn cancel_request(request_id: String) {
    let tokens = cancellation_tokens().lock().unwrap();
    if let Some(token) = tokens.get(&request_id) {
        token.cancel()
    }
}

#[tauri::command]
async fn clear_cached_authorization(authorization_id: String) -> bool {
    clear_oauth2_token(authorization_id).await
}

#[tauri::command]
fn get_clipboard_image_base64(clipboard: State<Clipboard>) -> Result<String, String> {
    match clipboard.has_image() {
        Ok(has_image) => {
            if has_image {
                clipboard.read_image_base64()
            } else {
                Err(String::from("Clipboard does not contain an image"))
            }
        }
        Err(msg) => Err(msg),
    }
}

// #[tauri::command]
// fn get_environment_variables() -> Vec<(String, String)> {
//     std::env::vars().into_iter().map1(|e| e).collect()
// }

#[tauri::command]
fn is_release_mode() -> bool {
    ! cfg!(debug_assertions)
}
