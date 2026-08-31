/* Settings.
 *
 * The form is bound to the config object by data-path attributes rather than
 * by hand-written getters, so adding a setting is a matter of adding a field
 * to the HTML and to the Rust struct. */

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const els = {
  status: document.getElementById("status"),
  save: document.getElementById("save"),
  reset: document.getElementById("reset"),
  preset: document.getElementById("preset"),
  preview: document.getElementById("preview"),
  notes: document.getElementById("notes"),
  sourceNote: document.getElementById("source-note"),
  filesNote: document.getElementById("files-note"),
};

let config = null;
let presets = [];
let dirty = false;
let lastWarnings = [];

const fields = Array.from(document.querySelectorAll("[data-path]"));

function readPath(object, path) {
  return path.split(".").reduce((node, key) => (node == null ? node : node[key]), object);
}

function writePath(object, path, value) {
  const keys = path.split(".");
  const last = keys.pop();
  const parent = keys.reduce((node, key) => node[key], object);
  parent[last] = value;
}

/* Escapes are stored as the two characters a text input can hold. A real
 * newline in the field would break the single-line inputs the format section
 * is built from. */
function toField(value) {
  return typeof value === "string" ? value : String(value);
}

function fillForm() {
  for (const field of fields) {
    const value = readPath(config, field.dataset.path);
    if (field.type === "checkbox") {
      field.checked = Boolean(value);
    } else if (field.type === "number") {
      field.value = Number(value);
    } else {
      field.value = toField(value);
    }
  }
  matchPreset();
}

function collect(field) {
  if (field.type === "checkbox") return field.checked;
  if (field.type === "number") {
    const n = Number.parseInt(field.value, 10);
    if (Number.isNaN(n)) return readPath(config, field.dataset.path);
    const min = field.min === "" ? -Infinity : Number(field.min);
    const max = field.max === "" ? Infinity : Number(field.max);
    return Math.min(Math.max(n, min), max);
  }
  return field.value;
}

function onFieldChange(field) {
  writePath(config, field.dataset.path, collect(field));
  markDirty();
  if (field.dataset.path.startsWith("template.")) {
    matchPreset();
  }
  schedulePreview();
}

function markDirty() {
  dirty = true;
  els.save.disabled = false;
  setStatus("Unsaved changes", null);
}

function setStatus(text, level) {
  els.status.textContent = text;
  if (level) {
    els.status.dataset.level = level;
  } else {
    delete els.status.dataset.level;
  }
}

/* Preview ------------------------------------------------------------- */

let previewTimer = null;
function schedulePreview() {
  clearTimeout(previewTimer);
  previewTimer = setTimeout(refreshPreview, 120);
}

function refreshPreview() {
  invoke("preview", { template: config.template })
    .then((text) => {
      els.preview.textContent = text;
    })
    .catch((e) => {
      els.preview.textContent = "";
      note("error", "preview", String(e));
    });
}

/* Presets ------------------------------------------------------------- */

function fillPresets() {
  for (const preset of presets) {
    const option = document.createElement("option");
    option.value = preset.id;
    option.textContent = preset.label;
    option.title = preset.description;
    els.preset.appendChild(option);
  }
}

function sameTemplate(a, b) {
  return Object.keys(a).every((key) => a[key] === b[key]);
}

/* The picker is a starting point, not a mode: as soon as the fields diverge
 * from a preset it falls back to Custom rather than lying about which one is
 * in effect. */
function matchPreset() {
  const hit = presets.find((p) => sameTemplate(p.template, config.template));
  els.preset.value = hit ? hit.id : "";
}

els.preset.addEventListener("change", () => {
  const preset = presets.find((p) => p.id === els.preset.value);
  if (!preset) return;
  config.template = { ...preset.template };
  fillForm();
  markDirty();
  schedulePreview();
});

/* Shortcut recorder --------------------------------------------------- */

const MODIFIER_CODES = new Set([
  "ControlLeft", "ControlRight", "AltLeft", "AltRight",
  "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight",
]);

/* IS_MAC comes from shortcut.js, loaded first. */
const isMac = IS_MAC;

