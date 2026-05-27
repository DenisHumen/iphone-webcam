import type { SessionStateKind } from "../lib/types";

function label(s: SessionStateKind): string {
  switch (s.kind) {
    case "idle":
      return "Не запущен";
    case "listening":
      return "Ожидание iPhone";
    case "handshaking":
      return "Подключение…";
    case "ready":
      return "Подключено";
    case "reconnecting":
      return "Переподключение…";
    case "closed":
      return `Завершено: ${s.reason}`;
  }
}

function tone(s: SessionStateKind): string {
  switch (s.kind) {
    case "idle":
      return "bg-neutral-700 text-neutral-200";
    case "listening":
    case "handshaking":
      return "bg-amber-700 text-amber-100";
    case "ready":
      return "bg-emerald-700 text-emerald-100";
    case "reconnecting":
      return "bg-orange-700 text-orange-100";
    case "closed":
      return "bg-rose-700 text-rose-100";
  }
}

export default function StatusBadge({ state }: { state: SessionStateKind }) {
  return (
    <span
      className={`inline-flex items-center gap-2 rounded-full px-3 py-1 text-xs font-medium ${tone(state)}`}
    >
      <span className="size-1.5 rounded-full bg-current" />
      {label(state)}
    </span>
  );
}
