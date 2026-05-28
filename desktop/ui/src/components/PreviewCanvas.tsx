import { useEffect, useRef } from "react";
import { onPreviewFrame } from "../lib/tauri";

export default function PreviewCanvas() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      off = await onPreviewFrame(async (f) => {
        const canvas = canvasRef.current;
        if (!canvas || cancelled) return;
        try {
          const bin = atob(f.jpegBase64);
          const buf = new Uint8Array(bin.length);
          for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i);
          const blob = new Blob([buf], { type: "image/jpeg" });
          const bitmap = await createImageBitmap(blob);
          if (cancelled) {
            bitmap.close();
            return;
          }
          if (canvas.width !== bitmap.width) canvas.width = bitmap.width;
          if (canvas.height !== bitmap.height) canvas.height = bitmap.height;
          const ctx = canvas.getContext("2d");
          ctx?.drawImage(bitmap, 0, 0);
          bitmap.close();
        } catch {
          // ignore decode races during teardown
        }
      });
    })();
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);

  return (
    <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-4 shadow-xl">
      <h2 className="text-sm font-medium text-neutral-300 mb-2">Превью</h2>
      <canvas
        ref={canvasRef}
        className="w-full rounded-lg bg-black"
        style={{ aspectRatio: "16/9" }}
      />
    </section>
  );
}
