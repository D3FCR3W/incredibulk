/* Past sessions.
 *
 * One tick replays a session. Several ticks paste them as one block, in the
 * order they happened rather than the order they were ticked, because that is
 * how the content reads.
 *
 * As everywhere else, session content is written with textContent: it came off
 * a clipboard and is not markup. */

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const el = {
  list: document.getElementById("list"),
  preview: document.getElementById("preview"),
  note: document.getElementById("note"),
  subtitle: document.getElementById("subtitle"),
  paste: document.getElementById("paste"),
  copy: document.getElementById("copy"),
  forget: document.getElementById("forget"),
  clear: document.getElementById("clear"),
  toast: document.getElementById("toast"),
};

let entries = [];
const selected = new Set();
let flushKey = "Ctrl+Alt+V";

/* formatShortcut comes from shortcut.js, loaded first. */
const pretty = (accelerator) => formatShortcut(accelerator, "the paste shortcut");

function when(ms) {
  const then = new Date(ms);
  const now = new Date();
  const sameDay = then.toDateString() === now.toDateString();
  const time = then.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (sameDay) return `Today ${time}`;

  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (then.toDateString() === yesterday.toDateString()) return `Yesterday ${time}`;

  return `${then.toLocaleDateString([], { day: "numeric", month: "short" })} ${time}`;
}

function plural(n, one, many) {
  return n === 1 ? `1 ${one}` : `${n} ${many}`;
}

function entryRow(entry) {
  const row = document.createElement("div");
  row.className = "entry";
  if (selected.has(entry.id)) row.classList.add("selected");

  const tick = document.createElement("input");
  tick.type = "checkbox";
  tick.checked = selected.has(entry.id);
  tick.setAttribute("aria-label", `Select the session from ${when(entry.ended_at_ms)}`);
  tick.addEventListener("click", (e) => e.stopPropagation());
  tick.addEventListener("change", () => toggle(entry.id, tick.checked));

  if (entry.thumbnail) {
    const thumb = document.createElement("img");
    thumb.className = "thumb";
    thumb.src = entry.thumbnail;
    thumb.alt = "";
    row.append(tick, thumb);
  } else {
    row.append(tick, document.createElement("span"));
  }

  const body = document.createElement("div");
  body.className = "body";

  const head = document.createElement("div");
  head.className = "when";
  const stamp = document.createElement("strong");
  // The name is what the session is; the time is only when it happened.
  stamp.textContent = entry.name || when(entry.ended_at_ms);
  head.appendChild(stamp);

  if (entry.name) {
    const at = document.createElement("span");
    at.textContent = when(entry.ended_at_ms);
    head.appendChild(at);
  }

  const count = document.createElement("span");
  count.textContent = plural(entry.count, "fragment", "fragments");
  head.appendChild(count);

  // Kinds are called out because a session of screenshots has almost no text
  // to preview and would otherwise look empty.
  if (entry.images > 0) head.appendChild(badge(plural(entry.images, "image", "images")));
  if (entry.files > 0) head.appendChild(badge(plural(entry.files, "file", "files")));

  const summary = document.createElement("div");
  summary.className = "summary";
  summary.textContent = entry.preview;

  body.append(head, summary);

  const rename = document.createElement("button");
  rename.className = "rename";
  rename.textContent = entry.name ? "Rename" : "Name it";
  rename.title = "Give this session a name";
  rename.addEventListener("click", (e) => {
    e.stopPropagation();
    beginRename(entry, head, stamp);
  });

  row.append(body, rename);

  // Clicking the row is the same as ticking it: the checkbox is a small target
  // and selecting is the only thing this list is for.
  row.addEventListener("click", () => toggle(entry.id, !selected.has(entry.id)));
  return row;
}

/* Renaming happens in the row: opening a dialog to type six characters is
 * more ceremony than the act deserves. */
function beginRename(entry, head, stamp) {
  if (head.querySelector("input")) return;

  const input = document.createElement("input");
  input.type = "text";
  input.className = "rename-input";
  input.value = entry.name || "";
  input.placeholder = when(entry.ended_at_ms);
  input.maxLength = 80;
  input.addEventListener("click", (e) => e.stopPropagation());

  const commit = (save) => {
    if (!input.isConnected) return;
    const value = input.value;
    input.replaceWith(stamp);
    if (save) {
      invoke("rename_history_entry", { id: entry.id, name: value }).catch(reportFailure);
    }
  };

  input.addEventListener("keydown", (e) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      commit(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      commit(false);
    }
  });
  input.addEventListener("blur", () => commit(true));

  stamp.replaceWith(input);
  input.focus();
  input.select();
}

