# Решение проблем

## Быстрая диагностика

```sh
niripip doctor
niripip status
systemctl --user status niripip.service --no-pager -l
journalctl --user -u niripip.service -n 100 --no-pager
```

## `NIRI_SOCKET` отсутствует

Проверь:

```sh
echo "$NIRI_SOCKET"
niri msg version
```

Для нормальной systemd-сессии Niri рекомендуется запуск через `niri-session` или session-режим Niri, чтобы окружение было импортировано в user manager.

## Service active, но CLI не подключается

Проверь сокет:

```sh
ls -la "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/niri-pip/"
```

Перезапусти:

```sh
systemctl --user restart niripip.service
sleep 1
niripip doctor
```

## PiP не определяется

Посмотри, как окно видит Niri:

```sh
niri msg windows
```

Сравни `title` и `app-id` с detector rules. Для Chromium отдельный случай `app-id=""` уже поддерживается.

## PiP стал прозрачным

```sh
niripip opacity 100
```

Вернуть обычные правила Niri/iNiR:

```sh
niripip opacity auto
```

Проверить runtime rule:

```sh
cat ~/.config/niri/niri-pip-runtime.kdl
niri validate
```

## Размер возвращается назад

Проверь lock:

```sh
niripip status
niripip unlock
```

Без lock ручной resize должен запоминаться. С lock daemon намеренно возвращает зафиксированную геометрию.

## Сброс управления PiP

```sh
niripip reset
```

## Полный debug bundle

Из репозитория:

```sh
./scripts/collect-debug.sh
```

Просмотри файл перед публикацией в issue.
