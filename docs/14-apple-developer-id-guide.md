# 14 — Apple Developer ID: пошаговый гайд получения

Связано: [docs/12-decisions-log.md](12-decisions-log.md) ADR-013 (CMIO Camera Extension),
[docs/10-roadmap-and-plan.md](10-roadmap-and-plan.md) Фаза 3.

Документ — практическая инструкция для владельца проекта (Denis), что и в какой
последовательности сделать, чтобы получить две сущности Apple, без которых нельзя
собрать и распространить виртуальную камеру macOS из Фазы 3:

1. **Apple Developer Program membership** ($99/год, физлицо или ИП).
2. **Developer ID Application** + **Developer ID Installer** сертификаты для подписи
   приложения и его CMIO System Extension вне App Store.
3. Доступ к нотаризации (Notary Service) — включён автоматически вместе с членством.

> Если на каком-то шаге форма Apple сломалась или поменялась — Apple обновляет UI
> довольно часто, последовательность шагов остаётся той же.

---

## 0. Что именно нужно нам и зачем

| Нужно для | Сущность | Где используется |
|---|---|---|
| Собрать и запустить **iOS-приложение ClearCam** на реальном iPhone | Apple Developer Program | iOS Фазы 1–5 (хотя бесплатный free provisioning тоже работает для разработки, но не для распространения) |
| Подписать **macOS-приложение** для распространения вне App Store | Developer ID Application | Фаза 6 (полировка), Фаза 7 (Windows-релиз) — для нашего CMIO нужно ОБЯЗАТЕЛЬНО |
| Подписать **CMIO System Extension** (виртуальная камера) | Developer ID Application + entitlement `com.apple.developer.system-extension.install` | Фаза 3 |
| Подписать **установщик `.pkg`** при сборке релиза | Developer ID Installer | Фаза 3/6 — необязательно, если распространяем `.dmg` |
| **Нотаризация** скомпилированного `.app`/`.pkg`/extension | Notary Service (включён в Developer Program) | Фаза 3 — без неё macOS Gatekeeper заблокирует расширение даже с подписью |

Технически бесплатный «free provisioning» от Apple позволяет запускать iOS-приложения
на собственном iPhone 7 дней (потом надо переподписывать). Для CMIO macOS-расширения
free-вариант **не работает вообще** — расширение должно быть подписано Developer ID и
нотаризовано, иначе `systemextensionsctl` его отвергнет.

---

## 1. Подготовка (один раз, до первой покупки членства)

### 1.1. Создать (или взять существующий) Apple ID

