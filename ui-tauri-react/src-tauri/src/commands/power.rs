use crate::hardware::power::{PowerAction, PowerStatus};
use crate::ipc::protocol::{DaemonRequest, DaemonResponse};
use crate::runtime::client;

#[tauri::command]
pub fn get_power_status() -> Result<PowerStatus, String> {
    match client::request(DaemonRequest::GetPowerStatus)? {
        DaemonResponse::PowerStatus { status } => Ok(status),
        DaemonResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected power response: {other:?}")),
    }
}
#[tauri::command]
pub fn set_power_control(action: PowerAction) -> Result<PowerStatus, String> {
    match client::request(DaemonRequest::SetPowerControl { action })? {
        DaemonResponse::PowerStatus { status } => Ok(status),
        DaemonResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected power response: {other:?}")),
    }
}
