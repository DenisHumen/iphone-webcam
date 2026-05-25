# 08 — Структура проекта

Связано: [02-architecture.md](02-architecture.md), [05-desktop-app.md](05-desktop-app.md),
[09-tech-stack.md](09-tech-stack.md). Это целевая раскладка; код появится по [10-roadmap-and-plan.md](10-roadmap-and-plan.md).

## 1. Монорепозиторий

Один репозиторий: десктоп, iOS и общая документация. Так проще держать протокол согласованным.

```
iphone_webcam/
├── README.md
├── .gitignore
├── docs/                          # вся документация (этот каталог)
│
├── desktop/                       # десктоп-приложение
│   ├── Cargo.toml                 # Rust workspace (виртуальный)
│   ├── crates/
│   │   ├── ccp-protocol/          # типы control (serde) + media-заголовок (кодек/декодек), версии
│   │   ├── transport/             # трейт Transport + реализации wifi/ и usb/
│   │   ├── session/               # ControlPlane/SessionManager (actor, источник правды)
│   │   ├── adaptive/              # SpeedTest + выбор/коррекция режима
│   │   ├── mediapipeline/         # приём, декод/passthrough, NV12-нормализация, джиттер-буфер
│   │   ├── decode/                # трейт Decoder + адаптеры (VideoToolbox / ffmpeg)
│   │   ├── sink/                  # трейт FrameSink + preview/recorder + re-export платформенных
│   │   └── app/                   # сборка ядра: tokio-задачи, конфиг, логирование
│   ├── src-tauri/                 # Tauri-обвязка (commands/events, окно, трей)
│   │   ├── Cargo.toml
│   │   ├── tauri.conf.json
│   │   └── src/
│   ├── ui/                        # фронтенд (TypeScript + React + Vite + Tailwind)
│   │   ├── package.json
│   │   └── src/
│   └── sinks/                     # платформенные стоки виртуальной камеры
│       ├── macos-extension/       # Swift CMIO Camera Extension + container glue (Xcode)
│       ├── windows-mf/            # MFCreateVirtualCamera (C++/Rust) — фаза 2
│       └── linux-v4l2/            # запись в /dev/videoN (v4l2loopback) — фаза 3
│
├── ios/                           # iPhone-приложение (Xcode проект / SwiftPM)
│   ├── ClearCam.xcodeproj (или Package.swift)
│   └── Sources/
│       ├── App/                   # SwiftUI приложение, экраны
│       ├── Capture/               # CaptureEngine (AVFoundation)
│       ├── Encode/                # Encoder (raw + VideoToolbox)
│       ├── Transport/             # TransportClient (Network.fw / usbmuxd listener)
│       ├── Protocol/              # CCP: Codable control + бинарный media-заголовок (зеркало ccp-protocol)
│       ├── Session/               # SessionController (state machine)
│       └── Status/                # StatusAgent (battery/thermal/telemetry)
│
├── scripts/                       # сборка/запуск/установка (см. ниже)
└── tools/                         # вспомогательные утилиты (мок-iPhone, генераторы и пр.)
```

## 2. Границы модулей (правила)

- **`ccp-protocol` не зависит ни от чего платформенного и сетевого** — только типы и (де)сериализация.
- **Ядро (crates) не содержит платформенного UI и платформенных API**, кроме как за трейтами
  (`Transport`, `FrameSink`, `Decoder`). Платформенное — в `sinks/`, `decode/` адаптерах, `transport/usb`.
- **`session` — единственный владелец состояния сессии.** Остальные общаются с ним сообщениями.
- **UI (`ui/`) не содержит бизнес-логики** — только отображение и вызовы Tauri-команд.
- **iOS `Protocol/` зеркалит `ccp-protocol`** байт-в-байт по формату media-заголовка и именам полей
  control. [03-protocol.md](03-protocol.md) — единый контракт; при изменении правится он и обе реализации.
- Никаких «сквозных» зависимостей в обход трейтов. Добавление транспорта/кодека/стока = новая реализация
  трейта, без правок ядра.

## 3. Сборка и запуск

### Десктоп (macOS, v1)
- Требования: Rust (stable), Node.js + pnpm, Tauri CLI, Xcode (для CMIO-расширения и подписи).
- Dev: `pnpm install` в `ui/`; `cargo tauri dev` из `desktop/`.
- Виртуальная камера: собрать/установить `sinks/macos-extension` (System Extension); требует подписи
  Developer ID и одобрения пользователем. Скрипт `scripts/install-macos-extension.sh`.
- Release: `cargo tauri build` + подпись/нотаризация.

### iOS
- Открыть `ios/` в Xcode; задать Bundle ID и команду подписи; собрать на устройство (камера нужна на
  реальном iPhone, не симуляторе).

### Скрипты (`scripts/`)
- `dev-desktop.sh` — запуск десктопа в dev.
- `install-macos-extension.sh` / `uninstall-…` — управление CMIO-расширением.
- `setup-linux-v4l2.sh` — установка/загрузка v4l2loopback (фаза 3).
- `mock-iphone.sh` — запустить мок-клиент (из `tools/`) для интеграционных тестов без телефона.

## 4. Соглашения

- **Язык кода/идентификаторов — английский.** Комментарии — по ситуации (допустимо русские пояснения).
- Rust: `rustfmt` + `clippy` (CI-гейты). Swift: `swiftformat`/`swiftlint`. UI: `prettier` + `eslint`.
- Имена крейтов — `kebab-case`; модулей Rust — `snake_case`; Swift-типов — `UpperCamelCase`.
- Версия протокола (`protoVer`) поднимается при несовместимых изменениях; ADR в [12](12-decisions-log.md).
- Каждая фаза из [10-roadmap-and-plan.md](10-roadmap-and-plan.md) — отдельная ветка/набор PR.
