/* The stack window: the list you work in while a session is open.
 *
 * It is an editor, not a readout. Every line can be unticked without losing
 * it, edited in place, reordered, or typed from scratch, because the decision
 * about what belongs in the paste is one you make after collecting, not
 * during.
 *
 * Everything shown here comes off somebody's clipboard, so every string from
 * the backend is written with textContent and never as markup. A fragment
 * copied from a web page will contain angle brackets sooner or later. */

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const el = {
  body: document.body,
  count: document.getElementById("count"),
  sessionName: document.getElementById("session-name"),
  stack: document.getElementById("stack"),
  empty: document.getElementById("empty"),
  emptyTitle: document.getElementById("empty-title"),
  emptyBody: document.getElementById("empty-body"),
  emptyAction: document.getElementById("empty-action"),
  toggleAll: document.getElementById("toggle-all"),
  compose: document.getElementById("compose"),
  addLine: document.getElementById("add-line"),
  flush: document.getElementById("flush"),
  clear: document.getElementById("clear"),
  discard: document.getElementById("discard"),
  toast: document.getElementById("toast"),
  settings: document.getElementById("open-settings"),
  history: document.getElementById("open-history"),
  hide: document.getElementById("hide"),
};

const ICONS = {
  remove: "M18 6 6 18M6 6l12 12",
};

/* Six dots, the grip everyone already recognises as "drag me". */
const GRIP_DOTS = [
  [9, 6], [15, 6],
  [9, 12], [15, 12],
  [9, 18], [15, 18],
];

let hotkeys = null;
let snapshot = { active: false, count: 0, included: 0, items: [] };

/* An edit in progress. The list re-renders on every capture, and a fragment
 * arriving mid-sentence must not wipe out what is being typed, so the render
 * is deferred until the edit is finished. A drag holds the render off for the
 * same reason: the row being moved must not vanish mid-gesture. */
let editing = null;
let dragging = null;
let deferred = null;

function icon(path) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const p = document.createElementNS("http://www.w3.org/2000/svg", "path");
  p.setAttribute("d", path);
  svg.appendChild(p);
  return svg;
}

function toolButton(label, path, onClick) {
  const b = document.createElement("button");
  b.className = "icon";
  b.title = label;
  b.setAttribute("aria-label", label);
  b.appendChild(icon(path));
  // Taking focus here would blur an open editor, commit it, and rebuild the
  // list out from under the click that is still in progress. Refusing the
  // focus keeps both the editor and the click intact.
  b.addEventListener("mousedown", (e) => e.preventDefault());
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onClick();
  });
  return b;
}

/* Sizes are shown per kind because "1244" means characters for text and
 * pixels for an image, and one unit label for both would be a lie. */
function describe(item) {
  if (item.kind === "image") return "image";
  if (item.kind === "files") {
    return item.weight === 1 ? "1 file" : `${item.weight} files`;
  }
  return item.weight === 1 ? "1 char" : `${item.weight} chars`;
}

function grip(item, index) {
  const b = document.createElement("button");
  b.className = "grip";
  b.title = "Drag to reorder, or focus and use the arrow keys";
  b.setAttribute("aria-label", `Reorder ${item.preview}`);

  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  for (const [cx, cy] of GRIP_DOTS) {
    const dot = document.createElementNS("http://www.w3.org/2000/svg", "circle");
    dot.setAttribute("cx", String(cx));
    dot.setAttribute("cy", String(cy));
    dot.setAttribute("r", "1.6");
    svg.appendChild(dot);
  }
  b.appendChild(svg);

  b.addEventListener("pointerdown", (e) => startDrag(e, b, item, index));
  b.addEventListener("keydown", (e) => {
    // The same reordering without a mouse, for anyone who cannot drag.
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    move(item.id, e.key === "ArrowUp" ? "up" : "down");
  });
  return b;
}

