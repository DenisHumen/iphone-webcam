import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DeviceSnapshot,
  PreviewFrame,
  QrPayloadOut,
  SessionSnapshot,
  SessionStateKind,
  SpeedtestOutcome,
  Telemetry,
  TransportSource,
  TrustRequest,
  UsbDevice,
} from "./types";

export async function startServer(): Promise<QrPayloadOut> {
  return invoke("start_server");
}

export async function stopServer(): Promise<void> {
  return invoke("stop_server");
}

export async function getSnapshot(): Promise<SessionSnapshot> {
  return invoke("get_snapshot");
}

export async function onSessionState(cb: (s: SessionStateKind) => void): Promise<UnlistenFn> {
  return listen<SessionStateKind>("session://state", (e) => cb(e.payload));
}

export async function onDevice(cb: (d: DeviceSnapshot) => void): Promise<UnlistenFn> {
  return listen<DeviceSnapshot>("session://device", (e) => cb(e.payload));
}

export async function onTelemetry(cb: (t: Telemetry) => void): Promise<UnlistenFn> {
  return listen<Telemetry>("session://telemetry", (e) => cb(e.payload));
}

export async function onClosed(cb: (reason: string) => void): Promise<UnlistenFn> {
  return listen<string>("session://closed", (e) => cb(e.payload));
}

export async function onPreviewFrame(cb: (f: PreviewFrame) => void): Promise<UnlistenFn> {
  return listen<PreviewFrame>("session://preview", (e) => cb(e.payload));
}

export async function setCamera(id: string): Promise<void> {
  return invoke("set_camera", { id });
}

export async function runSpeedtest(): Promise<SpeedtestOutcome> {
  return invoke("run_speedtest");
}

export async function listUsbDevices(): Promise<UsbDevice[]> {
  return invoke("list_usb_devices");
}

export async function trustUsbDevice(udid: string): Promise<void> {
  return invoke("trust_usb_device", { udid });
}

export async function forgetUsbDevice(udid: string): Promise<void> {
  return invoke("forget_usb_device", { udid });
}

export async function onTransportChanged(cb: (src: TransportSource) => void): Promise<UnlistenFn> {
  return listen<TransportSource>("session://transport_changed", (e) => cb(e.payload));
}

export async function onTrustRequest(cb: (req: TrustRequest) => void): Promise<UnlistenFn> {
  return listen<TrustRequest>("session://usb_trust_request", (e) => cb(e.payload));
}
