# Конфигурация

Основной файл:

```text
~/.config/niri-pip/config.toml
```

При первой установке он создаётся из `config/config.example.toml`. Повторная установка не перезаписывает существующий пользовательский конфиг.

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
