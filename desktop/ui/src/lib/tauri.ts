import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DeviceSnapshot,
  QrPayloadOut,
  SessionSnapshot,
  SessionStateKind,
  Telemetry,
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
