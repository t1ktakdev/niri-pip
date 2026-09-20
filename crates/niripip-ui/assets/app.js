"use strict";

const TOKEN = document.querySelector('meta[name="niripip-token"]').content;
const $ = (id) => document.getElementById(id);

const TEXT = {
  ru: {
    navGeneral:"Общие", navBehavior:"Поведение", navIntegration:"Интеграция",
    navShortcuts:"Горячие клавиши", navAbout:"О программе",
    daemonLabel:"Daemon", niriLabel:"Niri IPC", tagline:"Маленькие окна.<br>Большие возможности.",
    generalTitle:"Общие", generalSubtitle:"Основные настройки niri-pip. Просто и удобно.",
    languageTitle:"Язык интерфейса", languageDesc:"Выберите язык приложения",
    autoDetectTitle:"Автоматически определять PiP", autoDetectDesc:"Автоматически распознавать PiP-окна",
    autostartTitle:"Запускать вместе с Niri", autostartDesc:"Автоматический запуск при старте сессии",
    rememberGeometryTitle:"Запоминать размер и позицию", rememberGeometryDesc:"Учиться на ваших ручных изменениях PiP-окна",
    opacityTitle:"Прозрачность по умолчанию", opacityDesc:"Прозрачность новых PiP-окон",
    advancedTitle:"Дополнительные настройки",
    advancedGeneralNote:"Редкие технические параметры остаются в config.toml, чтобы основной экран был простым.",
    geometryNote:"Размер и позиция PiP запоминаются автоматически, когда вы меняете окно вручную.",
    resetGeometry:"Сбросить размер и позицию", resetConfirm:"Нажмите ещё раз для сброса",
    openConfig:"Открыть config.toml",
    behaviorTitle:"Поведение", behaviorSubtitle:"Как PiP ведёт себя при переключении workspace и управлении окнами.",
    followTitle:"Следовать за workspace", followDesc:"Перемещать PiP вместе с рабочим пространством",
    followModeTitle:"Режим follow", followModeDesc:"Куда переносить PiP при смене фокуса",
    followWorkspace:"Текущий workspace", followFocusedOutput:"Активный монитор", stayOnOutput:"Оставаться на мониторе",
    restoreTitle:"Восстанавливать исходное состояние", restoreDesc:"После unpin вернуть workspace, tiling/floating и размер",
    focusTitle:"Не красть фокус", focusDesc:"PiP появляется, не перебивая текущее окно",
    aspectTitle:"Сохранять пропорции", aspectDesc:"Сохранять соотношение сторон при автоматическом размере",
    behaviorNote:"Настройки применяются сразу после автосохранения.",
    minimizeSectionTitle:"Скрытые окна",
    minimizeSectionDesc:"Hide убирает окно из обычной работы, но приложение продолжает работать в фоне.",
    minimizeEnabledTitle:"Скрытие окон",
    minimizeEnabledDesc:"Переносит окно в защищённый служебный workspace до возврата",
    minimizeFocusTitle:"Фокусировать окно после возврата",
    minimizeFocusDesc:"После Restore перейти к возвращённому окну",
    minimizedNowTitle:"Скрыто сейчас",
    minimizedNone:"Нет скрытых окон",
    minimizedOne:"Скрыто: {title}",
    minimizedMany:"Скрыто окон: {count}. Последнее: {title}",
    restoreLast:"Вернуть последнее",
    restoreAll:"Вернуть все",
    restoredLast:"Окно восстановлено",
    restoredAll:"Все окна восстановлены",
    compatTitle:"Совместимость Niri",
    compatAffected:"есть проблема",
    compatUnknown:"отдельный режим",
    compatAffectedText:"В Niri 26.04 родная кнопка «свернуть» может подвешивать приложения. Используйте Hide: Mod+Alt+M, вернуть: Mod+Alt+U.",
    compatOtherText:"Hide — безопасная эмуляция: окно уходит с обычного workspace, приложение остаётся запущенным. Это не native Wayland minimize.",
    integrationTitle:"Интеграция", integrationSubtitle:"Состояние daemon, Niri IPC и пользовательской установки.",
    autostartStatusTitle:"Автозапуск", autostartStatusDesc:"systemd --user",
    inirDesc:"Runtime KDL include", desktopEntryTitle:"Меню приложений",
    restartDaemon:"Перезапустить daemon", configPathLabel:"Конфигурация",
    shortcutsTitle:"Горячие клавиши", shortcutsSubtitle:"Проверяем активный Niri config и показываем, свободна ли комбинация.",
    shortcutsNote:"Hotkeys Hide используют Mod+Alt+M и Mod+Alt+U, чтобы не конфликтовать с обычным maximize/mute.",
    aboutTitle:"О программе", aboutSubtitle:"Небольшой companion для Picture-in-Picture в Niri.",
    aboutCopy:"Автоматический PiP, sticky-окна и аккуратное восстановление состояния для Niri.",
    documentation:"Документация ↗", reportIssue:"Сообщить о проблеме ↗",
    connected:"подключено", disconnected:"нет связи", running:"работает", stopped:"остановлен",
    enabled:"включён", disabled:"выключен", installed:"установлено", missing:"не найдено",
    saving:"Сохранение…", saved:"Сохранено", saveError:"Не удалось сохранить",
    copied:"Скопировано", copy:"Копировать", geometryReset:"Сохранённая геометрия сброшена",
    daemonRestarted:"Daemon перезапущен", configOpened:"config.toml открыт",
    settingsShortcut:"Открыть настройки", hideShortcut:"Скрыть окно", restoreHiddenShortcut:"Вернуть скрытое", toggleShortcut:"Toggle PiP", peekShortcut:"Peek", restoreShortcut:"Восстановить", shortcutInstalled:"установлено", shortcutFree:"свободно", shortcutConflict:"конфликт", restoreThis:"Вернуть",
    automaticRussian:"Автоматически (Русский)", automaticEnglish:"Автоматически (English)"
  },
  en: {
    navGeneral:"General", navBehavior:"Behavior", navIntegration:"Integration",
    navShortcuts:"Shortcuts", navAbout:"About",
    daemonLabel:"Daemon", niriLabel:"Niri IPC", tagline:"Small windows.<br>More possibilities.",
    generalTitle:"General", generalSubtitle:"The everyday niri-pip settings. Simple and clear.",
    languageTitle:"Interface language", languageDesc:"Choose the language used by the settings app",
    autoDetectTitle:"Detect PiP automatically", autoDetectDesc:"Recognize supported Picture-in-Picture windows",
    autostartTitle:"Start with Niri", autostartDesc:"Start automatically with the graphical session",
    rememberGeometryTitle:"Remember size and position", rememberGeometryDesc:"Learn from manual PiP window changes",
    opacityTitle:"Default opacity", opacityDesc:"Opacity used for new PiP windows",
    advancedTitle:"Advanced settings",
    advancedGeneralNote:"Rare technical options stay in config.toml so the main settings remain simple.",
    geometryNote:"PiP size and position are learned automatically when you resize or move the real window.",
    resetGeometry:"Reset size and position", resetConfirm:"Click again to reset",
    openConfig:"Open config.toml",
    behaviorTitle:"Behavior", behaviorSubtitle:"How PiP behaves when workspaces and windows change.",
    followTitle:"Follow workspace", followDesc:"Move PiP when workspace focus changes",
    followModeTitle:"Follow mode", followModeDesc:"Where PiP follows when focus changes",
    followWorkspace:"Current workspace", followFocusedOutput:"Focused output", stayOnOutput:"Stay on output",
    restoreTitle:"Restore original state", restoreDesc:"After unpin, restore workspace, tiling/floating and size",
    focusTitle:"Never steal focus", focusDesc:"Show PiP without interrupting the current window",
    aspectTitle:"Preserve aspect ratio", aspectDesc:"Preserve video proportions during automatic sizing",
    behaviorNote:"Settings apply immediately after autosave.",
    minimizeSectionTitle:"Hidden windows",
    minimizeSectionDesc:"Hide removes a window from normal workspace use while the application keeps running.",
    minimizeEnabledTitle:"Hide windows",
    minimizeEnabledDesc:"Parks the window on a guarded service workspace until restored",
    minimizeFocusTitle:"Focus returned window",
    minimizeFocusDesc:"Focus a window after Restore",
    minimizedNowTitle:"Hidden now",
    minimizedNone:"No hidden windows",
    minimizedOne:"Hidden: {title}",
    minimizedMany:"Hidden: {count}. Latest: {title}",
    restoreLast:"Restore latest",
    restoreAll:"Restore all",
    restoredLast:"Window restored",
    restoredAll:"All windows restored",
    compatTitle:"Niri compatibility",
    compatAffected:"affected",
    compatUnknown:"separate mode",
    compatAffectedText:"On Niri 26.04, application-native minimize can freeze some clients. Use Hide: Mod+Alt+M, restore: Mod+Alt+U.",
    compatOtherText:"Hide is a safe emulation: the window leaves normal workspace use while the app stays running. It is not native Wayland minimize.",
    integrationTitle:"Integration", integrationSubtitle:"Daemon, Niri IPC and user installation status.",
    autostartStatusTitle:"Autostart", autostartStatusDesc:"systemd --user",
    inirDesc:"Runtime KDL include", desktopEntryTitle:"Application menu",
    restartDaemon:"Restart daemon", configPathLabel:"Configuration",
    shortcutsTitle:"Shortcuts", shortcutsSubtitle:"Active Niri config is checked so you can see whether a shortcut is free.",
    shortcutsNote:"Hide uses Mod+Alt+M and Mod+Alt+U to avoid common maximize and mute bindings.",
    aboutTitle:"About", aboutSubtitle:"A small Picture-in-Picture companion for Niri.",
    aboutCopy:"Automatic PiP, sticky windows and careful state restoration for Niri.",
    documentation:"Documentation ↗", reportIssue:"Report an issue ↗",
    connected:"connected", disconnected:"offline", running:"running", stopped:"stopped",
    enabled:"enabled", disabled:"disabled", installed:"installed", missing:"missing",
    saving:"Saving…", saved:"Saved", saveError:"Could not save",
    copied:"Copied", copy:"Copy", geometryReset:"Remembered geometry reset",
    daemonRestarted:"Daemon restarted", configOpened:"config.toml opened",
    settingsShortcut:"Open settings", hideShortcut:"Hide window", restoreHiddenShortcut:"Restore hidden", toggleShortcut:"Toggle PiP", peekShortcut:"Peek", restoreShortcut:"Restore", shortcutInstalled:"installed", shortcutFree:"free", shortcutConflict:"conflict", restoreThis:"Restore",
    automaticRussian:"Automatic (Russian)", automaticEnglish:"Automatic (English)"
  }
};

