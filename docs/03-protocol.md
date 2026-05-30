# 03 — Сетевой протокол ClearCam (CCP)

**ClearCam Protocol (CCP)** — прикладной протокол между iPhone и десктопом. Версия документа
описывает **CCP v1**. Связано: [02-architecture.md](02-architecture.md),
[06-adaptive-engine-and-speedtest.md](06-adaptive-engine-and-speedtest.md).

## 1. Цели дизайна

- Один протокол поверх двух транспортов (Wi-Fi/USB) — транспорт скрыт.
- Разделение control (надёжно, низкая частота, расширяемо) и media (высокая частота, низкая задержка).
- Простота отладки control (JSON) при эффективности media (бинарный заголовок).
- Версионирование и согласование возможностей с первого дня.
- Заделы: аудио-дорожка и несколько источников — без слома совместимости.

## 2. Транспорт и соединения

CCP использует **два TCP-соединения** на сессию:

| Соединение | Назначение | Формат |
|---|---|---|
| **control** | команды, телеметрия, тест скорости, keepalive | length-prefixed JSON |
| **media** | видеокадры | бинарный заголовок + payload |

**Кто слушает (dialing) — деталь транспорта, скрытая адаптером.** После установления оба соединения
симметричны и проходят одинаковый handshake.

- **Wi-Fi:** десктоп слушает (server) на `controlPort`/`mediaPort`; iPhone подключается (client) после
  сканирования QR. Десктоп анонсирует себя в mDNS опционально; основной путь привязки — QR (ниже).
- **USB:** iPhone слушает (server) на тех же логических портах внутри устройства; десктоп подключается
  через **usbmuxd** (libusbmuxd / libimobiledevice), который пробрасывает TCP-подобный канал по USB.
  Для приложения это обычный сокет.

> Транспортный слой выдаёт ядру абстракцию `Transport::connect() -> (ControlStream, MediaStream, PeerInfo)`.
> Всё, что ниже §3, одинаково для Wi-Fi и USB.

TCP выбран для v1 ради простоты и надёжности на LAN/USB (низкие потери). Возможный переход на
QUIC/UDP+FEC для Wi-Fi с потерями — будущее расширение (см. [12-decisions-log.md](12-decisions-log.md)).

## 3. Обнаружение и привязка (Wi-Fi)

### 3.1. mDNS/Bonjour (обнаружение)

- Тип сервиса: `_clearcam._tcp`. Анонсирует **десктоп** (так как он server на Wi-Fi).
- TXT-записи: `v` (версия протокола), `name` (имя ПК), `cport`, `mport`, `id` (uuid инстанса).
- iPhone может находить ПК в сети для показа в списке; но для соединения нужен токен (QR).

### 3.2. QR-привязка (основной путь)

Десктоп показывает QR с JSON (компактным), iPhone сканирует камерой:

```json
{ "v":1, "host":"192.168.1.42", "cport":7000, "mport":7001,
  "token":"b6f3…(32 байта base64url)", "fp":"sha256:…(опц., отпечаток TLS)" }
```

iPhone подключается к `host:cport` и `host:mport`, в handshake предъявляет `token`.

### 3.3. Ручной ввод

Пользователь вводит `host` и `token` (или короткий PIN) вручную — fallback при проблемах с QR/mDNS.

### 3.4. USB-привязка

При первом подключении по USB телефон показывает запрос подтверждения («Доверять этому компьютеру для
ClearCam?»). После подтверждения сохраняется долгоживущий `pairingKey` (на обеих сторонах) → последующие
USB-подключения авто-устанавливаются без QR.

## 4. Handshake и аутентификация

Выполняется на **обоих** соединениях сразу после установления (каждое привязывается к сессии по
`sessionId`). Порядок на control:

