import { useEffect, useState } from "react";
import { onTrustRequest, trustUsbDevice } from "../lib/tauri";
import type { TrustRequest } from "../lib/types";

export function TrustDialog() {
  const [req, setReq] = useState<TrustRequest | null>(null);
  useEffect(() => {
    const off = onTrustRequest(setReq);
    return () => {
      off.then((u) => u());
    };
  }, []);
  if (!req) return null;
  return (
    <div
      role="dialog"
      aria-modal
      className="fixed inset-0 bg-black/40 flex items-center justify-center"
    >
      <div className="bg-white rounded-2xl p-6 max-w-md shadow-xl">
        <h2 className="text-lg font-semibold">Trust this iPhone?</h2>
        <p className="text-sm text-slate-600 mt-2">
          UDID{" "}
          <span className="font-mono">{req.udid.slice(0, 12)}…</span> is
          connected via USB. Allow it to stream to this computer?
        </p>
        <div className="flex justify-end gap-2 mt-4">
          <button
            onClick={() => setReq(null)}
            className="px-3 py-1.5 rounded-md text-slate-700 hover:bg-slate-100"
          >
            Cancel
          </button>
          <button
            onClick={async () => {
              await trustUsbDevice(req.udid);
              setReq(null);
            }}
            className="px-3 py-1.5 rounded-md bg-emerald-600 text-white hover:bg-emerald-700"
          >
            Trust
          </button>
        </div>
      </div>
    </div>
  );
}
