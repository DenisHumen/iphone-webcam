import type { DeviceSnapshot, Telemetry } from "../lib/types";
import CameraPicker from "./CameraPicker";

function fmtPct(x: number): string {
  return `${Math.round(x * 100)}%`;
}

function thermalLabel(t: Telemetry["thermalState"]): string {
  switch (t) {
    case "nominal":
      return "Норма";
    case "fair":
      return "Тепло";
    case "serious":
      return "Жарко";
    case "critical":
      return "Критично";
  }
}

export default function DeviceCard({
  device,
  telemetry,
  activeCameraId,
  onCameraChange,
}: {
  device: DeviceSnapshot;
  telemetry: Telemetry | null;
  activeCameraId: string | null;
  onCameraChange: (id: string) => void;
}) {
  const battery = telemetry ? telemetry.batteryLevel : device.battery_level;
  return (
    <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-6 shadow-xl space-y-5">
      <header className="flex items-baseline justify-between">
        <div>
          <h2 className="text-lg font-semibold">{device.model}</h2>
          <p className="text-sm text-neutral-400">{device.os_ver}</p>
        </div>
        {device.usb3_capable && (
          <span className="rounded-full bg-sky-900 px-2 py-0.5 text-xs text-sky-200">USB 3</span>
        )}
      </header>

      <CameraPicker cameras={device.cameras} activeId={activeCameraId} onChange={onCameraChange} />

      <div className="grid grid-cols-3 gap-3 text-sm">
        <Metric label="Батарея" value={fmtPct(battery)} />
        <Metric label="Тепло" value={telemetry ? thermalLabel(telemetry.thermalState) : "—"} />
        <Metric label="Bitrate" value={telemetry ? `${telemetry.sentBitrateKbps} kbps` : "—"} />
      </div>

      <div>
        <h3 className="text-sm font-medium text-neutral-300 mb-2">Камеры</h3>
        <ul className="space-y-1 text-sm">
          {device.cameras.map((c) => (
            <li key={c.id} className="flex justify-between text-neutral-300">
              <span>
                {c.name} <span className="text-neutral-500">({c.position})</span>
              </span>
              <span className="text-neutral-500">
                {c.maxWidth}×{c.maxHeight} @ {c.maxFps}fps
              </span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl bg-neutral-950 p-3">
      <p className="text-xs text-neutral-500">{label}</p>
      <p className="font-mono text-base text-neutral-100">{value}</p>
    </div>
  );
}