1. Клиент → `HELLO` (версия, возможности, `sessionId`-кандидат).
2. Сервер → `HELLO_ACK` (согласованная версия/возможности) **или** `ERROR` (несовместимость).
3. Клиент → `AUTH` (`token` или `pairingKey`).
4. Сервер → `AUTH_OK` **или** `ERROR` (`unauthorized`).
5. media-соединение: клиент → `MEDIA_HELLO` с тем же `sessionId` + `token` → сервер связывает его с
   control-сессией. Несвязанное media-соединение закрывается.

**Пример JSON `AUTH` (Wi-Fi):**

```json
{ "t": "AUTH", "seq": 3, "token": "b6f3..." }
```

**Пример JSON `AUTH` (USB, привязанное устройство):**

```json
{ "t": "AUTH", "seq": 3, "pairingKey": "VL0e..." }
```

Поле `token` и `pairingKey` обрабатываются как взаимоисключающие альтернативы.
Десктоп различает их по присутствующему ключу. См. реализацию: `ccp-protocol::Auth`
(`untagged` serde enum), `session::handshake` отвергает `PairingKey` на Wi-Fi-пути
с `ErrorCode::Unauthorized` (USB-путь будет принимать его в Phase 6).

Тайм-аут handshake — 5 c. Незавершённый handshake → закрытие.

## 5. Формат кадрирования (framing)

### 5.1. Control (length-prefixed JSON)

```
[uint32 BE length][UTF-8 JSON payload]   // length = размер JSON в байтах
```

Каждый JSON-объект обязан содержать поле `t` (type) и `seq` (uint, монотонный на отправителе).
Ответы ссылаются на запрос через `ack` = `seq` запроса (где применимо).

### 5.2. Media (бинарный заголовок + payload)

Заголовок фиксированной длины (**сетевой порядок байт, big-endian** — как и префикс длины control), затем payload:

| Поле | Тип | Размер | Описание |
|---|---|---|---|
| `magic` | u16 | 2 | `0xCC01` (маркер + версия media-формата) |
| `type` | u8 | 1 | `1`=video, (резерв: `2`=audio) |
| `flags` | u8 | 1 | бит0 `keyframe`; бит1 `encoded`(1) vs raw(0); бит2 `fullRange`; бит3 `config`(содержит codec config) |
| `codec` | u8 | 1 | `0`=raw NV12, `1`=HEVC, `2`=H.264 |
| `reserved` | u8[3] | 3 | выравнивание до 28 байт, нули |
| `width` | u16 | 2 | пиксели |
| `height` | u16 | 2 | пиксели |
| `seq` | u32 | 4 | номер кадра, монотонный |
| `ptsUsec` | u64 | 8 | метка времени захвата, микросекунды (часы устройства) |
| `payloadLen`| u32 | 4 | длина payload в байтах |
| **payload** | bytes | `payloadLen` | см. ниже |

Итого заголовок = 28 байт. Один media-кадр = один заголовок + один payload (для TCP дробление не нужно;
TCP сам соберёт). Для будущего UDP-режима payload дробится на chunk'и (поля chunk в расширенном заголовке).

**Payload, raw (`codec=0`):** NV12 — плоскость Y (`width*height` байт) + плоскость CbCr
(`width*height/2` байт, чередующиеся). Stride = width (без выравнивания) в v1; при необходимости
выравнивание добавляется флагом и доп. полем (расширение).

**Payload, encoded (`codec=1|2`):** один access unit (кадр) в формате length-prefixed NAL units:
повтор `[uint32 BE nalLen][nal bytes]`. Параметры (VPS/SPS/PPS) присылаются: (а) в `config`-кадре
(`flags.config=1`) при старте/смене режима, и (б) перед каждым keyframe (для устойчивости к подключению
в середине). Так декодер на ПК всегда может инициализироваться.

## 6. Каталог сообщений control (CCP v1)

JSON, поле `t` — тип. Ниже — назначение и ключевые поля (примеры сокращены).

### 6.1. Установление

