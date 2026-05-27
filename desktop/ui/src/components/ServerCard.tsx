import type { QrPayloadOut } from "../lib/types";

export default function ServerCard({ qr, onStop }: { qr: QrPayloadOut; onStop: () => void }) {
  return (
    <section className="rounded-2xl border border-neutral-800 bg-neutral-900 p-6 shadow-xl">
      <h2 className="text-lg font-semibold mb-4">Отсканируйте QR с iPhone</h2>
      <div className="flex flex-col items-center gap-4">
        <div
          className="size-72 rounded-xl bg-white p-3"
          dangerouslySetInnerHTML={{ __html: qr.svg }}
        />
        <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm text-neutral-300">
          <dt className="text-neutral-500">Хост</dt>
          <dd>{qr.host}</dd>
          <dt className="text-neutral-500">Control</dt>
          <dd>{qr.cport}</dd>
          <dt className="text-neutral-500">Media</dt>
          <dd>{qr.mport}</dd>
        </dl>
        <button
          onClick={onStop}
          className="mt-2 rounded-lg border border-neutral-700 px-4 py-2 text-sm text-neutral-200 hover:bg-neutral-800"
        >
          Остановить
        </button>
      </div>
    </section>
  );
}