const state = {
  bootstrap: null,
  languagePref: "auto",
  activePage: "general",
  saveTimer: null,
  saveSeq: 0,
  busy: false,
  resetArmed: false
};

function systemLanguage() {
  return (navigator.language || "en").toLowerCase().startsWith("ru") ? "ru" : "en";
}

function locale() {
  return state.languagePref === "auto" ? systemLanguage() : state.languagePref;
}

function t(key) {
  const lang = locale();
  return TEXT[lang][key] || TEXT.en[key] || key;
}

async function api(path, options = {}) {
  const headers = new Headers(options.headers || {});
  headers.set("X-NiriPip-Token", TOKEN);
  if (options.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  const response = await fetch(path, { ...options, headers, cache: "no-store" });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(data.error || `HTTP ${response.status}`);
  }
  return data;
}

function setBusy(value) {
  state.busy = value;
  document.body.classList.toggle("busy", value);
}

function showSaveState(kind, message) {
  const node = $("saveIndicator");
  node.className = `save-indicator visible ${kind || ""}`;
  node.textContent = message;
  window.clearTimeout(showSaveState.timer);
  showSaveState.timer = window.setTimeout(() => {
    node.classList.remove("visible");
  }, kind === "error" ? 4200 : 1800);
}

function toast(message, kind = "") {
  const node = document.createElement("div");
  node.className = `toast ${kind}`;
  node.textContent = message;
  $("toastStack").append(node);
  window.setTimeout(() => node.remove(), 3200);
}