function row(item, index, total, previous) {
  const li = document.createElement("li");
  li.className = "item";
  li.dataset.kind = item.kind;
  li.dataset.id = String(item.id);
  if (!item.included) li.classList.add("excluded");

  const tick = document.createElement("input");
  tick.type = "checkbox";
  tick.className = "tick";
  tick.checked = item.included;
  tick.title = item.included ? "Leave out of the paste" : "Put back in the paste";
  tick.setAttribute("aria-label", tick.title);
  tick.addEventListener("change", () => {
    invoke("set_included", { id: item.id, included: tick.checked }).catch(reportFailure);
  });

  const ordinal = document.createElement("span");
  ordinal.className = "ordinal";
  // The number is the position in the block, so an unticked line has none.
  ordinal.textContent = item.position === null ? "–" : String(item.position);

  const content = document.createElement("div");
  content.className = "content";

  if (editing && editing.id === item.id) {
    content.appendChild(editorFor(item));
  } else {
    if (item.thumbnail) {
      // An image is worth showing. A line of text saying "image 1920x1080"
      // tells you almost nothing about which screenshot it was.
      const thumb = document.createElement("img");
      thumb.className = "thumb";
      thumb.src = item.thumbnail;
      thumb.alt = item.preview;
      content.appendChild(thumb);
    }
    const preview = document.createElement("div");
    preview.className = "preview";
    preview.textContent = item.preview;
    if (item.editable) {
      preview.title = "Click to edit";
      preview.addEventListener("click", () => startEdit(item));
    }
    content.appendChild(preview);
    content.appendChild(meta(item, previous));
  }

  const tools = document.createElement("div");
  tools.className = "tools";
  tools.append(toolButton("Remove", ICONS.remove, () => remove(item.id)));

  li.append(grip(item, index), tick, ordinal, content, tools);
  return li;
}

function meta(item, previous) {
  const wrap = document.createElement("div");
  wrap.className = "meta";

  const size = document.createElement("span");
  size.textContent = describe(item);
  wrap.appendChild(size);

  if (item.edited) {
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.textContent = "edited";
    wrap.appendChild(chip);
  }

  // Repeating the same window title on every row eats the width without
  // saying anything, so it appears only when the source changes.
  if (item.source && item.source !== previous) {
    const dot = document.createElement("span");
    dot.textContent = "·";
    const source = document.createElement("span");
    source.className = "source";
    source.textContent = item.source;
    wrap.append(dot, source);
  }
  return wrap;
}

/* Editing ------------------------------------------------------------- */

function editorFor(item) {
  const area = document.createElement("textarea");
  area.className = "editor";
  area.value = editing.text;
  area.spellcheck = false;
  area.rows = Math.min(10, Math.max(1, editing.text.split("\n").length));

  area.addEventListener("input", () => {
    editing.text = area.value;
    autosize(area);
  });

  area.addEventListener("keydown", (e) => {
    // Enter commits, because a fragment is usually one line and reaching for
    // the mouse to confirm every edit would defeat the point. Shift+Enter is
    // there for the fragments that are not.
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      commitEdit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelEdit();
    }
  });

  area.addEventListener("blur", commitEdit);

  // Focus after the row is in the document, otherwise there is nothing to
  // put the caret into.
  queueMicrotask(() => {
    area.focus();
    area.setSelectionRange(area.value.length, area.value.length);
    autosize(area);
  });
  return area;
}

function autosize(area) {
  area.style.height = "auto";
  area.style.height = `${Math.min(area.scrollHeight, 220)}px`;
}

function startEdit(item) {
  if (editing && editing.id === item.id) return;
  invoke("item_text", { id: item.id })
    .then((text) => {
      if (text === null || text === undefined) return;
      editing = { id: item.id, text, original: text };
      draw();
    })
    .catch(reportFailure);
}

function commitEdit() {
  if (!editing) return;
  const { id, text, original } = editing;
  editing = null;
  if (text === original) {
    finishEdit();
    return;
  }
  invoke("edit_item", { id, text })
    .catch(reportFailure)
    .finally(finishEdit);
}

function cancelEdit() {
  editing = null;
  finishEdit();
}

/* Any captures that arrived during the edit are drawn now. */
function finishEdit() {
  if (deferred) {
    snapshot = deferred;
    deferred = null;
  }
  draw();
}

/* Dragging ------------------------------------------------------------ */

/* Pointer events rather than HTML drag and drop: this needs a drop line drawn
 * between rows and a list that scrolls while the pointer is held near an edge,
 * neither of which the native drag API gives you in a webview. */
