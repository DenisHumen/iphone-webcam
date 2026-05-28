//! ClearCam desktop application shell.

mod commands;
mod events;
mod preview;

use std::sync::Arc;

use commands::AppState;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tauri=warn".into()),
        )
        .init();
    let state: AppState = Arc::new(Mutex::new(None));
    tauri::Builder::default()
        .manage(state.clone())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                events::pump(app_handle, state).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_snapshot,
            commands::set_camera,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ClearCam application");
}
