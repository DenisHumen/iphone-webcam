export default function ErrorBanner({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  return (
    <div className="flex items-center justify-between rounded-xl border border-rose-900/60 bg-rose-950/40 px-4 py-3 text-sm text-rose-200">
      <span>{message}</span>
      <button onClick={onDismiss} className="text-rose-300 hover:text-rose-100">
        ×
      </button>
    </div>
  );
}
