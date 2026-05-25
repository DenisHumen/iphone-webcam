# 05 — Десктоп-приложение (Rust core + Tauri UI)

Связано: [02-architecture.md](02-architecture.md), [03-protocol.md](03-protocol.md),
[08-project-structure.md](08-project-structure.md), [09-tech-stack.md](09-tech-stack.md).

## 1. Назначение

Принять поток с iPhone, привести к каноническому NV12, опубликовать как системную виртуальную камеру,
показать превью и панель управления, запускать тест скорости и адаптацию.

## 2. Состав

```
desktop/
  core/        (Rust-крейты: вся логика, без платформенного UI)
  src-tauri/   (Tauri-обвязка: команды/события, окно, трей)
  ui/          (фронтенд: TypeScript + React + Vite + Tailwind)
  sinks/       (платформенные стоки виртуальной камеры)
    macos-extension/   (Swift CMIO Camera Extension + container glue)
    windows-mf/        (C++/Rust обёртка MFCreateVirtualCamera)
    linux-v4l2/        (запись в /dev/videoN через v4l2loopback)
```

Точная раскладка крейтов — в [08-project-structure.md](08-project-structure.md).

## 3. Rust-ядро: модули (крейты)

- **`ccp-protocol`** — типы сообщений control (serde), бинарный media-заголовок (кодек/декодек),
  версии/возможности. Без I/O — чистые типы и (де)сериализация. Переиспользуется в тестах и (концептуально)
  как референс для Swift-стороны.
- **`transport`** — трейт `Transport` → `(ControlStream, MediaStream, PeerInfo)`. Реализации:
  `wifi` (tokio TCP server + mDNS + опц. TLS), `usb` (libusbmuxd-обёртка: enumerate + connect к порту
  устройства). Скрывает «кто дозванивается».
- **`session`** (ControlPlane/SessionManager) — actor, владеющий состоянием сессии: устройство, камеры,
  активный режим, телеметрия. Принимает команды (от UI и AdaptiveEngine), шлёт их пиру, публикует
  состояние наверх. Источник правды.
- **`adaptive`** — SpeedTest + выбор/коррекция режима. См. [06](06-adaptive-engine-and-speedtest.md).
- **`mediapipeline`** — приём media-кадров; декод (адаптер) или raw-passthrough; нормализация в NV12;
  минимальный джиттер-буфер с **drop-oldest**; раздача в `FrameSink`-потребители. Тайминг по `ptsUsec`.
- **`sink`** — трейт `FrameSink { start(format); submit(&Frame); stop() }`. Реализации: virtual camera
  (платформенные), preview, recorder.
- **`decode`** — трейт `Decoder` (HEVC/H.264 → NV12). macOS: VideoToolbox (через swift/objc bridge или
  `core-foundation`/`coremedia` crates). Кросс-платформенный fallback: ffmpeg (`ffmpeg-next`).
- **`app`** — сборка всего: запуск задач tokio, конфиг, логирование (`tracing`), graceful shutdown.

## 4. Tauri-обвязка (`src-tauri`)

- **Commands (UI→core):** `list_devices`, `connect(device|qrPayload)`, `disconnect`, `start`, `stop`,
  `set_camera(id)`, `set_mode(mode)`, `run_speedtest`, `start_recording(path)`, `stop_recording`,
  `get_state`.
- **Events (core→UI):** `state_changed`, `telemetry`, `speedtest_progress`, `speedtest_result`,
  `preview_frame` (или превью через отдельный канал — см. §7), `error`, `device_discovered`.
- Tauri хранит handle к ядру (запущенным tokio-задачам) в `State`. Команды — тонкие, вся логика в ядре.

## 5. Генерация QR (привязка)

Десктоп — server на Wi-Fi: генерирует `token`, формирует payload (см. [03](03-protocol.md) §3.2),
рендерит QR в UI (крейт `qrcode` → SVG/PNG в UI). Показывает `host:cport/mport`. Также анонс mDNS.

## 6. Стоки виртуальной камеры (VirtualCameraSink)

Общий трейт `FrameSink`; платформенные реализации:

- **macOS — CMIO Camera Extension** (приоритет v1):
  - Отдельный таргет **System Extension** (Swift), встроенный в контейнер-приложение. Регистрирует
    устройство-камеру в системе (CoreMediaIO). Современный путь (macOS 12.3+), заменяет DAL-плагины.
  - Ядро (Rust/контейнер) передаёт кадры расширению через **CMIO sink stream** (расширение «тянет»/
    принимает кадры). Транспорт ядро↔расширение: через контейнер-приложение (XPC/`IOSurface` shared
    memory) — расширение изолировано и не имеет сети.
  - Требует подписи Developer ID + нотаризации; пользователь разрешает System Extension при установке.
  - Формат подачи: NV12/`'420v'`/BGRA — согласуем с тем, что ждёт CMIO; конверсия в `mediapipeline`.
- **Windows — Media Foundation** (фаза 2): `MFCreateVirtualCamera` (Win11 22000+), кадры через
  `IMFMediaSource`. Видна и MF-, и DirectShow-приложениям.
- **Linux — v4l2loopback** (фаза 3): запись кадров (NV12 или YUYV) в `/dev/videoN`, созданное модулем
  v4l2loopback. Простейший из трёх.

Подробности API и подводные камни — в [09-tech-stack.md](09-tech-stack.md) и [12-decisions-log.md](12-decisions-log.md).

## 7. Превью в UI

Видео в WebView напрямую тянуть дорого. Варианты (выбран **A**):
- **A (v1):** `mediapipeline` отдаёт downscaled-кадры (например ≤720p, ≤30fps), конверсия в RGBA/JPEG,
  передача в UI через быстрый канал (Tauri IPC батчами или локальный WebSocket/`<canvas>`). Достаточно
  для превью, не нагружает.
- B (позже): нативный слой поверх WebView для zero-copy превью (сложнее).
UI рисует кадры в `<canvas>`; основной (полнокачественный) поток идёт мимо UI — прямо в виртуальную камеру.

## 8. Запись (Recorder, опц. — может уехать в v1.1)

`FrameSink`, пишущий в контейнер (MP4/MKV) через ffmpeg. Кодек: при encoded-входе можно ремуксить без
перекодирования; при raw — кодировать. Управление: `start_recording/stop_recording`.

## 9. Конкурентность, конфиг, логи

- **tokio** runtime; media-путь — приоритетная задача; bounded-каналы с drop-oldest (см. [02](02-architecture.md) §6).
- Конфиг: файл (TOML) + дефолты; путь по ОС-конвенции (`dirs` crate). Хранит `pairingKey`, последние
  устройства, предпочтения режима.
- Логи: `tracing` + `tracing-subscriber`; уровни; диагностический режим (подробные метрики конвейера).

## 10. Точки тестирования (см. [11-testing-strategy.md](11-testing-strategy.md))

- Юнит: `ccp-protocol` round-trip; выбор режима в `adaptive`; нормализация форматов в `mediapipeline`;
  drop-oldest backpressure.
- Интеграция: мок-iPhone (Rust), гоняющий протокол по loopback → проверка пути до `FrameSink`-мока.
- Платформенный ручной чек-лист: виртуальная камера появляется в Zoom/Meet/OBS/QuickTime (macOS v1).
