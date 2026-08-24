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

macro_rules! connector_command {
    ($name:ident, $method:ident, $result:ty) => {
        #[tauri::command]
        fn $name(
            envelope: CommandEnvelope<serde_json::Value>,
            state: tauri::State<'_, thalassaops::app::AppState>,
        ) -> thalassaops::app::IpcResult<$result> {
            state.$method(envelope)
        }
    };
}
connector_command!(
    connector_list,
    connector_list,
    Vec<thalassaops::connectors::ConnectorSummary>
);
#[tauri::command]
async fn connector_test(
    envelope: CommandEnvelope<serde_json::Value>,
    state: tauri::State<'_, thalassaops::app::AppState>,
) -> Result<thalassaops::app::IpcResult<thalassaops::connectors::ConnectorSummary>, String> {
    Ok(state.inner().clone().connector_test(envelope).await)
}
#[tauri::command]
async fn kubernetes_inventory(
    envelope: CommandEnvelope<serde_json::Value>,
    state: tauri::State<'_, thalassaops::app::AppState>,
) -> Result<thalassaops::app::IpcResult<thalassaops::kubernetes::KubernetesInventory>, String> {
    Ok(state.inner().clone().kubernetes_inventory(envelope).await)
}
#[tauri::command]
async fn kubernetes_pod_logs(
    envelope: CommandEnvelope<serde_json::Value>,
    state: tauri::State<'_, thalassaops::app::AppState>,
) -> Result<thalassaops::app::IpcResult<String>, String> {
    Ok(state.inner().clone().kubernetes_pod_logs(envelope).await)
}
#[tauri::command]
async fn kubernetes_pod_events(
    envelope: CommandEnvelope<serde_json::Value>,
    state: tauri::State<'_, thalassaops::app::AppState>,
) -> Result<thalassaops::app::IpcResult<Vec<thalassaops::kubernetes::KubernetesEvent>>, String> {
    Ok(state.inner().clone().kubernetes_pod_events(envelope).await)
}
connector_command!(
    connector_add,
    connector_add,
    thalassaops::connectors::ConnectorSummary
);
connector_command!(
    connector_enable,
    connector_enable,
    thalassaops::connectors::ConnectorSummary
);
connector_command!(
    connector_disable,
    connector_disable,
    thalassaops::connectors::ConnectorSummary
);
connector_command!(connector_remove, connector_remove, serde_json::Value);
connector_command!(
    connector_diagnose,
    connector_diagnose,
    thalassaops::connectors::ConnectorDiagnostics
);

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
        .invoke_handler(tauri::generate_handler![
            system_health,
            system_context,
            connector_list,
            connector_add,
            connector_enable,
            connector_disable,
            connector_remove,
            connector_test,
            connector_diagnose,
            kubernetes_inventory,
            kubernetes_pod_logs,
            kubernetes_pod_events
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ThalassaOps");
}
