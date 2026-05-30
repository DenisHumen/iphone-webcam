import { useEffect, useState } from "react";
import { forgetUsbDevice, listUsbDevices, trustUsbDevice } from "../lib/tauri";
import type { UsbDevice } from "../lib/types";

export function UsbDevicesList() {
  const [devices, setDevices] = useState<UsbDevice[]>([]);
  useEffect(() => {
    let alive = true;
    const tick = async () => {
      try {
        const d = await listUsbDevices();
        if (alive) setDevices(d);
      } catch {
        /* ignore */
      }
    };
    tick();
    const handle = setInterval(tick, 1000);
    return () => {
      alive = false;
      clearInterval(handle);
    };
  }, []);
  if (devices.length === 0) {
    return <p className="text-sm text-slate-500">No USB devices.</p>;
  }
  return (
    <ul className="space-y-1">
      {devices.map((d) => (
        <li key={d.udid} className="flex items-center justify-between text-sm">
          <span className="font-mono">{d.udid.slice(0, 8)}…</span>
          {d.trusted ? (
            <button
              onClick={() => forgetUsbDevice(d.udid)}
              className="text-rose-700 hover:underline"
            >
              Forget
            </button>
          ) : (
            <button
              onClick={() => trustUsbDevice(d.udid)}
              className="text-emerald-700 hover:underline"
            >
              Trust
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}
