import { useState } from "react";
import { runSpeedtest } from "../lib/tauri";
import type { SpeedtestOutcome } from "../lib/types";

export default function SpeedtestPanel() {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<SpeedtestOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleRun() {
    setRunning(true);
    setError(null);
    try {
      const r = await runSpeedtest();
      setResult(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-4 shadow-xl space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-neutral-300">Тест скорости</h2>
        <button
          onClick={handleRun}
          disabled={running}
          className="rounded-lg bg-sky-700 px-3 py-1 text-xs font-medium text-sky-50 hover:bg-sky-600 disabled:opacity-50"
        >
          {running ? "Измеряется…" : "Запустить"}
        </button>
      </div>
      {error && <p className="text-sm text-rose-300">{error}</p>}
      {result && (
        <div className="grid grid-cols-2 gap-2 text-sm">
          <Stat label="Goodput" value={`${result.measurement.goodput_mbps.toFixed(1)} Мбит/с`} />
          <Stat label="RTT" value={`${result.measurement.rtt_ms.toFixed(1)} мс`} />
          <Stat
            label="Рекомендуемый"
            value={`${result.recommended.width}×${result.recommended.height} @ ${result.recommended.fps}fps`}
          />
          <Stat
            label="Формат"
            value={
              result.recommended.format === "raw"
                ? "RAW NV12"
                : `${result.recommended.codec.toUpperCase()} ${(result.recommended.bitrateKbps / 1000).toFixed(0)} Мбит/с`
            }
          />
        </div>
      )}
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-neutral-950 px-3 py-2">
      <p className="text-xs text-neutral-500">{label}</p>
      <p className="font-mono text-neutral-100">{value}</p>
    </div>
  );
}