function badge(text) {
  const b = document.createElement("span");
  b.className = "badge";
  b.textContent = text;
  return b;
}

function toggle(id, on) {
  if (on) {
    selected.add(id);
  } else {
    selected.delete(id);
  }
  draw();
  refreshPreview();
  publishSelection();
}

/* The paste shortcut works with this window closed, so the backend has to know
 * what is ticked rather than asking for it. */
function publishSelection() {
  invoke("set_history_selection", { ids: [...selected] }).catch(() => {});
}

function draw() {
  el.subtitle.textContent =
    entries.length === 0
      ? "Nothing kept yet."
      : `${plural(entries.length, "session", "sessions")}, newest first.`;

  const count = selected.size;
  el.paste.disabled = count === 0;
  el.copy.disabled = count === 0;
  el.forget.disabled = count === 0;
  el.clear.disabled = entries.length === 0;
  el.paste.textContent = count > 1 ? `Paste ${count} together` : "Paste selected";

  el.note.textContent =
    count > 1
      ? `Combined oldest first. ${pretty(flushKey)} pastes these ${count}.`
      : count === 1
        ? `${pretty(flushKey)} pastes this one.`
        : `Nothing ticked, so ${pretty(flushKey)} pastes the most recent session.`;

  if (entries.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    const title = document.createElement("strong");
    title.textContent = "No sessions yet";
    const body = document.createElement("p");
    body.textContent = "A session is kept here once you paste it.";
    empty.append(title, body);
    el.list.replaceChildren(empty);
    return;
  }

  el.list.replaceChildren(...entries.map(entryRow));
}

function refreshPreview() {
  const ids = [...selected];
  if (ids.length === 0) {
    el.preview.textContent = "";
    return;
  }
  invoke("preview_history", { ids })
    .then((text) => {
      el.preview.textContent = text;
    })
    .catch(reportFailure);
}

let toastTimer = null;
function toast(level, text) {
  el.toast.textContent = text;
  el.toast.dataset.level = level;
  el.toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    el.toast.hidden = true;
  }, level === "error" ? 6000 : 2400);
}

function reportFailure(e) {
  toast("error", String(e));
}

el.paste.addEventListener("click", () => {
  const ids = [...selected];
  invoke("replay_history", { ids })
    .then((count) => {
      // The window is in the way of whatever the paste is going into, so it
      // gets out of the way as soon as the block is on its way.
      invoke("close_window", { label: "history" }).catch(() => {});
      toast("info", `Pasted ${plural(count, "fragment", "fragments")}`);
    })
    .catch(reportFailure);
});

el.copy.addEventListener("click", () => {
  const ids = [...selected];
  invoke("preview_history", { ids })
    .then((text) => navigator.clipboard.writeText(text))
    .then(() => toast("info", "Copied to the clipboard"))
    .catch(reportFailure);
});

el.forget.addEventListener("click", () => {
  const ids = [...selected];
  selected.clear();
  invoke("forget_history", { ids }).catch(reportFailure);
});

el.clear.addEventListener("click", () => {
  selected.clear();
  invoke("clear_history").catch(reportFailure);
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    invoke("close_window", { label: "history" }).catch(() => {});
  }
  if ((e.ctrlKey || e.metaKey) && e.key === "a") {
    e.preventDefault();
    for (const entry of entries) selected.add(entry.id);
    draw();
    refreshPreview();
    publishSelection();
  }
});

function load(next) {
  entries = next;
  // A session that has been deleted elsewhere must not stay selected.
  const alive = new Set(entries.map((e) => e.id));
  for (const id of [...selected]) {
    if (!alive.has(id)) selected.delete(id);
  }
  draw();
  refreshPreview();
  publishSelection();
}

listen("history", (event) => load(event.payload));

invoke("get_state")
  .then((state) => {
    flushKey = state.config.hotkeys.flush;
    draw();
  })
  .catch(() => {});

invoke("get_history").then(load).catch(reportFailure);