function startDrag(event, handle, item, index) {
  if (event.button !== 0) return;
  event.preventDefault();
  if (editing) commitEdit();

  handle.setPointerCapture(event.pointerId);
  dragging = { id: item.id, from: index, target: index, pointerId: event.pointerId };
  el.stack.classList.add("reordering");
  rowAt(index)?.classList.add("drag-source");
  markDrop(index);

  const onMove = (e) => {
    if (!dragging) return;
    autoScroll(e.clientY);
    const target = insertionIndexAt(e.clientY);
    if (target !== dragging.target) {
      dragging.target = target;
      markDrop(target);
    }
  };

  const onUp = () => {
    handle.removeEventListener("pointermove", onMove);
    handle.removeEventListener("pointerup", onUp);
    handle.removeEventListener("pointercancel", onCancel);
    finishDrag();
  };

  const onCancel = () => {
    handle.removeEventListener("pointermove", onMove);
    handle.removeEventListener("pointerup", onUp);
    handle.removeEventListener("pointercancel", onCancel);
    dragging = null;
    clearDrop();
    finishEdit();
  };

  handle.addEventListener("pointermove", onMove);
  handle.addEventListener("pointerup", onUp);
  handle.addEventListener("pointercancel", onCancel);
}

function rowAt(index) {
  return el.stack.children[index] || null;
}

/* Where the row would land, as a slot between rows: 0 is above the first,
 * children.length is below the last. */
function insertionIndexAt(y) {
  const rows = Array.from(el.stack.children);
  for (let i = 0; i < rows.length; i++) {
    const box = rows[i].getBoundingClientRect();
    if (y < box.top + box.height / 2) return i;
  }
  return rows.length;
}

function markDrop(slot) {
  clearDrop();
  const rows = Array.from(el.stack.children);
  if (slot >= rows.length) {
    rows[rows.length - 1]?.classList.add("drop-after");
  } else {
    rows[slot].classList.add("drop-before");
  }
}

function clearDrop() {
  for (const r of el.stack.children) {
    r.classList.remove("drop-before", "drop-after");
  }
}

/* Keep the list moving when the pointer is held against an edge, so a row can
 * be dragged further than one screenful. */
function autoScroll(y) {
  const scroller = el.stack.parentElement;
  const view = scroller.getBoundingClientRect();
  const margin = 28;
  if (y < view.top + margin) {
    scroller.scrollTop -= 12;
  } else if (y > view.bottom - margin) {
    scroller.scrollTop += 12;
  }
}

function finishDrag() {
  if (!dragging) return;
  const { id, from, target } = dragging;
  dragging = null;
  el.stack.classList.remove("reordering");
  clearDrop();

  // The slot is counted with the row still in place, so dropping below its own
  // position lands one slot too low once it is lifted out.
  const to = target > from ? target - 1 : target;
  if (to === from) {
    finishEdit();
    return;
  }
  invoke("reorder_item", { id, index: to }).catch(reportFailure).finally(finishEdit);
}

/* Rendering ----------------------------------------------------------- */

function kbd(text) {
  const k = document.createElement("kbd");
  k.textContent = text;
  return k;
}

function renderEmpty(active) {
  el.stack.hidden = true;
  el.empty.hidden = false;

  if (active) {
    el.emptyTitle.textContent = "Session open";
    el.emptyBody.replaceChildren(
      document.createTextNode("Copy anything and it lands here. Paste it all with "),
      kbd(formatShortcut(hotkeys && hotkeys.flush)),
      document.createTextNode("."),
    );
    el.emptyAction.textContent = "Discard session";
    el.emptyAction.dataset.action = "cancel";
  } else {
    el.emptyTitle.textContent = "No session open";
    el.emptyBody.replaceChildren(
      document.createTextNode("Press "),
      kbd(formatShortcut(hotkeys && hotkeys.toggle_session)),
      document.createTextNode(" to start collecting."),
    );
    el.emptyAction.textContent = "Start a session";
    el.emptyAction.dataset.action = "start";
  }
}

/* Naming ---------------------------------------------------------------- */

/* A name is the only thing that tells two sessions apart an hour later, so it
 * is editable in place rather than hidden behind a dialog. */
function startRename() {
  if (!snapshot.active || el.sessionName.dataset.editing) return;

  const input = document.createElement("input");
  input.type = "text";
  input.className = "name-input";
  input.value = snapshot.name || "";
  input.placeholder = "Name this session";
  input.maxLength = 80;

  const commit = (save) => {
    if (!el.sessionName.dataset.editing) return;
    delete el.sessionName.dataset.editing;
    const value = input.value;
    input.replaceWith(el.sessionName);
    if (save) {
      invoke("rename_session", { name: value }).catch(reportFailure);
    } else {
      draw();
    }
  };

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commit(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      commit(false);
    }
  });
  input.addEventListener("blur", () => commit(true));

  el.sessionName.dataset.editing = "1";
  el.sessionName.replaceWith(input);
  input.focus();
  input.select();
}

