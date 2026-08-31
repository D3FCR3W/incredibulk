/* The front door.
 *
 * First launch runs the introduction, which can be skipped at any point.
 * Finishing or skipping it writes the config file, and that is what stops it
 * from appearing again. Afterwards this window is the launcher: one obvious
 * action, and the two places worth going. */

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const el = {
  tour: document.getElementById("tour"),
  home: document.getElementById("home"),
  dots: document.getElementById("dots"),
  steps: Array.from(document.querySelectorAll(".step")),
  back: document.getElementById("back"),
  next: document.getElementById("next"),
  skip: document.getElementById("skip"),
  keyStart: document.getElementById("key-start"),
  keyFlush: document.getElementById("key-flush"),
  optAutostart: document.getElementById("opt-autostart"),
  optHistory: document.getElementById("opt-history"),
  optRich: document.getElementById("opt-rich"),
  status: document.getElementById("status"),
  start: document.getElementById("start"),
  startTitle: document.getElementById("start-title"),
  startSub: document.getElementById("start-sub"),
  history: document.getElementById("history"),
  historySub: document.getElementById("history-sub"),
  settings: document.getElementById("settings"),
  cheatsheet: document.getElementById("cheatsheet"),
  replay: document.getElementById("replay"),
  update: document.getElementById("update"),
  updateTitle: document.getElementById("update-title"),
  updateNote: document.getElementById("update-note"),
  updateInstall: document.getElementById("update-install"),
};

let config = null;
let step = 0;

/* formatShortcut comes from shortcut.js, loaded first. */
const pretty = (accelerator) => formatShortcut(accelerator, "unbound");

/* Tour ------------------------------------------------------------------ */

function showStep(index) {
  step = Math.max(0, Math.min(el.steps.length - 1, index));
  el.steps.forEach((s, i) => s.classList.toggle("on", i === step));

  el.dots.replaceChildren(
    ...el.steps.map((_, i) => {
      const dot = document.createElement("i");
      if (i === step) dot.className = "on";
      return dot;
    }),
  );

  el.back.disabled = step === 0;
  el.next.textContent = step === el.steps.length - 1 ? "Start using Incredibulk" : "Next";
  el.steps[step].scrollIntoView({ block: "nearest" });
}

el.back.addEventListener("click", () => showStep(step - 1));

el.next.addEventListener("click", () => {
  if (step < el.steps.length - 1) {
    showStep(step + 1);
  } else {
    finish();
  }
});

// Skipping still records the choices already made: the options step is a
// setting, not a formality, and losing it would be surprising.
el.skip.addEventListener("click", finish);

function collectOptions() {
  if (!config) return;
  config.behavior.autostart = el.optAutostart.checked;
  config.behavior.keep_history = el.optHistory.checked;
  config.behavior.paste_format = el.optRich.checked ? "rich" : "plain";
  config.onboarded = true;
}

function finish() {
  collectOptions();
  invoke("save_config", { config })
    .catch(() => invoke("finish_onboarding"))
    .finally(() => {
      el.tour.hidden = true;
      el.home.hidden = false;
      refresh();
    });
}

document.addEventListener("keydown", (e) => {
  if (!el.tour.hidden) {
    if (e.key === "ArrowRight" || e.key === "Enter") el.next.click();
    if (e.key === "ArrowLeft") el.back.click();
    return;
  }
  if (e.key === "Escape") {
    invoke("close_window", { label: "home" }).catch(() => {});
  }
});

/* Home ------------------------------------------------------------------ */

el.start.addEventListener("click", () => {
  const active = el.start.dataset.action === "stack";
  const command = active ? "open_stack" : "start_session";
  invoke(command)
    .catch(() => {})
    .finally(() => {
      // Get out of the way: the next thing the user does is copy something.
      invoke("close_window", { label: "home" }).catch(() => {});
    });
});

el.history.addEventListener("click", () => {
  invoke("open_history").catch(() => {});
});

el.settings.addEventListener("click", () => {
  invoke("open_settings").catch(() => {});
});

el.replay.addEventListener("click", () => {
  el.home.hidden = true;
  el.tour.hidden = false;
  showStep(0);
});

