import { useEffect, useState } from "react";
import DeviceCard from "./components/DeviceCard";
import ErrorBanner from "./components/ErrorBanner";
import PreviewCanvas from "./components/PreviewCanvas";
import ServerCard from "./components/ServerCard";
import StatusBadge from "./components/StatusBadge";
import {
  getSnapshot,
  onClosed,
  onDevice,
  onSessionState,
  onTelemetry,
  startServer,
  stopServer,
} from "./lib/tauri";
import type { DeviceSnapshot, QrPayloadOut, SessionStateKind, Telemetry } from "./lib/types";

export default function App() {
  const [qr, setQr] = useState<QrPayloadOut | null>(null);
  const [state, setState] = useState<SessionStateKind>({ kind: "idle" });
  const [device, setDevice] = useState<DeviceSnapshot | null>(null);
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);
  const [activeCameraId, setActiveCameraId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const off: Array<() => void> = [];
    let cancelled = false;
    (async () => {
      try {
        const snap = await getSnapshot();
        if (cancelled) return;
        setState(snap.state);
        setDevice(snap.device);
        setTelemetry(snap.last_telemetry);
        off.push(await onSessionState((s) => setState(s)));
        off.push(
          await onDevice((d) => {
            setDevice(d);
            // first time we see cameras, pick the first as active.
            if (d.cameras.length > 0 && !activeCameraId) {
              setActiveCameraId(d.cameras[0].id);
            }
          }),
        );
        off.push(await onTelemetry((t) => setTelemetry(t)));
        off.push(
          await onClosed((reason) => {
            setError(`Сессия закрыта: ${reason}`);
            setDevice(null);
            setTelemetry(null);
            setActiveCameraId(null);
          }),
        );
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
      for (const f of off) f();
    };
  }, [activeCameraId]);

  async function handleStart() {
    setError(null);
    try {
      const out = await startServer();
      setQr(out);
    } catch (e) {
      setError(String(e));
    }
  }
  async function handleStop() {
    try {
      await stopServer();
      setQr(null);
      setState({ kind: "idle" });
      setDevice(null);
      setTelemetry(null);
      setActiveCameraId(null);
    } catch (e) {
      setError(String(e));
    }
  }

  const showServer = qr && state.kind !== "ready";
  const showDevice = device && state.kind === "ready";

  return (
    <main className="min-h-screen bg-neutral-950 text-neutral-100 px-6 py-10">
      <div className="mx-auto w-full max-w-3xl space-y-6">
        <header className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold">ClearCam</h1>
            <p className="text-sm text-neutral-400">iPhone-as-webcam · Phase 2</p>
          </div>
          <StatusBadge state={state} />
        </header>

        {error && <ErrorBanner message={error} onDismiss={() => setError(null)} />}

        {!qr && (
          <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-6 text-center">
            <p className="text-neutral-300 mb-4">Запустите сервер и отсканируйте QR с iPhone.</p>
            <button
              onClick={handleStart}
              className="rounded-lg bg-emerald-700 px-5 py-2 text-sm font-medium text-emerald-50 hover:bg-emerald-600"
            >
              Запустить сервер
            </button>
          </section>
        )}

        {showServer && qr && <ServerCard qr={qr} onStop={handleStop} />}
        {showDevice && device && (
          <>
            <PreviewCanvas />
            <DeviceCard
              device={device}
              telemetry={telemetry}
              activeCameraId={activeCameraId}
              onCameraChange={setActiveCameraId}
            />
          </>
        )}
      </div>
    </main>
  );
}