| `t` | Направление | Поля | Назначение |
|---|---|---|---|
| `HELLO` | iPhone→ПК | `protoVer`,`app`,`device`,`sessionId`,`caps` | начало handshake |
| `HELLO_ACK` | ПК→iPhone | `protoVer`,`caps` | согласование версии/возможностей |
| `AUTH` | iPhone→ПК | `token`\|`pairingKey` | аутентификация |
| `AUTH_OK` | ПК→iPhone | `sessionId` | успех |
| `MEDIA_HELLO`| iPhone→ПК | `sessionId`,`token` | привязка media-соединения |
| `BYE` | оба | `reason` | корректное завершение |
| `ERROR` | оба | `code`,`message` | ошибка (см. §11; `seq` неудачного запроса несёт `ack` конверта) |

### 6.2. Устройство и камеры

| `t` | Направление | Поля | Назначение |
|---|---|---|---|
| `DEVICE_INFO` | iPhone→ПК | `model`,`osVer`,`batteryLevel`,`batteryState`,`thermalState`,`usb3Capable` | паспорт устройства |
| `CAMERA_LIST` | iPhone→ПК | `cameras:[{id,name,position,maxRes,maxFps,supportedFormats}]` | список объективов и их пределы |
| `SET_CAMERA` | ПК→iPhone | `cameraId` | сменить активный объектив |
| `CAMERA_STATE`| iPhone→ПК | `activeCameraId`,`appliedFormat` | подтверждение/текущее состояние |

### 6.3. Трансляция и режим

| `t` | Направление | Поля | Назначение |
|---|---|---|---|
| `START` | ПК→iPhone | `mode` (см. §7) | начать выдачу media |
| `STOP` | ПК→iPhone | — | остановить выдачу media |
| `SET_MODE` | ПК→iPhone | `mode` | сменить режим на лету (raw/encoded, res, fps, bitrate) |
| `MODE_APPLIED`| iPhone→ПК | `mode`,`atSeq` | режим применён начиная с media `seq` |

### 6.4. Телеметрия и keepalive

| `t` | Направление | Поля | Назначение |
|---|---|---|---|
| `TELEMETRY` | iPhone→ПК | `tsUsec`,`batteryLevel`,`batteryState`,`thermalState`,`sentBitrate`,`encFps`,`captureFps`,`queueDepth`,`dropCount` | периодика (~2 Гц) |
| `PING` | оба | `tsUsec` | измерение RTT/часов |
| `PONG` | оба | `tsUsec`,`echoUsec` | ответ на PING |

### 6.5. Тест скорости (см. [06](06-adaptive-engine-and-speedtest.md))

| `t` | Направление | Поля | Назначение |
|---|---|---|---|
| `SPEEDTEST_START` | ПК→iPhone | `id`,`targetBitrate`,`durationMs`,`pattern` | запросить генерацию нагрузки |
| `SPEEDTEST_TICK` | iPhone→ПК | `id`,`tickIndex`,`tsUsec` (+ нагрузка идёт по media как `type=video,flags.config? нет` спец-маркер; внутренний счётчик зовётся `tickIndex`, чтобы не пересекаться с `seq` конверта) | прогресс |
| `SPEEDTEST_RESULT`| ПК→iPhone | `id`,`goodputMbps`,`rttMs`,`jitterMs`,`lossPct`,`recommendedMode` | итог |

> Нагрузка теста физически идёт по **media**-соединению (реалистично измеряет именно видео-путь),
> помеченная спец-значением `seq`-диапазона/флага, чтобы MediaPipeline её не выводил в камеру.

## 7. Описание режима (`mode`)

Единый объект, используемый в `START`/`SET_MODE`/`SPEEDTEST_RESULT.recommendedMode`:

```json
{ "format":"raw"|"encoded", "codec":"none"|"hevc"|"h264",
  "width":1920, "height":1080, "fps":30,
  "bitrateKbps":40000,            // для encoded; игнор для raw
  "pixelFormat":"nv12", "fullRange":true }
```