function keyToken(code) {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

function accelerator(event) {
  const parts = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push(isMac ? "Cmd" : "Super");
  parts.push(keyToken(event.code));
  return parts.join("+");
}

for (const field of document.querySelectorAll("input.shortcut")) {
  field.addEventListener("focus", () => {
    field.classList.add("recording");
    field.dataset.previous = field.value;
    field.value = "Press a combination";
  });

  field.addEventListener("blur", () => {
    field.classList.remove("recording");
    if (field.value === "Press a combination") {
      field.value = field.dataset.previous || "";
    }
  });

  field.addEventListener("keydown", (event) => {
    event.preventDefault();

    // Escape leaves the field as it was; Backspace clears the binding, which
    // is the only way to say "no shortcut for this".
    if (event.code === "Escape") {
      field.value = field.dataset.previous || "";
      field.blur();
      return;
    }
    if (event.code === "Backspace" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      field.value = "";
      writePath(config, field.dataset.path, "");
      markDirty();
      field.blur();
      return;
    }
    if (MODIFIER_CODES.has(event.code)) return;

    // A global shortcut with no modifier would swallow that key in every
    // application on the system.
    if (!event.ctrlKey && !event.altKey && !event.metaKey) {
      setStatus("A shortcut needs Ctrl, Alt or Cmd", "error");
      return;
    }

    const combination = accelerator(event);
    field.value = combination;
    field.dataset.previous = combination;
    writePath(config, field.dataset.path, combination);
    markDirty();
    field.blur();
  });
}

/* Notes --------------------------------------------------------------- */

function renderNotes(warnings, extra) {
  const rows = [];
  for (const item of extra) rows.push(item);
  for (const w of warnings) rows.push({ level: "warn", field: w.field, text: w.message });

  lastWarnings = warnings;
  els.notes.replaceChildren(
    ...rows.map((row) => {
      const div = document.createElement("div");
      div.className = "note";
      div.dataset.level = row.level;
      const field = document.createElement("span");
      field.className = "field";
      field.textContent = row.field || "";
      const text = document.createElement("span");
      text.textContent = row.text;
      div.append(field, text);
      return div;
    }),
  );
}

let transientNotes = [];
function note(level, field, text) {
  transientNotes = [{ level, field, text }];
  renderNotes(lastWarnings, transientNotes);
}

/* Save ---------------------------------------------------------------- */

function save() {
  els.save.disabled = true;
  invoke("save_config", { config })
    .then((warnings) => {
      dirty = false;
      setStatus("Saved", "ok");
      transientNotes = [];
      renderNotes(warnings, []);
    })
    .catch((e) => {
      setStatus("Not saved", "error");
      renderNotes([], [{ level: "error", field: "", text: String(e) }]);
    })
    .finally(() => {
      els.save.disabled = !dirty;
    });
}

els.save.addEventListener("click", save);

els.reset.addEventListener("click", () => {
  invoke("reset_config")
    .then(() => load())
    .then(() => {
      setStatus("Back to defaults", "ok");
    })
    .catch((e) => setStatus(String(e), "error"));
});

document.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "s") {
    event.preventDefault();
    save();
  }
  if (event.key === "Escape" && document.activeElement === document.body) {
    invoke("close_window", { label: "settings" }).catch(() => {});
  }
});

for (const field of fields) {
  if (field.classList.contains("shortcut")) continue;
  const event = field.type === "checkbox" || field.tagName === "SELECT" ? "change" : "input";
  field.addEventListener(event, () => onFieldChange(field));
}

/* Boot ---------------------------------------------------------------- */

function load() {
  return invoke("get_state").then((state) => {
    config = state.config;
    if (presets.length === 0) {
      presets = state.presets;
      fillPresets();
    }
    fillForm();

    const extra = [];
    if (state.platform.config_error) {
      extra.push({
        level: "error",
        field: "config file",
        text: `${state.platform.config_error} Defaults are in use until you save.`,
      });
    }
    els.filesNote.textContent = state.platform.file_capture
      ? "Selecting files in the file manager and copying them adds their paths."
      : `Not available on ${state.platform.os}, so a file copy is not seen.`;

    if (state.platform.hotkey_error) {
      extra.push({ level: "error", field: "shortcuts", text: state.platform.hotkey_error });
    }

    if (!state.platform.source_tracking) {
      els.sourceNote.textContent =
        `Not available on ${state.platform.os}, so {source} will be empty here.`;
    } else {
      els.sourceNote.textContent = "Fills in the {source} token.";
    }
    renderNotes(state.warnings, extra);
    refreshPreview();
    setStatus("", null);
    dirty = false;
    els.save.disabled = true;
  });
}

// A capture during a session changes what the preview should show.
listen("session", () => {
  if (config) refreshPreview();
});

load().catch((e) => setStatus(String(e), "error"));
