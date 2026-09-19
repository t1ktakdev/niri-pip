# niri-pip

[![CI](https://github.com/t1ktakdev/niri-pip/actions/workflows/ci.yml/badge.svg)](https://github.com/t1ktakdev/niri-pip/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/t1ktakdev/niri-pip)](https://github.com/t1ktakdev/niri-pip/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · **Русский**

Умный Picture-in-Picture, sticky-окна и компактные overlay-окна для Wayland-композитора Niri.

`niri-pip` слушает поток событий Niri, автоматически находит браузерные PiP-окна, держит их плавающими, переносит за активным workspace без кражи фокуса, запоминает свободный ручной размер, умеет безопасно «сворачивать» обычные окна в scratchpad и даёт управление размером, положением, прозрачностью, follow-режимом, блокировкой геометрии и медиа-кнопками.

## Возможности

- Событийный Niri IPC без постоянного опроса `niri msg`.
- Поддержка Chromium PiP, когда Niri показывает пустой `app-id`.
- Готовые детекторы Firefox и Chromium-семейства.
- Следование за workspace через `focus=false`.
- Любая ширина и высота: ручной размер запоминается и не заменяется пресетом.
- Lock/unlock геометрии.
- Пять позиций и точное смещение по пикселям.
- Отдельная прозрачность только для PiP без изменения общих правил iNiR.
- `pin`, `unpin`, `toggle` для обычных окон.
- `overlay` превращает любое окно в компактный sticky-overlay.
- Временный `peek`: увеличить tracked-окно и затем точно вернуть базовую геометрию.
- Восстановление origin: ручное окно возвращается на исходный workspace и в исходный floating/tiling режим; для исходно floating окна восстанавливается прежняя геометрия.
- Именованные overlay-профили для учёбы, звонков, мониторинга и других сценариев.
- Scratchpad-минимизация `minimize` / `restore-minimized` / `restore-all` для обычных окон с восстановлением workspace/layout и сохранением списка через рестарт daemon.
- Опциональное управление MPRIS через `playerctl`.
- Компактный контроллер через fuzzel с fallback на rofi/gum.
- Человеческий Settings UI с автоматическим/русским/английским языком и автосохранением.
- systemd user-service: запускается вместе с графической Niri-сессией и автоматически перезапускается.
- Безопасная интеграция iNiR через отдельный runtime KDL и точечный include.

## Проверенная среда

v0.3.0 прошла полный release-preflight на Arch Linux с Niri 26.04 и Rust 1.97.1: clippy с запретом warnings, весь набор тестов, release build, реальная установка, `doctor` и live-проверка Settings UI в браузере. Во время финального smoke не было открытого auto-PiP, поэтому эта одна разрушающая live-ветка была честно пропущена; точная матрица приведена ниже.

Подробно: [VERIFICATION.md](VERIFICATION.md).

## Установка

### Быстрая установка из GitHub Release

После публикации релиза:

```sh
curl -fsSL https://raw.githubusercontent.com/t1ktakdev/niri-pip/main/scripts/install-release.sh | bash
```

Скрипт скачивает x86_64 release-архив и checksum, проверяет SHA-256, устанавливает всё только для текущего пользователя, включает systemd-service, добавляет launcher и валидирует Niri-интеграцию.

### Сборка из исходников

```sh
git clone https://github.com/t1ktakdev/niri-pip.git
cd niri-pip
./install.sh
```

Для сборки нужны:

- Niri 26.04+
- Rust/Cargo
- systemd user session
- Python 3
- Bash

На Arch Linux:

```sh
sudo pacman -S --needed rust python
```

Для меню и медиа-кнопок:

```sh
sudo pacman -S --needed fuzzel playerctl
```

## Постоянный запуск

После установки `niripip.service` включён для графической пользовательской сессии. Вручную запускать daemon после входа в Niri не нужно.

```sh
systemctl --user status niripip.service
niripip doctor
```

Управление сервисом:

```sh
systemctl --user restart niripip.service
systemctl --user stop niripip.service
journalctl --user -u niripip.service -f
```

Пока активна графическая Niri-сессия, service использует `Restart=always`. Обычный `systemctl --user stop` останавливает его нормально.

## Настройки

Открой **Настройки niri-pip** через launcher приложений или выполни:

```sh
niripip ui
```

Это именно небольшое приложение настроек, а не отдельный центр управления окнами. Размер и позицию настоящего PiP-окна меняй как обычно прямо в Niri; если включено **Запоминать размер и позицию**, niri-pip автоматически учится на этих ручных изменениях.

В UI доступны:

- выбор языка: автоматически / Русский / English;
- автоматическое определение PiP;
- автозапуск через systemd user session;
- запоминание геометрии PiP;
- follow за workspace и выбор follow mode;
- восстановление исходного состояния после unpin;
- защита от перехвата фокуса и сохранение пропорций;
- прозрачность PiP по умолчанию;
- настройка scratchpad-minimize, focus после restore, счётчик свернутых окон и кнопки восстановления;
- статус daemon, Niri IPC, iNiR и desktop entry;
- безопасные готовые Niri bind'ы для копирования.

Настройки сохраняются автоматически. Язык хранится в `~/.config/niri-pip/ui-language`, а поведенческие параметры остаются обычными полями `config.toml`. UI меняет только принадлежащие ему поля и сохраняет остальные ручные настройки.

Удобный хоткей Niri:

```kdl
binds {
    Mod+M { spawn "niripip" "minimize"; }
    Mod+Shift+M { spawn "niripip" "restore-minimized"; }
    Mod+Alt+P { spawn "niripip" "ui"; }
}
```

Старый компактный controller через fuzzel/rofi/gum остаётся доступен командой `niripip menu`.

Если iNiR уже управляет блоком `binds`, добавь только сам binding, а не второй конфликтующий блок.

## Основные команды

```sh
niripip status
niripip list
niripip doctor

niripip minimize
niripip restore-minimized
niripip restore-minimized --window-id 123
niripip restore-all
niripip minimized

niripip size 1131 636
niripip scale 10
niripip scale -10

niripip position top-left
niripip position top-right
niripip position bottom-left
niripip position bottom-right
niripip position center

niripip nudge -20 0
niripip nudge 20 0
niripip nudge 0 -20
niripip nudge 0 20

niripip opacity 100
niripip opacity 80
niripip opacity auto

niripip follow on
niripip follow off
niripip follow-mode follow-workspace
niripip follow-mode follow-focused-output
niripip follow-mode stay-on-output

niripip lock
niripip unlock
niripip reset

niripip preset tiny
niripip preset small
niripip preset medium
niripip preset large
niripip preset cinema
niripip preset movie
niripip preset study

niripip pin
niripip overlay
niripip overlay --profile study

niripip peek
niripip peek on
niripip peek off

niripip unpin
niripip toggle
```

Ручной resize остаётся главным. Пресеты — только быстрые варианты, а не ограничения.

### Minimize / scratchpad

`niripip minimize` переносит текущее обычное окно в динамически созданный scratchpad-workspace и не уводит туда фокус. `niripip restore-minimized` возвращает последнее свернутое окно, `--window-id` — конкретное, а `restore-all` возвращает весь стек. Состояние свернутых окон сохраняется на диск и переживает рестарт `niripipd` внутри той же Niri-сессии. После рестарта Niri или ПК старые live-ID автоматически отбрасываются.

Это отдельная функция, а не перехват родной Wayland-кнопки «свернуть». В Niri 26.04 нет традиционной модели minimize/taskbar, и у части приложений native minimize может визуально подвешивать окно. `niri-pip` не может перехватить этот запрос до Niri, поэтому даёт предсказуемую compositor-side альтернативу через `Mod+M` / `Mod+Shift+M`.

Для floating-окон используется собственная память Niri о позиции и размере между workspace — она возвращает геометрию точнее всего. Для tiled-окон niri-pip дополнительно восстанавливает сохранённые размеры, потому что Niri может потерять высоту при round-trip. Когда последнее управляемое свернутое окно возвращено или закрыто, имя scratchpad снимается автоматически.

### Sticky, overlay и peek

`pin` закрепляет выбранное окно без изменения размера. `overlay` дополнительно применяет
компактный размер и позицию из конфига:

```sh
niripip pin
niripip overlay
niripip overlay --profile study
```

Перед ручным управлением niri-pip сохраняет исходное состояние окна. После `unpin` окно возвращается на исходный workspace и в прежний floating/tiling режим. Исходный размер восстанавливается, когда он был доступен; для изначально floating окна также возвращается нормализованная позиция. Niri пока не даёт ID-addressable action для точного возврата tiled-окна в прежний индекс колонки, поэтому niri-pip намеренно не жонглирует фокусом и не имитирует ненадёжное восстановление.

`peek` — полностью временный режим:

```sh
niripip peek
niripip peek on
niripip peek off
```

Он увеличивает и перемещает tracked-окно, а затем возвращает сохранённую базовую геометрию.
Peek-геометрия не попадает в learned state. Пока peek активен, команды изменения базового
size/position/lock/preset отклоняются, чтобы временное состояние случайно не стало постоянным.

Именованные профили задаются обычными секциями `[profiles.NAME]`: размер, позиция и follow-policy.

### Медиа

Если браузер предоставляет MPRIS и установлен `playerctl`:

```sh
niripip media play-pause
niripip media back 10
niripip media forward 10
niripip media volume-down 5
niripip media volume-up 5
```

Медиа-управление не связано с основным управлением окном: проблема с MPRIS не отключает PiP.

## Прозрачность и iNiR

iNiR может делать неактивные окна слегка прозрачными глобальным rule. `niri-pip` не меняет это правило. Он подключает отдельный файл:

```text
~/.config/niri/niri-pip-runtime.kdl
```

`niripip opacity 100` оставляет PiP полностью непрозрачным. `niripip opacity auto` убирает специальное переопределение и возвращает обычные правила Niri/iNiR.

Интегратор делает backup, меняет только свой marker-блок, запускает `niri validate` и откатывается при ошибке.

## Конфигурация

Основной файл:

```text
~/.config/niri-pip/config.toml
```

Пример: [config/config.example.toml](config/config.example.toml)

Подробно: [docs/ru/configuration.md](docs/ru/configuration.md) · [English](docs/configuration.md)

## Детект браузеров

По умолчанию есть:

- Chromium PiP с `app-id=""`;
- Chrome/Chromium/Brave/Vivaldi/Edge;
- Firefox `Picture-in-Picture`;
- аккуратный fallback по точному PiP-заголовку.

Детекторы настраиваются regex-правилами в TOML. Подробнее: [docs/ru/browsers.md](docs/ru/browsers.md).

## Архитектура

```text
Niri EventStream
      │
      ▼
 адаптер Niri IPC ──────► типизированные события
      │
      ▼
 policy engine ─────────► детект / follow / geometry / focus
      │
      ├───────────────► Niri actions
      │
      ├───────────────► XDG state
      │
      └───────────────► Unix control socket
                               │
                               ▼
                         niripip CLI/menu
```

Проводной IPC-формат отделён от основной логики, поэтому почти всё поведение можно тестировать без запуска композитора.

## Что устанавливается

```text
~/.local/bin/niripip
~/.local/bin/niripipd
~/.local/bin/niripip-menu
~/.local/bin/niripip-integrate
~/.local/bin/niripip-unintegrate
~/.config/niri-pip/config.toml
~/.config/niri-pip/ui-language
~/.config/niri/niri-pip-runtime.kdl
~/.config/systemd/user/niripip.service
~/.local/share/applications/niri-pip.desktop
~/.local/state/niri-pip/state.json
```

Также добавляется небольшой marker-scoped include в `90-user-extra.kdl` iNiR, а если его нет — в основной Niri config.

## Удаление

Оставить настройки и сохранённую геометрию:

```sh
./uninstall.sh
```

Удалить всё, что принадлежит niri-pip:

```sh
./uninstall.sh --purge
```

Uninstaller удаляет только собственный marker-блок и сохраняет timestamped backups.

## Ограничения

- Обычное floating-окно Niri нельзя честно гарантировать поверх сфокусированного true fullscreen. `niri-pip` не маскируется под layer-shell overlay.
- Niri IPC не выдаёт отдельный готовый working-area rectangle, поэтому угловое позиционирование использует work-area-relative move и настраиваемые margins.
- Niri IPC не даёт надёжный XWayland/native-флаг для каждого окна.
- Сложные multi-monitor режимы `follow-focused-output` и `stay-on-output` лучше проверять на конкретной раскладке мониторов.
- Родной minimize приложения — это взаимодействие клиента с Niri, его нельзя перехватить из niri-pip; на затронутых версиях используй scratchpad-команды/бинды.
- MPRIS зависит от браузера, сайта и медиаплеера.

## Разработка

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --release --workspace
./scripts/release-preflight.sh
```

## Документация

- [Русская документация](docs/ru/README.md)
- [Конфигурация](docs/ru/configuration.md)
- [Браузеры](docs/ru/browsers.md)
- [Решение проблем](docs/ru/troubleshooting.md)
- [English documentation](README.md)

## Лицензия

MIT. См. [LICENSE](LICENSE).