- Идём в [appleid.apple.com](https://appleid.apple.com), регистрируем Apple ID, если нет.
- Включаем **двухфакторную аутентификацию** (2FA) — Apple Developer Program требует.
- Имя/фамилия в Apple ID должны совпадать с тем, что укажете в членстве — пишите
  на латинице сразу (это будет видно в подписи).
- На этот Apple ID должен быть привязан реальный платёжный метод (карта). Apple
  принимает Visa/Mastercard. С российскими картами на 2026-05 стандартная схема
  не работает; обходные пути (карта другой юрисдикции, оплата через банк-партнёр)
  не описываются в этом гайде — это вопрос платежа, не технической интеграции.

### 1.2. Решить: личное членство (individual) или организация (company)

| Тип | Стоимость | Что в подписи | Особенности |
|---|---|---|---|
| **Individual** | $99/год | Имя и фамилия владельца | Регистрация занимает 1–2 дня, иногда сразу. Минимум формальностей. |
| **Organization** | $99/год | Имя организации (нужен DUNS-номер) | Регистрация 1–4 недели (DUNS-верификация). Можно потом приглашать соразработчиков. |

Для соло-проекта берите **Individual**. Из individual в organization можно
переключиться позже (но это перевыпуск всех сертификатов).

### 1.3. Установить Xcode

- Xcode 16+ (минимум для нашего проекта; см. ADR-021 — iOS 17 / macOS 13).
- Через App Store, ~10 GB.
- Запустить хотя бы раз → принять лицензионное соглашение.
- В `Xcode → Settings → Accounts` добавить тот же Apple ID, что планируете оформить
  на Developer Program (потом Xcode подтянет сертификаты).

---

## 2. Покупка Apple Developer Program

### 2.1. Регистрация

1. Откройте [developer.apple.com/programs](https://developer.apple.com/programs/).
2. Кнопка **Enroll**.
3. Войдите тем же Apple ID, что в Xcode (см. 1.3).
4. Apple задаст несколько вопросов — стандарт: имя, адрес, телефон. Адрес должен
   соответствовать стране карты оплаты. Поля — латиницей.
5. Выбираете тип — **Individual** (см. 1.2).
6. Соглашаетесь с тремя Apple-договорами (Apple Developer Agreement, Apple Developer
   Program License Agreement, отдельный для App Store).
7. Оплачиваете $99 (Apple снимает раз в год).

### 2.2. Ожидание

- Individual: обычно одобряется за час–сутки. Иногда сразу. Иногда Apple просит
  скан паспорта/ID — присылают email.
- Organization: 1–4 недели + DUNS-верификация (об этом отдельный шаг 2.3.).

После одобрения на email приходит «Welcome to the Apple Developer Program»;
аккаунт получает доступ к [developer.apple.com/account](https://developer.apple.com/account/)
со всеми разделами (Certificates, Identifiers, Profiles, Provisioning, etc.).

### 2.3. (Только organization) DUNS-номер

- DUNS — это идентификатор организации от Dun & Bradstreet (D&B).
- Заявка бесплатна: [developer.apple.com/enrollment/duns-lookup](https://developer.apple.com/enrollment/duns-lookup/).
- D&B обычно подтверждает за 1–2 недели; у Apple свой backoffice ещё неделю.

Для соло-проекта пропускайте — берите individual (2.1).

---

## 3. Выпуск сертификатов Developer ID

После одобрения членства:

### 3.1. Через Xcode (рекомендуемый путь — Xcode сам генерит CSR)

1. `Xcode → Settings → Accounts → <ваш Apple ID> → Manage Certificates…`
2. В левом нижнем углу **+** → выбрать:
   - **Developer ID Application** — для подписи `.app` / system extension.
   - **Developer ID Installer** — для подписи `.pkg` (опционально, если будете делать .pkg).
3. Xcode сгенерирует CSR, отправит Apple, скачает сертификат и положит в Keychain
   автоматически.

### 3.2. Альтернатива — через веб-консоль (если Xcode-путь сломался)

1. Открыть Keychain Access (Связка ключей) на mac.
2. Меню **Certificate Assistant → Request a Certificate From a Certificate Authority…**
3. Заполнить email + Common Name, выбрать **Saved to disk**. Получаете файл `.certSigningRequest`.
4. На [developer.apple.com/account/resources/certificates/list](https://developer.apple.com/account/resources/certificates/list)
   нажать **+**, выбрать **Developer ID Application**, загрузить CSR, скачать `.cer`.
5. Двойной клик на скачанном `.cer` → Keychain установит сертификат.

> **Важно про резервную копию приватного ключа.** Каждый Developer ID сертификат
> создаётся вместе с приватным ключом *локально в Keychain*. Если потеряете ключ —
> отозвать-и-перевыпустить можно (но если он уже использован для нотаризации
> старых релизов, придётся всё перезаливать). **Сделайте экспорт `.p12`** прямо
> сейчас (Keychain → правой кнопкой на сертификате → Export → .p12 с паролем) и
> сохраните в безопасном месте (1Password, USB-флешка в сейфе и т.п.). Без этого
> переехать на новый Mac будет больно.

### 3.3. Создание App ID + entitlements для CMIO-расширения

Для Фазы 3 потребуется отдельный App ID для extension:

1. [developer.apple.com/account/resources/identifiers/list](https://developer.apple.com/account/resources/identifiers/list) → **+**.
2. Тип — **App IDs**.
3. Bundle ID: например `dev.clearcam.ClearCam` (для основного приложения) и
   отдельный для extension типа `dev.clearcam.ClearCam.CameraExtension`.
4. Capabilities — отметить:
   - **System Extension** (для extension).
   - **Camera** (для основного приложения).
   - **App Sandbox** (по необходимости — для CMIO лучше включить).

App ID создаётся бесплатно, на это не тратится годовое членство.

---

## 4. Нотаризация (Notary Service)

Нотаризация — отдельный шаг **после** подписи сертификатом. Apple-сервер
сканирует ваш `.app` на malware и выдаёт «билет», который macOS Gatekeeper
проверяет при первом запуске.

### 4.1. Подготовка app-specific password

1. [appleid.apple.com](https://appleid.apple.com) → Sign-In and Security → App-Specific Passwords → **+**.
2. Назвать, например, `notarytool-clearcam`. Получите 16-символьный пароль.
3. Запомнить (показывается один раз). Хранить в Keychain или 1Password.

### 4.2. Локальная настройка `notarytool` keychain entry

```bash
xcrun notarytool store-credentials "clearcam-notary" \
  --apple-id "your@email.com" \
  --team-id "ABCD123456" \
  --password "<16-символьный app-specific password>"
```

`--team-id` смотрим в [developer.apple.com/account](https://developer.apple.com/account/)
(сверху справа).

Команда сохранит профиль в локальном keychain. Дальше можно нотаризовать без
ввода пароля:

```bash
xcrun notarytool submit ClearCam.dmg --keychain-profile "clearcam-notary" --wait
xcrun stapler staple ClearCam.dmg
```

Эти два шага будут зашиты в build-скрипт Фазы 3.

---

## 5. Что после получения

Когда Apple Developer ID активен и сертификаты в Keychain:

1. **Обновить `docs/12-decisions-log.md`** — снять статус «🕗 D-3» (если есть)
   с любых пунктов, которые ждали Apple ID. ADR-013 остаётся принятым.
2. **Обновить `MEMORY.md`** агента — пометить Phase 3 как unblocked.
3. **Начать Фазу 3** по [docs/10-roadmap-and-plan.md](10-roadmap-and-plan.md):
   создать CMIO Camera Extension таргет (Swift), скрипт установки/удаления
   (`scripts/install-macos-extension.sh`), интеграцию с десктоп-ядром через
   IOSurface/XPC. Перед стартом — `superpowers:writing-plans` для детального
   плана Фазы 3.
4. **Включить в CI macOS-job сборку extension** (signing с временным сертификатом
   — Apple Self-Signed для PR-проверок; полная подпись Developer ID — только в
   release-pipeline).

## 6. Частые проблемы и обход

| Симптом | Причина | Решение |
|---|---|---|
| «There is no Developer ID Application certificate» в Xcode | Сертификат не выпущен / истёк | См. шаг 3.1, выпустить через Xcode |
| `codesign --verify` says «valid on disk» но Gatekeeper отвергает | Не нотаризовано | Шаг 4.2 + `stapler staple` |
| `notarytool submit` падает с «Invalid Team ID» | `--team-id` неправильный | Проверить в [developer.apple.com/account](https://developer.apple.com/account/) → справа сверху |
| `systemextensionsctl install` падает с `Code Signature Invalid` | Расширение собрано Apple Development сертификатом, а не Developer ID Application | Пересобрать с правильным сертификатом + перенотаризовать |
| Apple отказала в членстве «Identity verification needed» | Apple просит скан паспорта | Загрузить через email-ссылку, ждать ещё 1–3 дня |
| Расширение установилось, но Zoom его не видит | macOS не «прокинул» CMIO для этого приложения — нужен пользовательский enable в `System Settings → Privacy & Security → Extensions → Camera Extensions` | После установки попросить пользователя зайти и включить |
| Срок Developer Program закончился | Apple отзывает все сертификаты автоматически | Продлить, перевыпустить сертификаты, пересобрать релизы |

## 7. Чеклист «всё ли готово к Фазе 3»

- [ ] Apple Developer Program активен (видно `Active` на [account](https://developer.apple.com/account/)).
- [ ] Developer ID Application сертификат установлен в Keychain mac.
- [ ] (Опционально) Developer ID Installer сертификат установлен.
- [ ] `.p12` обоих сертификатов сохранён в надёжном месте (не на этом Mac).
- [ ] `xcrun notarytool store-credentials clearcam-notary` выполнен и работает.
- [ ] App ID `dev.clearcam.ClearCam` + `dev.clearcam.ClearCam.CameraExtension`
      созданы в [Identifiers](https://developer.apple.com/account/resources/identifiers/list).
- [ ] System Extension capability включена для extension App ID.
- [ ] Бюджет/время на проблемы: первый раз нотаризация может занять несколько
      попыток (Apple возвращает stapling-логи, по которым становится понятно,
      какой entitlement добавить).

После всех галочек — можно стартовать Фазу 3 (CMIO Camera Extension); план
напишем тогда отдельно.
