# Браузеры и PiP

## Chromium / Chrome

На Niri через XWayland/xwayland-satellite PiP может появляться как:

```text
title="Picture in picture"
app-id=""
```

Поэтому `niri-pip` не полагается только на browser app-id. Встроенный `chromium-empty-app-id` специально покрывает этот случай.

## Firefox

По умолчанию поддерживается точный PiP-заголовок Firefox и его обычные app-id варианты.

## Brave / Vivaldi / Edge

Они покрываются Chromium-family детектором, когда Niri передаёт browser identity.

## Локализованные заголовки

Если браузер использует другой точный заголовок, добавь свой detector в `~/.config/niri-pip/config.toml`:

```toml
[[detectors]]
name = "localized-pip"
action = "pip"
title_regex = "(?i)^картинка в картинке$"
max_width = 1280
max_height = 900
score = 130
```

Затем:

```sh
niripip reload
```

Не делай слишком широкие regex: обычное окно браузера не должно ошибочно считаться PiP.