el.sessionName.addEventListener("click", startRename);

function draw() {
  const { active, count, included, items } = snapshot;

  // A renamed session says its name; an unnamed one says what it is.
  if (!el.sessionName.dataset.editing) {
    el.sessionName.textContent = snapshot.name || "Session";
    el.sessionName.classList.toggle("named", Boolean(snapshot.name));
  }

  el.body.classList.toggle("live", active);
  // Only worth spelling out the difference when there is one.
  el.count.textContent = included === count ? String(count) : `${included} of ${count}`;

  el.flush.disabled = included === 0;
  el.clear.disabled = count === 0;
  el.discard.disabled = !active;
  el.flush.textContent = included > 0 ? `Paste all (${included})` : "Paste all";

  el.compose.hidden = !active;
  el.toggleAll.hidden = count === 0;
  el.toggleAll.textContent = included === count ? "Untick all" : "Tick all";

  if (items.length === 0) {
    renderEmpty(active);
    return;
  }

  el.empty.hidden = true;
  el.stack.hidden = false;
  el.stack.replaceChildren(
    ...items.map((item, i) =>
      row(item, i, items.length, i > 0 ? items[i - 1].source : null),
    ),
  );
}

function apply(next) {
  // Redrawing under an open editor would take the caret with it, and under a
  // drag it would delete the row being held. Renaming is the same story.
  if (editing || dragging || el.sessionName.dataset.editing) {
    deferred = next;
    return;
  }
  snapshot = next;
  draw();
}

/* Toasts -------------------------------------------------------------- */

let toastTimer = null;
function toast(level, text) {
  el.toast.textContent = text;
  el.toast.dataset.level = level;
  el.toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(
    () => {
      el.toast.hidden = true;
    },
    level === "error" ? 6000 : 2200,
  );
}

function reportFailure(e) {
  toast("error", String(e));
}

function move(id, direction) {
  invoke("move_item", { id, direction }).catch(reportFailure);
}

function remove(id) {
  invoke("remove_item", { id }).catch(reportFailure);
}

/* Wiring -------------------------------------------------------------- */

el.flush.addEventListener("click", () => {
  invoke("flush_session").catch(reportFailure);
});
el.clear.addEventListener("click", () => {
  invoke("clear_items").catch(reportFailure);
});
el.discard.addEventListener("click", () => {
  invoke("cancel_session").catch(reportFailure);
});
el.toggleAll.addEventListener("click", () => {
  const included = snapshot.included !== snapshot.count;
  invoke("set_all_included", { included }).catch(reportFailure);
});
el.emptyAction.addEventListener("click", () => {
  const action = el.emptyAction.dataset.action === "start" ? "start_session" : "cancel_session";
  invoke(action).catch(reportFailure);
});
el.settings.addEventListener("click", () => {
  invoke("open_settings").catch(reportFailure);
});
el.history.addEventListener("click", () => {
  invoke("open_history").catch(reportFailure);
});
el.hide.addEventListener("click", () => {
  invoke("hide_hud").catch(reportFailure);
});

el.addLine.addEventListener("keydown", (e) => {
  if (e.key !== "Enter") return;
  const text = el.addLine.value;
  if (!text.trim()) return;
  el.addLine.value = "";
  invoke("add_text", { text }).catch((err) => {
    // Hand the text back rather than swallowing what was typed.
    el.addLine.value = text;
    reportFailure(err);
  });
});

document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  // Escape belongs to the editor and the compose box while they are in use.
  if (editing || document.activeElement === el.addLine) return;
  invoke("hide_hud").catch(reportFailure);
});

listen("session", (event) => apply(event.payload));
listen("notice", (event) => toast(event.payload.level, event.payload.text));

/* The window can be shown long after the last event was emitted, so the first
 * paint pulls the state rather than waiting for something to change. */
invoke("get_state")
  .then((state) => {
    hotkeys = state.config.hotkeys;
    apply(state.snapshot);
  })
  .catch(reportFailure);