Допустимые комбинации ограничены возможностями активной камеры (`CAMERA_LIST`) и платформой
(USB3 для высоких raw-режимов). Валидация — на обеих сторонах.

## 8. Синхронизация часов и измерение задержки

`ptsUsec` в media-кадрах — часы устройства (монотонные). Десктоп оценивает смещение часов через
`PING/PONG` (NTP-подобная формула, RTT/2). Это даёт оценку glass-to-glass задержки для метрик (NFR-1)
и не требуется для корректности воспроизведения.

## 9. Версионирование и согласование возможностей

- `protoVer` (целое) сравнивается в `HELLO`/`HELLO_ACK`. Несовпадение мажорной версии → `ERROR
  (incompatible_version)`.
- `caps` — список строк-флагов (например `["hevc","h264","raw_nv12","usb3","speedtest_v1"]`). Стороны
  используют пересечение. Новые возможности добавляются как новые флаги без слома старых клиентов.

## 10. Расширяемость (заделы)

- **Аудио:** media-заголовок уже имеет `type` (`2`=audio зарезервирован). Добавление аудио = новый тип
  media-кадров + cap `"audio_aac"`/`"audio_pcm"` + виртуальный микрофон на ПК. Совместимость не ломается.
- **Несколько источников:** каждый источник = своя сессия (свой `sessionId`, своя пара соединений).
  ControlPlane агрегирует. Протокол менять не нужно; нужен лишь «активный источник» на ПК.
- **Новый транспорт/кодек:** добавляется как новое значение `codec`/новый транспорт-адаптер.

## 11. Ошибки, тайм-ауты, keepalive

- Коды `ERROR.code`: `incompatible_version`, `unauthorized`, `bad_request`, `unsupported_mode`,
  `camera_unavailable`, `internal`, `timeout`.
- **Keepalive:** `PING` каждые 2 c; нет `PONG`/данных 6 c → соединение считается мёртвым → `reconnecting`.
- Handshake-тайм-аут 5 c. Тест скорости имеет собственный `durationMs` + запас.
- Любая нераспознанная `t` на control → игнор (forward-compat), но логируется.

## 12. Безопасность

- **Привязка по токену обязательна** (QR-токен для Wi-Fi, pairingKey для USB).
- **Опциональный TLS** для Wi-Fi (self-signed, отпечаток `fp` в QR для пиннинга). В v1 может быть выключен
  по умолчанию (LAN), но интерфейс транспорта закладывает TLS-обёртку. См. [12-decisions-log.md](12-decisions-log.md).
- Токены — криптослучайные (≥128 бит), не логируются.
- USB-канал физически локален; Wi-Fi-канал ограничен LAN (без проброса в интернет).

## 13. Сценарии (sequence)

**Подключение + старт (Wi-Fi):**

```
iPhone                         Desktop
  |  (скан QR: host,ports,token)   |
  |----- TCP connect control ----->|
  |----------- HELLO ------------->|
  |<--------- HELLO_ACK -----------|
  |------------ AUTH ------------->|
  |<---------- AUTH_OK ------------|
  |----- TCP connect media ------->|
  |--------- MEDIA_HELLO --------->|
  |------- DEVICE_INFO ----------->|
  |------- CAMERA_LIST ----------->|
  |<---- SPEEDTEST_START ----------|
  |==== нагрузка по media =========>|
  |<---- SPEEDTEST_RESULT ---------|   (recommendedMode)
  |<--------- START(mode) ---------|
  |==== media-кадры =============>|   → MediaPipeline → VirtualCamera
  |------- TELEMETRY (2 Гц) ------>|
```

**Адаптация на лету:** ПК видит рост `lossPct`/задержки → `SET_MODE(пониже)` → iPhone `MODE_APPLIED`.
