# 04 — iPhone-приложение (Swift)

Связано: [02-architecture.md](02-architecture.md), [03-protocol.md](03-protocol.md),
[07-ui-ux.md](07-ui-ux.md), [09-tech-stack.md](09-tech-stack.md).

## 1. Назначение

Захватить видео с выбранного объектива, при необходимости закодировать и передать на десктоп с
минимальной задержкой; принимать команды управления; сообщать телеметрию (батарея, тепло, метрики).

## 2. Технологии

| Задача | API |
|---|---|
| UI | SwiftUI |
| Захват | AVFoundation (`AVCaptureSession`, `AVCaptureVideoDataOutput`) |
| Перечисление камер | `AVCaptureDevice.DiscoverySession` |
| Кодирование | VideoToolbox (`VTCompressionSession`) |
| Wi-Fi транспорт | Network.framework (`NWConnection`, `NWBrowser`/`NWListener`) |
| USB транспорт | `NWListener` на TCP-порту; десктоп подключается через usbmuxd |
| QR-скан | AVFoundation (`AVCaptureMetadataOutput`, тип `.qr`) или VisionKit DataScanner |
| Батарея/тепло | `UIDevice.batteryLevel/batteryState`, `ProcessInfo.thermalState` |
| Конкурентность | Swift Concurrency (async/await, actors) + dispatch-очереди для захвата |

Минимальная iOS: **17.0** (тестировать на 18+). USB 3 (высокие raw-режимы) — iPhone 15 Pro+.

## 3. Захват (CaptureEngine)

- `AVCaptureSession` с `sessionPreset`/конкретным `AVCaptureDevice.Format`, выбранным под `mode`
  (разрешение/fps). Для точного fps — `activeVideoMinFrameDuration/Max`.
- Выход: `AVCaptureVideoDataOutput` с `kCVPixelFormatType_420YpCbCr8BiPlanarFullRange` (NV12 full-range)
  или `…VideoRange` — формат фиксируем как **NV12**, флаг `fullRange` сообщаем в media-заголовке.
- Кадры приходят как `CMSampleBuffer` → `CVPixelBuffer` (backed by IOSurface, без лишних копий).
- **Перечисление камер:** `AVCaptureDevice.DiscoverySession` по типам
  `[.builtInWideAngleCamera, .builtInUltraWideCamera, .builtInTelephotoCamera, .builtInTrueDepthCamera/front]`.
  Для каждого — id, позиция, поддерживаемые форматы (макс. разрешение/fps). Отдаём в `CAMERA_LIST`.
- **Переключение объектива (`SET_CAMERA`):** `beginConfiguration` → заменить `AVCaptureDeviceInput` →
  `commitConfiguration`. Бесшовно, без пересоздания сессии. Подтверждаем `CAMERA_STATE`.
- **Ориентация:** учитывать ориентацию устройства/`AVCaptureConnection.videoRotationAngle`; передавать
  упрямо «как снято» + флаг ориентации (десктоп при необходимости поворачивает в конвейере).
- v1: одна активная камера в момент времени (мульти-кам AVFoundation — будущее, см. не-цели).

## 4. Кодирование (Encoder)

Два пути по `mode.format`:

- **raw:** взять плоскости из `CVPixelBuffer` (Y + CbCr) → собрать payload NV12 → media-кадр
  (`codec=0`). Без копий, где можно (прямое чтение base address плоскостей). Это самый тяжёлый по
  полосе путь — допустим только если канал измерен достаточным.
- **encoded:** `VTCompressionSession` (`kCMVideoCodecType_HEVC` или `_H264`):
  - Реальное время: `kVTCompressionPropertyKey_RealTime = true`.
  - Низкая задержка: `AllowFrameReordering = false` (нет B-кадров).
  - Битрейт: `AverageBitRate` = `mode.bitrateKbps`; `DataRateLimits` для пиков.
  - Keyframe-интервал: умеренный (например 1–2 c) + keyframe по запросу/смене режима.
  - На keyframe выдаём VPS/SPS/PPS (из format description) → отправляем `config`-кадр перед AU.
  - Выход — AU в формате length-prefixed NAL (см. [03](03-protocol.md) §5.2).
