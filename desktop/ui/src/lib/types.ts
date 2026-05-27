export type SessionStateKind =
  | { kind: "idle" }
  | { kind: "listening"; control_port: number; media_port: number }
  | { kind: "handshaking" }
  | { kind: "ready" }
  | { kind: "reconnecting" }
  | { kind: "closed"; reason: string };

export type ThermalState = "nominal" | "fair" | "serious" | "critical";
export type BatteryState = "unknown" | "unplugged" | "charging" | "full";

export type CameraEntry = {
  id: string;
  name: string;
  position: "front" | "back";
  maxWidth: number;
  maxHeight: number;
  maxFps: number;
  supportedFormats: string[];
};

export type DeviceSnapshot = {
  model: string;
  os_ver: string;
  usb3_capable: boolean;
  battery_level: number;
  cameras: CameraEntry[];
};

export type Telemetry = {
  tsUsec: number;
  batteryLevel: number;
  batteryState: BatteryState;
  thermalState: ThermalState;
  sentBitrateKbps: number;
  encFps: number;
  captureFps: number;
  queueDepth: number;
  dropCount: number;
};

export type SessionSnapshot = {
  state: SessionStateKind;
  device: DeviceSnapshot | null;
  last_telemetry: Telemetry | null;
};

export type QrPayloadOut = {
  v: number;
  host: string;
  cport: number;
  mport: number;
  token: string;
  svg: string;
};
