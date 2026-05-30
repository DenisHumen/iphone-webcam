//! ClearCam desktop application shell.

mod commands;
mod events;
mod preview;

use std::sync::Arc;

use app::PairingStore;
use commands::{AppState, UsbState};
use tokio::sync::Mutex;
use transport::usb::{LoopbackConductor, UsbConductor};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tauri=warn".into()),
        )
        .init();
    let state: AppState = Arc::new(Mutex::new(None));

    // Build the USB state synchronously via the Tauri async runtime before
    // the builder runs. This is the supported approach in Tauri 2.
    let usb_state = tauri::async_runtime::block_on(async {
        let pairing = Arc::new(
            PairingStore::open_default()
                .await
                .expect("PairingStore::open_default failed"),
        );
        let conductor: Arc<dyn UsbConductor + Send + Sync> = build_conductor();
        UsbState { conductor, pairing }
    });

    tauri::Builder::default()
        .manage(state.clone())
        .manage(usb_state.clone())
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
            commands::list_usb_devices,
            commands::trust_usb_device,
            commands::forget_usb_device,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ClearCam application");
}

#[cfg(feature = "usb-idevice")]
fn build_conductor() -> Arc<dyn UsbConductor + Send + Sync> {
    Arc::new(transport::usb::IdeviceConductor::new().expect("IdeviceConductor"))
}

#[cfg(not(feature = "usb-idevice"))]
fn build_conductor() -> Arc<dyn UsbConductor + Send + Sync> {
    Arc::new(LoopbackConductor::new())
}