- Смена режима (`SET_MODE`) — пересоздать/перенастроить сессию; отметить `MODE_APPLIED.atSeq`.

«Visually-lossless» достигается высоким `bitrateKbps` для текущего разрешения/fps (значения и логика —
[06-adaptive-engine-and-speedtest.md](06-adaptive-engine-and-speedtest.md)).

## 5. Транспорт (TransportClient)

- **Wi-Fi (client):** `NWConnection` к `host:cport` и `host:mport` из QR. TCP, `NWParameters.tcp`
  (опц. TLS). `serviceClass`/`.interactive` для приоритета.
- **USB (server):** `NWListener` на TCP-порту (control и media). Десктоп подключается через usbmuxd по
  этому порту. Параллельно работают оба режима только при необходимости; обычно активен один транспорт.
- **Bonjour/mDNS:** опц. для показа доступных ПК; основной путь — QR.
- Сериализация: control — JSON (`Codable`); media — ручная упаковка бинарного заголовка (28 байт LE) +
  payload. Отправка media — на выделенной очереди, без блокировки захвата.
- **Backpressure:** если сокет не успевает — **дропать кадры на отправке** (а не копить), вести `dropCount`.

## 6. Статус и телеметрия (StatusAgent)

- `UIDevice.current.isBatteryMonitoringEnabled = true` → `batteryLevel` (0..1), `batteryState`.
- `ProcessInfo.processInfo.thermalState` (`.nominal/.fair/.serious/.critical`) — при serious/critical
  предупреждать и опц. снижать режим.
- Метрики отправки: `sentBitrate`, `captureFps`, `encFps`, `queueDepth`, `dropCount`.
- Слать `TELEMETRY` ~2 Гц по control. `DEVICE_INFO` — один раз при подключении и при изменениях.

## 7. Контроллер сессии (SessionController)

State machine: `idle → connecting → handshaking → ready → speedtest → streaming → (reconnecting) →
stopped`. Обрабатывает входящие control-команды:

- `SET_CAMERA` → CaptureEngine.setCamera; `START/STOP` → запуск/останов выдачи media;
  `SET_MODE` → Encoder/Capture перенастройка; `SPEEDTEST_START` → генератор нагрузки.
- При разрыве — авто-переподключение с backoff; восстановление последнего режима.

## 8. Экраны (см. [07-ui-ux.md](07-ui-ux.md))

1. **Onboarding/Connect:** скан QR (большая рамка-видоискатель), статус подключения, ручной ввод как fallback.
2. **Streaming:** живой видоискатель (что снимает камера), индикаторы (подключено/канал Wi-Fi|USB,
   режим, fps, битрейт, заряд, тепло), кнопка «Стоп», переключатель объектива (дублирует управление с ПК).
3. **Settings:** выбор камеры по умолчанию, разрешение/fps по умолчанию, поведение экрана (не гаснуть),
   о приложении.

## 9. Разрешения и Info.plist

- `NSCameraUsageDescription` — доступ к камере.
- `NSLocalNetworkUsageDescription` + `NSBonjourServices` (`_clearcam._tcp`) — локальная сеть/mDNS.
- `UIBackgroundModes`: захват/стрим в фоне ограничен iOS; v1 — работа на переднем плане, экран не гаснет
  (`isIdleTimerDisabled = true`). Фоновую работу не обещаем (ограничения платформы).

## 10. Производительность и батарея

- Минимум копий: читать плоскости `CVPixelBuffer` напрямую; переиспользовать буферы (pools).
- Кодирование — аппаратное (VideoToolbox), не CPU.
- Следить за нагревом; при `.serious/.critical` — снизить fps/разрешение, предупредить.
- Экран можно затемнять/выключать рендер видоискателя в энергосбережении (передача продолжается).

## 11. Точки тестирования (см. [11-testing-strategy.md](11-testing-strategy.md))

- Юнит: упаковка media-заголовка (соответствие [03](03-protocol.md) §5.2), JSON Codable round-trip,
  state machine SessionController.
- Интеграция (loopback к мок-серверу): handshake, смена камеры, смена режима.
- Ручной чек-лист на устройстве: каждый объектив, Wi-Fi и USB, нагрев, разрыв/восстановление.
