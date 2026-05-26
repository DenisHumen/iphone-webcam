//! Device & camera messages (§6.2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    Unknown,
    Unplugged,
    Charging,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub model: String,
    pub os_ver: String,
    /// 0.0..=1.0
    pub battery_level: f64,
    pub battery_state: BatteryState,
    pub thermal_state: ThermalState,
    pub usb3_capable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraPosition {
    Front,
    Back,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraEntry {
    pub id: String,
    pub name: String,
    pub position: CameraPosition,
    pub max_width: u16,
    pub max_height: u16,
    pub max_fps: u16,
    /// Supported pixel formats as wire strings (`"nv12"`, `"hevc"`, `"h264"`).
    pub supported_formats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraList {
    pub cameras: Vec<CameraEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCamera {
    pub camera_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraState {
    pub active_camera_id: String,
    /// Mode string, e.g. `"1920x1080@30 hevc"`. Free-form for diagnostics.
    pub applied_format: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_info_round_trip() {
        let d = DeviceInfo {
            model: "iPhone15,3".into(),
            os_ver: "iOS 18.0".into(),
            battery_level: 0.82,
            battery_state: BatteryState::Unplugged,
            thermal_state: ThermalState::Nominal,
            usb3_capable: true,
        };
        let s = serde_json::to_string(&d).unwrap();
        let back: DeviceInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(back, d);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["batteryLevel"], 0.82);
        assert_eq!(v["batteryState"], "unplugged");
        assert_eq!(v["usb3Capable"], true);
    }

    #[test]
    fn camera_list_round_trip() {
        let list = CameraList {
            cameras: vec![CameraEntry {
                id: "wide".into(),
                name: "Wide".into(),
                position: CameraPosition::Back,
                max_width: 3840,
                max_height: 2160,
                max_fps: 60,
                supported_formats: vec!["nv12".into(), "hevc".into(), "h264".into()],
            }],
        };
        let s = serde_json::to_string(&list).unwrap();
        let back: CameraList = serde_json::from_str(&s).unwrap();
        assert_eq!(back, list);
    }
}