async function refresh(silent = false) {
  try {
    state.bootstrap = await api("/api/bootstrap");
    state.languagePref = state.bootstrap.language || "auto";
    render();
    $("loadingScreen").classList.add("done");
  } catch (error) {
    $("loadingScreen").classList.add("done");
    if (!silent) toast(error.message, "error");
  }
}

function render() {
  applyLanguage();
  renderPages();
  if (!state.bootstrap) return;
  renderSettings();
  renderIntegration();
  renderShortcuts();
  $("aboutVersion").textContent = `v${state.bootstrap.version}`;
}

function applyLanguage() {
  document.documentElement.lang = locale();

  document.querySelectorAll("[data-i18n]").forEach((node) => {
    const value = t(node.dataset.i18n);
    if (value.includes("<br>")) node.innerHTML = value;
    else node.textContent = value;
  });

  const language = $("languageSelect");
  const sys = systemLanguage();
  language.options[0].textContent = sys === "ru" ? t("automaticRussian") : t("automaticEnglish");
  language.options[1].textContent = "Русский";
  language.options[2].textContent = "English";
  language.value = state.languagePref;
}

function renderPages() {
  const valid = ["general", "behavior", "integration", "shortcuts", "about"];
  if (!valid.includes(state.activePage)) state.activePage = "general";

  document.querySelectorAll(".nav-item").forEach((button) => {
    button.classList.toggle("active", button.dataset.page === state.activePage);
  });
  document.querySelectorAll(".page").forEach((page) => {
    page.classList.toggle("active", page.dataset.pagePanel === state.activePage);
  });
}

