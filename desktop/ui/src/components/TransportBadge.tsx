import type { TransportSource } from "../lib/types";

interface Props {
  source: TransportSource;
}

export function TransportBadge({ source }: Props) {
  const label = source === "usb" ? "USB" : "Wi-Fi";
  const cls =
    source === "usb"
      ? "bg-emerald-100 text-emerald-900"
      : "bg-sky-100 text-sky-900";
  return (
    <span
      role="status"
      aria-label={`Active transport: ${label}`}
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${cls}`}
    >
      <span aria-hidden>{source === "usb" ? "\u{1F50C}" : "\u{1F4F6}"}</span>
      {label}
    </span>
  );
}
