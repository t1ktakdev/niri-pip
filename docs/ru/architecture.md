# Архитектура

`niripipd` — пользовательский daemon. Он держит отдельное долговременное подключение к Niri EventStream и отдельные короткие IPC-подключения для действий.

Основные части:

- `niripip-core` — состояние, scoring detector, follow, geometry, focus policy;
- `niripip-ipc` — wire format Niri, real/mock backend;
- `niripip-daemon` — EventStream supervisor, Unix socket, persistence, runtime KDL;
- `niripip-cli` — команды, doctor, media и запуск menu;
- `integrations/inir/niripip-menu` — внешний UI-контроллер.

Daemon не опрашивает `niri msg` циклом. После подключения EventStream получает authoritative snapshot, затем применяет события. Уже открытый PiP при bootstrap сохраняет свою живую ручную геометрию, кроме случая explicit geometry lock.

Команды CLI идут через Unix socket в `$XDG_RUNTIME_DIR/niri-pip/`. TCP/UDP портов нет.
