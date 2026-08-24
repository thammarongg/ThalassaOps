use std::fs;
use tauri::Manager;
use thalassa_ipc::CommandEnvelope;

#[tauri::command]
fn system_health(
    envelope: CommandEnvelope<serde_json::Value>,
    state: tauri::State<'_, thalassaops::app::AppState>,
) -> thalassaops::app::IpcResult<thalassaops::app::HealthResponse> {
    state.health(envelope)
}

#[tauri::command]
fn system_context(
    envelope: CommandEnvelope<serde_json::Value>,
    state: tauri::State<'_, thalassaops::app::AppState>,
) -> thalassaops::app::IpcResult<thalassaops::app::ContextResponse> {
    state.context(envelope)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            app.manage(thalassaops::app::AppState::open(
                data_dir.join("thalassaops.sqlite"),
            )?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![system_health, system_context])
        .run(tauri::generate_context!())
        .expect("failed to run ThalassaOps");
}