function cheatsheet(hotkeys) {
  const rows = [
    ["Open a session", hotkeys.toggle_session],
    ["Paste everything", hotkeys.flush],
    ["Show the stack", hotkeys.toggle_hud],
    ["History", hotkeys.history],
  ];
  el.cheatsheet.replaceChildren(
    ...rows.flatMap(([label, accel]) => {
      const dt = document.createElement("dt");
      const key = document.createElement("kbd");
      key.textContent = pretty(accel);
      dt.appendChild(key);
      const dd = document.createElement("dd");
      dd.textContent = label;
      return [dt, dd];
    }),
  );
}

function paintSession(snapshot) {
  const active = snapshot && snapshot.active;
  if (active) {
    const named = snapshot.name ? `"${snapshot.name}"` : "Session open";
    el.status.textContent =
      snapshot.count === 1
        ? `${named}, 1 fragment so far.`
        : `${named}, ${snapshot.count} fragments so far.`;
    el.startTitle.textContent = "Open the stack";
    el.startSub.textContent = "See and edit what you have collected";
    el.start.dataset.action = "stack";
  } else {
    el.status.textContent = "No session open.";
    el.startTitle.textContent = "Start a session";
    el.startSub.textContent = "Then copy as much as you need";
    el.start.dataset.action = "start";
  }
}

function paintHistory(entries) {
  const count = entries.length;
  el.historySub.textContent =
    count === 0
      ? "Nothing kept yet"
      : count === 1
        ? "1 session kept"
        : `${count} sessions kept`;
}

/* Updates ---------------------------------------------------------------

 * Offered, never imposed. The banner appears only when there is something to
 * install, and installing is a button because a restart in the middle of a
 * session would throw away what was collected. */
function showUpdate(status) {
  if (!status || !status.available) {
    el.update.hidden = true;
    return;
  }
  el.updateTitle.textContent = `Version ${status.version} is available`;
  el.updateNote.textContent = firstLine(status.notes) || "";
  el.update.hidden = false;
}

/* Release notes can run to paragraphs. The banner has room for a sentence. */
function firstLine(notes) {
  if (!notes) return "";
  const line = notes.split("\n").map((l) => l.trim()).find((l) => l.length > 0);
  return line && line.length > 90 ? `${line.slice(0, 89)}\u2026` : line || "";
}

el.updateInstall.addEventListener("click", () => {
  el.updateInstall.disabled = true;
  el.updateInstall.textContent = "Downloading\u2026";
  invoke("install_update").catch((e) => {
    el.updateInstall.disabled = false;
    el.updateInstall.textContent = "Install and restart";
    el.updateTitle.textContent = String(e);
  });
});

listen("update", (event) => showUpdate(event.payload));

function refresh() {
  return invoke("get_state").then((state) => {
    config = state.config;
    cheatsheet(config.hotkeys);
    paintSession(state.snapshot);
    return invoke("get_history").then(paintHistory);
  });
}

listen("session", (event) => paintSession(event.payload));
listen("history", (event) => paintHistory(event.payload));

/* The tour shows two keys before the backend has answered. Filling them from
 * the known defaults straight away avoids a Mac reading "Ctrl" for the moment
 * it takes to ask. */
el.keyStart.textContent = pretty("CmdOrCtrl+Alt+C");
el.keyFlush.textContent = pretty("CmdOrCtrl+Alt+V");

invoke("get_state")
  .then((state) => {
    config = state.config;
    el.keyStart.textContent = pretty(config.hotkeys.toggle_session);
    el.keyFlush.textContent = pretty(config.hotkeys.flush);
    el.optAutostart.checked = config.behavior.autostart;
    el.optHistory.checked = config.behavior.keep_history;
    el.optRich.checked = config.behavior.paste_format === "rich";
    cheatsheet(config.hotkeys);
    paintSession(state.snapshot);

    if (config.onboarded) {
      el.home.hidden = false;
      invoke("get_history").then(paintHistory).catch(() => {});
      // The startup check may already have run and emitted before this window
      // was opened, so ask rather than wait for an event that has been and
      // gone.
      if (config.behavior.check_for_updates) {
        invoke("check_for_update").then(showUpdate).catch(() => {});
      }
    } else {
      el.tour.hidden = false;
      showStep(0);
    }
  })
  .catch(() => {
    // Even with the backend unreachable the window should not be blank.
    el.home.hidden = false;
  });