function renderSettings() {
  const settings = state.bootstrap.settings;
  $("autoDetectToggle").checked = settings.auto_detect;
  $("autostartToggle").checked = state.bootstrap.integration.autostart_enabled;
  $("rememberGeometryToggle").checked = settings.remember_geometry;
  $("opacityRange").value = String(settings.opacity_percent);
  $("opacityValue").textContent = `${settings.opacity_percent}%`;

  $("followToggle").checked = settings.follow_workspace;
  $("followModeSelect").value = settings.follow_mode;
  $("followModeSelect").disabled = !settings.follow_workspace;
  $("restoreToggle").checked = settings.restore_layout_on_unpin;
  $("focusToggle").checked = settings.prevent_focus_stealing;
  $("aspectToggle").checked = settings.preserve_aspect_ratio;
  $("minimizeEnabledToggle").checked = settings.minimize_enabled;
  $("minimizeFocusToggle").checked = settings.minimize_restore_focus;
  $("minimizeFocusToggle").disabled = !settings.minimize_enabled;
  renderMinimize();
}

function setBadge(id, ok, goodText, badText) {
  const node = $(id);
  node.className = `state-badge ${ok ? "ok" : "bad"}`;
  node.textContent = ok ? goodText : badText;
}

function renderIntegration() {
  const info = state.bootstrap.integration;

  $("daemonDot").classList.toggle("ok", info.daemon_running);
  $("niriDot").classList.toggle("ok", info.niri_connected);
  $("daemonStatus").textContent = info.daemon_running ? t("connected") : t("disconnected");
  $("niriStatus").textContent = info.niri_connected ? t("connected") : t("disconnected");

  $("daemonDetail").textContent = info.daemon_version ? `v${info.daemon_version}` : "niripip.service";
  $("niriDetail").textContent = info.niri_version || "Niri IPC";
  setBadge("daemonBadge", info.daemon_running, t("running"), t("stopped"));
  setBadge("niriBadge", info.niri_connected, t("connected"), t("disconnected"));
  setBadge("autostartBadge", info.autostart_enabled, t("enabled"), t("disabled"));
  setBadge("inirBadge", info.inir_integrated, t("installed"), t("missing"));
  setBadge("desktopBadge", info.desktop_entry_installed, t("installed"), t("missing"));
  $("configPath").textContent = info.config_path;
}

