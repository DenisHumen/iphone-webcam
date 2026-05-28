import type { CameraEntry } from "../lib/types";
import { setCamera } from "../lib/tauri";

export default function CameraPicker({
  cameras,
  activeId,
  onChange,
}: {
  cameras: CameraEntry[];
  activeId: string | null;
  onChange: (id: string) => void;
}) {
  if (cameras.length === 0) return null;
  return (
    <div className="flex gap-2 flex-wrap">
      {cameras.map((c) => (
        <button
          key={c.id}
          onClick={async () => {
            try {
              await setCamera(c.id);
              onChange(c.id);
            } catch (e) {
              console.warn("set_camera failed", e);
            }
          }}
          className={`rounded-full px-3 py-1 text-sm transition-colors ${
            activeId === c.id
              ? "bg-emerald-700 text-emerald-50"
              : "bg-neutral-800 text-neutral-200 hover:bg-neutral-700"
          }`}
        >
          {c.name}
        </button>
      ))}
    </div>
  );
}
