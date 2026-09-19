# Конфигурация

Основной файл:

```text
~/.config/niri-pip/config.toml
```

При первой установке он создаётся из `config/config.example.toml`. Повторная установка не перезаписывает существующий пользовательский конфиг.

## Settings UI

Команда `niripip ui` открывает обычное окно настроек. Там нет списка окон и ручного ввода ширины/высоты:
реальное PiP-окно двигается и растягивается прямо в Niri, а при `remember_geometry = true` niri-pip
запоминает эту геометрию сам.

UI автосохраняет только принадлежащие ему поля и не удаляет остальные ручные секции TOML.
Язык хранится отдельно в `~/.config/niri-pip/ui-language`: `auto`, `ru` или `en`.

## Основные параметры

### `[general]`

- `enabled` — включает управление окнами.
- `auto_detect` — автоматически определяет PiP.
- `follow_workspace` — разрешает следование за workspace.
- `follow_mode` — `follow-workspace`, `follow-focused-output` или `stay-on-output`.
- `remember_geometry` — запоминает ручной размер и положение PiP.
- `detection_threshold` — минимальный score детектора.
- `restore_layout_on_unpin` — возвращает вручную pinned tiled-окно обратно в tiling после `unpin`.
- `action_suppression_ms` — короткое окно подавления собственных geometry-событий daemon.
- `workspace_debounce_ms` — debounce переключения workspace.
- `focus_restore_window_ms` — период, в котором daemon может вернуть предыдущий фокус, если только что созданный PiP украл его.

### `[pip]`

- `position` — `top-left`, `top-right`, `bottom-left`, `bottom-right`, `center`.
- `position_mode = "remember"` — учить ручную геометрию.
- `profile` — стартовый preset для нового PiP.
- `width`, `height` — custom-размер.
- `gap` — отступ от выбранной позиции.
- `steal_focus` — должен оставаться `false`.
- `preserve_aspect_ratio` — сохранять наблюдаемое соотношение сторон при первичном размере.

Ручной resize уже открытого PiP важнее стартового preset. Если включён `lock`, зафиксированная геометрия намеренно становится главной.

### `[overlay]`

Настройки `overlay` для произвольного окна:

```toml
[overlay]
position = "bottom-right"
width = 520
height = 340
follow_workspace = true
follow_mode = "follow-workspace"
```

`niripip pin` только делает окно sticky/floating и не меняет его размер. `niripip overlay` дополнительно применяет размер и позицию `[overlay]`.

До начала ручного управления daemon сохраняет исходный workspace, floating/tiling режим и текущий размер. Для изначально floating окна дополнительно сохраняется нормализованная позиция. `niripip unpin` возвращает окно в это исходное состояние, насколько это позволяет Niri IPC.

Точный прежний индекс tiled-колонки не обещается: у Niri сейчас нет ID-addressable action для такого восстановления. niri-pip не использует скрытое переключение фокуса ради имитации этой возможности.

### `[peek]`

Временное увеличение уже tracked-окна:

```toml
[peek]
position = "center"
width = 960
height = 540
```

```sh
niripip peek
niripip peek on
niripip peek off
```

Перед входом сохраняется базовая геометрия; после выхода она восстанавливается. Peek-события не записываются как learned PiP geometry. Пока peek активен, команды изменения базового size/position, lock/reset/preset отклоняются; follow-policy менять можно.

### Hide / `[minimize]`

```toml
[minimize]
enabled = true
scratchpad_name = "niri-pip:scratchpad"
restore_focus = true
```

Имя таблицы `[minimize]` сохраняется для совместимости с v0.3.0, но пользовательская функция теперь называется **Hide**. `niripip hide` переносит текущее обычное окно на динамически именованный защищённый служебный workspace через Niri IPC с `focus=false`. Постоянно добавлять workspace в `config.kdl` не нужно.

```sh
niripip hide
niripip restore-hidden
niripip restore-hidden --window-id 123
niripip restore-all-hidden
niripip hidden
```

Стек скрытых окон хранится в runtime state и переживает рестарт daemon внутри той же compositor-сессии. В state сохраняется ключ Niri-сессии из Linux boot ID и `NIRI_SOCKET`; при рестарте Niri или ПК старые live-ID удаляются до запуска engine. Stale-записи внутри текущей сессии дополнительно чистятся по authoritative snapshot Niri. Restore возвращает исходный workspace и floating/tiling режим. Floating-геометрию точнее всего сохраняет сам Niri, а tiled-размер niri-pip восстанавливает явно.

Операции transactional: persistent state коммитится только после успешного выполнения всех запланированных Niri IPC actions. При частичном сбое niri-pip выполняет best-effort компенсацию и не меняет сохранённый стек. Если пользователь случайно фокусирует служебный workspace со скрытыми окнами, daemon сразу возвращает предыдущий нормальный workspace.

Это не родной Wayland minimize: Niri 26.04 не даёт native hide/minimize IPC-action, а запрос клиента `set_minimized` идёт напрямую в Niri. Старые команды v0.3.0 остаются aliases.

### `[profiles.NAME]`

Именованный overlay-профиль использует те же поля, что `[overlay]`:

```toml
[profiles.study]
position = "top-right"
width = 700
height = 420
follow_workspace = true
follow_mode = "follow-workspace"
```

Применение:

```sh
niripip overlay --profile study
```

Неизвестное имя профиля возвращает ошибку до изменения окна.

### `[margins]`

Дополнительные безопасные отступы для позиционирования. Обычно iNiR layer-shell зоны уже учтены Niri, поэтому большие значения не нужны.

### `[browsers]`

Включают и выключают встроенные browser-specific детекторы.

### `[[detectors]]`

Каждый детектор может задавать regex заголовка/app-id, ограничения размера, базовый score и бонусы. Побеждает самый высокий подходящий score выше `detection_threshold`.

После ручного редактирования:

```sh
niripip reload
niripip doctor
```