function renderMinimize() {
  const windows = state.bootstrap.minimized || [];
  const count = windows.length;
  const badge = $("minimizedCountBadge");
  badge.textContent = String(count);
  badge.className = `state-badge ${count > 0 ? "ok" : ""}`;

  const summary = $("minimizedWindowSummary");
  if (count === 0) {
    summary.textContent = t("minimizedNone");
  } else {
    const latest = windows[count - 1];
    const title = latest.title || latest.app_id || `#${latest.id}`;
    const template = count === 1 ? t("minimizedOne") : t("minimizedMany");
    summary.textContent = template
      .replace("{count}", String(count))
      .replace("{title}", title);
  }

  const hiddenList = $("hiddenWindowList");
  hiddenList.replaceChildren();
  for (const windowInfo of windows.slice().reverse()) {
    const row = document.createElement("div");
    row.className = "hidden-window-row";

    const copy = document.createElement("div");
    copy.className = "hidden-window-copy";

    const title = document.createElement("strong");
    title.textContent = windowInfo.title || windowInfo.app_id || `#${windowInfo.id}`;

    const meta = document.createElement("span");
    meta.textContent = `#${windowInfo.id} · ${windowInfo.was_floating ? "floating" : "tiled"} · ws ${windowInfo.origin_workspace_id ?? "—"}`;

    const restore = document.createElement("button");
    restore.className = "button secondary";
    restore.type = "button";
    restore.textContent = t("restoreThis");
    restore.addEventListener("click", () => restoreMinimized(false, windowInfo.id, restore));

    copy.append(title, meta);
    row.append(copy, restore);
    hiddenList.append(row);
  }

  $("restoreLastButton").disabled = count === 0;
  $("restoreAllButton").disabled = count === 0;

  const version = state.bootstrap.integration.niri_version || "";
  const affected = /^26\.04(?:\s|$|\()/.test(version);
  const compatBadge = $("nativeMinimizeBadge");
  compatBadge.className = `state-badge ${affected ? "bad" : ""}`;
  compatBadge.textContent = affected ? t("compatAffected") : t("compatUnknown");
  $("nativeMinimizeText").textContent = affected ? t("compatAffectedText") : t("compatOtherText");
}

function shortcutTitle(name) {
  return t({
    settings: "settingsShortcut",
    hide: "hideShortcut",
    "restore-hidden": "restoreHiddenShortcut",
    toggle: "toggleShortcut",
    peek: "peekShortcut",
    restore: "restoreShortcut"
  }[name] || name);
}

function kdlFor(shortcut) {
  const parts = shortcut.command.trim().split(/\s+/);
  const args = parts.slice(1).map((part) => ` "${part.replaceAll('"', '\\"')}"`).join("");
  return `${shortcut.suggested_bind} { spawn "${parts[0]}"${args}; }`;
}

function renderShortcuts() {
  const list = $("shortcutList");
  list.replaceChildren();

  for (const shortcut of state.bootstrap.shortcuts) {
    const row = document.createElement("div");
    row.className = "shortcut-row";

    const name = document.createElement("span");
    name.className = "shortcut-name";
    name.textContent = shortcutTitle(shortcut.name);

    const command = document.createElement("code");
    command.className = "shortcut-command";
    command.textContent = shortcut.command;

    const key = document.createElement("span");
    key.className = "keycap";
    key.textContent = shortcut.suggested_bind;

    const status = document.createElement("span");
    status.className = `state-badge ${shortcut.conflict ? "bad" : shortcut.installed ? "ok" : ""}`;
    status.textContent = shortcut.conflict
      ? t("shortcutConflict")
      : shortcut.installed
        ? t("shortcutInstalled")
        : t("shortcutFree");

    const copy = document.createElement("button");
    copy.className = "copy-button";
    copy.type = "button";
    copy.textContent = t("copy");
    copy.disabled = shortcut.conflict;
    copy.addEventListener("click", () => copyText(kdlFor(shortcut)));

    row.append(name, command, key, status, copy);
    list.append(row);
  }
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.append(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
  toast(t("copied"), "success");
}

function settingsFromControls() {
  return {
    auto_detect: $("autoDetectToggle").checked,
    remember_geometry: $("rememberGeometryToggle").checked,
    follow_workspace: $("followToggle").checked,
    follow_mode: $("followModeSelect").value,
    restore_layout_on_unpin: $("restoreToggle").checked,
    prevent_focus_stealing: $("focusToggle").checked,
    preserve_aspect_ratio: $("aspectToggle").checked,
    minimize_enabled: $("minimizeEnabledToggle").checked,
    minimize_restore_focus: $("minimizeFocusToggle").checked,
    opacity_percent: Number($("opacityRange").value)
  };
}

function scheduleSettingsSave(delay = 140) {
  window.clearTimeout(state.saveTimer);
  const seq = ++state.saveSeq;
  showSaveState("", t("saving"));

  state.saveTimer = window.setTimeout(async () => {
    const payload = settingsFromControls();
    try {
      const saved = await api("/api/settings", {
        method: "PUT",
        body: JSON.stringify(payload)
      });
      if (seq !== state.saveSeq) return;
      state.bootstrap.settings = saved;
      renderSettings();
      showSaveState("saved", "✓ " + t("saved"));
    } catch (error) {
      if (seq !== state.saveSeq) return;
      showSaveState("error", t("saveError"));
      toast(error.message, "error");
      await refresh(true);
    }
  }, delay);
}

async function setAutostart(enabled) {
  $("autostartToggle").disabled = true;
  showSaveState("", t("saving"));
  try {
    const integration = await api("/api/autostart", {
      method: "POST",
      body: JSON.stringify({ enabled })
    });
    state.bootstrap.integration = integration;
    renderIntegration();
    $("autostartToggle").checked = integration.autostart_enabled;
    showSaveState("saved", "✓ " + t("saved"));
  } catch (error) {
    $("autostartToggle").checked = state.bootstrap.integration.autostart_enabled;
    showSaveState("error", t("saveError"));
    toast(error.message, "error");
  } finally {
    $("autostartToggle").disabled = false;
  }
}

async function restartDaemon() {
  const button = $("restartDaemonButton");
  button.disabled = true;
  try {
    const integration = await api("/api/restart-daemon", { method: "POST" });
    state.bootstrap.integration = integration;
    renderIntegration();
    toast(t("daemonRestarted"), "success");
  } catch (error) {
    toast(error.message, "error");
  } finally {
    button.disabled = false;
  }
}

async function openConfig() {
  try {
    await api("/api/open-config", { method: "POST" });
    toast(t("configOpened"), "success");
  } catch (error) {
    toast(error.message, "error");
  }
}

async function resetGeometry() {
  const button = $("resetGeometryButton");
  if (!state.resetArmed) {
    state.resetArmed = true;
    button.textContent = t("resetConfirm");
    window.setTimeout(() => {
      if (!state.resetArmed) return;
      state.resetArmed = false;
      button.textContent = t("resetGeometry");
    }, 3200);
    return;
  }

  state.resetArmed = false;
  button.disabled = true;
  try {
    await api("/api/reset-geometry", { method: "POST" });
    button.textContent = t("resetGeometry");
    toast(t("geometryReset"), "success");
  } catch (error) {
    toast(error.message, "error");
  } finally {
    button.disabled = false;
  }
}

async function restoreMinimized(all, windowId = null, sourceButton = null) {
  const button = sourceButton || (all ? $("restoreAllButton") : $("restoreLastButton"));
  button.disabled = true;
  try {
    const endpoint = all
      ? "/api/restore-all-minimized"
      : windowId == null
        ? "/api/restore-minimized"
        : `/api/restore-hidden/${windowId}`;
    const data = await api(endpoint, { method: "POST" });
    state.bootstrap = data;
    render();
    toast(all ? t("restoredAll") : t("restoredLast"), "success");
  } catch (error) {
    toast(error.message, "error");
  } finally {
    renderMinimize();
  }
}

function bindEvents() {
  document.querySelectorAll(".nav-item").forEach((button) => {
    button.addEventListener("click", () => {
      state.activePage = button.dataset.page;
      renderPages();
    });
  });

  $("languageSelect").addEventListener("change", async (event) => {
    const previous = state.languagePref;
    const requested = event.target.value;
    state.languagePref = requested;
    applyLanguage();
    showSaveState("", t("saving"));

    try {
      const saved = await api("/api/language", {
        method: "PUT",
        body: JSON.stringify({ language: requested })
      });
      state.languagePref = saved.language;
      applyLanguage();
      showSaveState("saved", "✓ " + t("saved"));
    } catch (error) {
      state.languagePref = previous;
      applyLanguage();
      showSaveState("error", t("saveError"));
      toast(error.message, "error");
    }
  });

  ["autoDetectToggle", "rememberGeometryToggle", "restoreToggle", "focusToggle", "aspectToggle",
   "minimizeEnabledToggle", "minimizeFocusToggle"]
    .forEach((id) => $(id).addEventListener("change", () => {
      $("minimizeFocusToggle").disabled = !$("minimizeEnabledToggle").checked;
      scheduleSettingsSave();
    }));

  $("followToggle").addEventListener("change", () => {
    $("followModeSelect").disabled = !$("followToggle").checked;
    scheduleSettingsSave();
  });
  $("followModeSelect").addEventListener("change", () => scheduleSettingsSave());

  $("opacityRange").addEventListener("input", () => {
    $("opacityValue").textContent = `${$("opacityRange").value}%`;
  });
  $("opacityRange").addEventListener("change", () => scheduleSettingsSave(40));

  $("autostartToggle").addEventListener("change", (event) => setAutostart(event.target.checked));
  $("restartDaemonButton").addEventListener("click", restartDaemon);
  $("openConfigButtonGeneral").addEventListener("click", openConfig);
  $("openConfigButtonIntegration").addEventListener("click", openConfig);
  $("resetGeometryButton").addEventListener("click", resetGeometry);
  $("restoreLastButton").addEventListener("click", () => restoreMinimized(false));
  $("restoreAllButton").addEventListener("click", () => restoreMinimized(true));
}

async function boot() {
  bindEvents();
  applyLanguage();
  await refresh();

  window.setInterval(() => {
    api("/api/ping").catch(() => {});
  }, 20000);

  window.setInterval(() => {
    if (!state.busy && (state.activePage === "integration" || state.activePage === "behavior")) refresh(true);
  }, 3000);
}

boot();
