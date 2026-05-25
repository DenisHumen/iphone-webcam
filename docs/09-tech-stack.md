# 09 — Технологический стек

Связано: [05-desktop-app.md](05-desktop-app.md), [08-project-structure.md](08-project-structure.md),
[12-decisions-log.md](12-decisions-log.md). Версии — «актуальная стабильная на 2026», без жёсткого
пиннинга патчей; мажорные фиксируются в `Cargo.toml`/`package.json`/`Package.swift`.

## 1. Десктоп — Rust-ядро

| Назначение | Крейт | Зачем |
|---|---|---|
| Async runtime | `tokio` | сетевой I/O, задачи, каналы |
| Сериализация control | `serde`, `serde_json` | JSON-сообщения CCP |
| Бинарные данные | `bytes`, `byteorder` | media-заголовок (big-endian), буферы |
| Логирование | `tracing`, `tracing-subscriber` | структурные логи, диагностика |
| Ошибки | `thiserror` (в крейтах), `anyhow` (в `app`) | типобезопасные ошибки |
| mDNS | `mdns-sd` | обнаружение/анонс `_clearcam._tcp` |
| QR | `qrcode` | генерация QR привязки |
| Конфиг/пути | `directories`, `toml` | конфиг по ОС-конвенции |
| TLS (опц.) | `rustls`, `tokio-rustls` | опц. шифрование Wi-Fi |
| Каналы/утилиты | `tokio::sync`, `parking_lot` | акторы, bounded-каналы |

### 1.1. Платформенные мосты (macOS, через FFI)

- **VideoToolbox (декод HEVC/H.264 → NV12):** крейты семейства **objc2** —
  `objc2-video-toolbox`, `objc2-core-media`, `objc2-core-video`. Чистый FFI без C-обёрток.
- **CoreMediaIO** (если ядро взаимодействует с расширением напрямую): `objc2-core-media-io`.
  Основной путь — расширение на Swift (см. §3), ядро общается с контейнером.

### 1.2. USB / usbmuxd

- Подход: **обёртка над libusbmuxd** (libimobiledevice) через FFI, либо реализация клиента usbmux
  протокола на Rust (plist-over-socket). Кандидаты (по приоритету): **`idevice`** (чистый Rust, async — говорит с usbmuxd/lockdownd
  напрямую, без C-зависимости) · `usbmux-client-tokio` (tokio usbmux) · `rusty_libimobiledevice` /
  `libimobiledevice-sys` (FFI-обёртки над C-библиотекой). usbmuxd-демон: на macOS — встроен (Apple Mobile Device), на Windows — ставится с Apple
  Mobile Device Support, на Linux — пакет `usbmuxd`. Решение и нюансы — ADR в [12](12-decisions-log.md).

### 1.3. Декод/запись (кросс-платформенный fallback)

- `ffmpeg-next` (bindings к FFmpeg) — опц. для декода вне macOS и для Recorder. **Лицензия:** собирать
  LGPL-вариант (без GPL-компонентов) либо избегать в v1. См. §6.

## 2. Десктоп — Tauri и UI

| Назначение | Технология |
|---|---|
| Оболочка приложения | **Tauri 2.x** (Rust backend + системный WebView) |
| Язык фронтенда | **TypeScript** |
| UI-фреймворк | **React 18+** |
| Сборщик | **Vite** |
| Стили | **Tailwind CSS** |
| Примитивы UI (опц.) | Radix UI / headless-компоненты |
| График метрик | **uPlot** (крошечный, быстрый) или `<canvas>` вручную |
| Пакетный менеджер | **pnpm** |

> React+Vite+Tailwind выбраны за распространённость и «дёшево красиво». Допустима замена на SvelteKit
> (легче бандл) — это локальное решение `ui/`, не влияет на ядро. ADR в [12](12-decisions-log.md).

## 3. Виртуальная камера (платформенные стоки)

| ОС | Технология | Заметки |
|---|---|---|
| **macOS** (v1) | **CoreMediaIO Camera Extension (CMIOExtension)**, Swift, System Extensions framework | macOS 12.3+; отдельный таргет, встроен в контейнер; подпись Developer ID + нотаризация |
| **Windows** (фаза 2) | **Media Foundation `MFCreateVirtualCamera`** (Win11 22000+), C++/Rust (`windows` crate) | видна MF- и DirectShow-приложениям |
| **Linux** (фаза 3) | **v4l2loopback** + запись через `v4l` crate (ioctl V4L2) | модуль ставится пользователем (DKMS) |

## 4. iPhone

| Назначение | API/технология |
|---|---|
| UI | SwiftUI |
| Конкурентность | Swift Concurrency (async/await, actors) |
| Захват | AVFoundation (`AVCaptureSession`, `AVCaptureVideoDataOutput`, `AVCaptureDevice.DiscoverySession`) |
| Кодирование | VideoToolbox (`VTCompressionSession`) |
| Сеть | Network.framework (`NWConnection`, `NWListener`, `NWBrowser`) |
| QR-скан | `AVCaptureMetadataOutput` (.qr) или VisionKit `DataScannerViewController` |
| Батарея/тепло | `UIDevice`, `ProcessInfo` |
| Сериализация | `Codable` (control) + ручная упаковка media-заголовка |

Минимум iOS 17.0. Менеджер зависимостей — SwiftPM (внешних зависимостей минимум; цель — нативные API).

## 5. Инструменты сборки и CI

- **Cargo** (Rust), **Tauri CLI**, **pnpm/Vite** (UI), **Xcode/xcodebuild + swift** (iOS, macOS-extension).
- **CI:** прогон `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`; lint/format UI; (на macOS-раннере) сборка iOS/extension. Детали — [11-testing-strategy.md](11-testing-strategy.md).
- Качество: `rustfmt`, `clippy`, `swiftformat`/`swiftlint`, `prettier`, `eslint`.

## 6. Лицензии (на что обратить внимание)

- **v4l2loopback** — GPLv2 (модуль ядра). Мы пишем в его устройство из userspace (не линкуемся) →
  наш код не становится производным; но распространение самого модуля имеет GPL-условия (обычно
  пользователь ставит его сам).
- **FFmpeg** — LGPL/GPL в зависимости от сборки. Если используем — LGPL-вариант; GPL-компоненты избегаем.
- **Qt не используется** (выбран Tauri) — соответствующих ограничений нет.
- Итоговая лицензия проекта — решение владельца (см. README, [12](12-decisions-log.md)).

## 7. Сводка внешних системных зависимостей (для пользователя/установщика)

- macOS: одобрение System Extension; (usbmuxd встроен).
- Windows: Apple Mobile Device Support (для USB); Windows 11 (для MF-камеры).
- Linux: пакет `usbmuxd`; модуль `v4l2loopback`.
