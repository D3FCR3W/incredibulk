<div align="center">

# Incredibulk

**Copy ten things in a row. Paste them all at once.**

[![ci](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml/badge.svg)](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![release](https://img.shields.io/github/v/release/D3FCR3W/incredibulk?include_prereleases&sort=semver)](../../releases)

</div>

Your clipboard holds one thing at a time, so collecting six fragments means six
trips back and forth. Incredibulk adds a **session**: open it, copy as much as
you like from anywhere, then paste everything as one block.

## How it works

1. **`Ctrl+Alt+C`** opens a session.
2. **Copy as usual** with `Ctrl+C`, from any application. Each copy is added to
   the stack instead of replacing the last one.
3. **Tidy the stack** in the small window that appears: untick a line to leave
   it out, click one to edit it, drag to reorder, or type a line by hand.
4. **`Ctrl+Alt+V`** pastes the whole stack as one block into the window you
   are in.

It is not a clipboard history you pick items out of one by one. A session has a
beginning and an end, and the end is one paste.

## Install

### Windows

**[Download the installer](../../releases/latest/download/Incredibulk-win-x64-setup.exe)**
and run it. The link always points at the newest release.

The first run shows **"Windows protected your PC"** because the build is not
code signed. Click *More info*, then *Run anyway*.

Prefer a single file? The zip on the [releases page](../../releases) contains
one portable executable. It cannot update itself, so you download it again
when a new version comes out.

### macOS and Linux

No prebuilt binaries yet. [Build from source](#building) and check
[platform support](#platform-support) for what works where.

## Shortcuts

| What | Default |
| --- | --- |
| Open a session, or discard the open one | `Ctrl+Alt+C` |
| Paste the stack, or repeat the last session | `Ctrl+Alt+V` |
| Discard without pasting | `Ctrl+Alt+X` |
| Undo the last capture | `Ctrl+Alt+Z` |
| Show or hide the stack window | `Ctrl+Alt+S` |
| Open the history | `Ctrl+Alt+H` |

On macOS read `Cmd+Alt` instead of `Ctrl+Alt`. All six can be changed in
Settings. `Ctrl+Alt` was chosen over `Ctrl+Shift` because `Ctrl+Shift+C` and
`Ctrl+Shift+V` are already copy and paste in most terminals.

## Using it

**The tray icon is the front door.** Incredibulk has no window of its own until
you ask for one. Click the icon to open the home window (start a session,
history, settings), or the stack when a session is running. The first launch
shows a short introduction; it can be skipped and reopened later from Settings.

**The stack window** appears when a session opens and never steals focus.
Tick boxes leave a line out without deleting it, clicking a line edits it in
place, the grip handle drags it elsewhere, the field at the bottom adds a line
that was never copied, and clicking the title names the session. Images show
as thumbnails. The paste button always says how many lines are going in.

**History** keeps every session you pasted, 50 by default, across restarts.
Open it from the tray, from the clock icon in the stack window, or with
`Ctrl+Alt+H`. Tick one session to paste it again, or several to paste them as
one block in the order they happened. **Copy without pasting** puts the block
on the clipboard instead. `Ctrl+F` searches by name, text, file path or source
window.

**`Ctrl+Alt+V` with no session open** still pastes something: the sessions
ticked in the history, or the most recent one when nothing is ticked.

**Output format** is a template, not a fixed join. Presets cover the usual
shapes (one per line, blank line between, numbered, markdown bullets, with
source and time, comma separated, fenced block) and every field can be edited,
with a live preview of your actual stack.

| Scope | Tokens |
| --- | --- |
| Any line | `{content}` `{index}` `{index0}` `{count}` `{source}` `{kind}` `{time}` `{date}` |
| Image placeholder | `{width}` `{height}` |
| File path | `{path}` `{name}` `{stem}` `{ext}` `{dir}` |

**Images** cannot go into a text block, so the paste goes on the clipboard
twice: as rich text with the image embedded, and as plain text with a
placeholder such as `[image 1920x1080]`. Editors and terminals get the plain
text, word processors get the picture. Set **Plain text only** in Settings to
never send formatting at all. History keeps images up to 4 MB each.

## Platform support

| | Windows | macOS | Linux (X11) | Linux (Wayland) |
| --- | --- | --- | --- | --- |
| Shortcuts, capture, tray, autostart | Yes | Yes | Yes | Compositor dependent |
| Automatic paste | Yes | Needs Accessibility | Yes | Often unavailable |
| Copied images | Yes | Yes | Yes | Yes |
| Copied file lists | Yes | No | No | No |
| `{source}` window title | Yes | No | No | No |
| Rich paste (HTML) | Tested | Untested | Untested | Untested |

Windows is where it was built and tested. macOS asks for Accessibility
permission before the paste keystroke works. Wayland restricts input
synthesis, so **Paste automatically** may need to be switched off, leaving the
block on the clipboard for you to paste yourself.

## Where it keeps its files

`config.json`, `history.json` and, after a crash, `crash.log`:

| OS | Folder |
| --- | --- |
| Windows | `%APPDATA%\dev.incredibulk.app` |
| macOS | `~/Library/Application Support/dev.incredibulk.app` |
| Linux | `$XDG_CONFIG_HOME/dev.incredibulk.app` |

A config file from an older or newer build still loads: missing fields fall
back to defaults, unknown ones are ignored. A file that cannot be parsed does
not stop the app; it starts on defaults and shows the reason in Settings.

## Updates

Incredibulk checks for a new version a few seconds after launch and shows an
**Install and restart** button on the home window when there is one. It never
installs anything on its own, and it refuses to restart while a session is
open, because a session lives in memory. Every update is signed and verified
against a key built into the program before it runs. Switch the check off
under **Look for new versions** in Settings.

## Removing it

**Remove Incredibulk** in the tray menu clears what the program leaves behind:
the login entry, your settings and saved sessions, and the webview cache (tens
of megabytes). It asks first and lists exactly what will go. If you used the
installer, finish by uninstalling the program from Windows Settings under
Installed apps; a portable copy is just a file to delete. Anything it could not
remove itself is reported, with the paths put on your clipboard.

## Building

You need a Rust toolchain and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform. There is no npm step: the frontend is plain HTML, CSS and JavaScript
served straight from the bundle.

```sh
bash tools/setup-linux.sh          # Ubuntu 22.04+ / Debian 12+: native libraries, once
cargo test --workspace             # the model, the shell, the platform helpers
cargo run -p incredibulk
cargo build -p incredibulk --release
```

**Releases** are built by CI: push a `v*` tag and the release workflow builds
and signs the Windows installer, then attaches it and the update manifest to a
draft release.

**Windows packaging by hand**, for a build to give to someone:

- `tools\installer.ps1` runs Clippy and the tests, builds the NSIS installer,
  writes it with its SHA-256 into `dist/`, and adds an
  `Incredibulk-update-<version>` folder with a one-click launcher that backs up
  your data, installs silently and restarts the app. Needs the x64 MSVC
  toolchain, `cargo install tauri-cli --locked` and 7-Zip.
- `tools\package.ps1` builds the portable zip instead.
- `cd app && cargo tauri build` produces the platform bundles (`.msi`, NSIS,
  `.dmg`, `.deb`, `.AppImage`). Nothing is code signed.

Building from a network or WSL mount fails on file locks; point
`CARGO_TARGET_DIR` at a local disk. Icons are generated by
`tools/make_icons.py`, not checked in.

## Layout

| Path | What is in it |
| --- | --- |
| [core/](core/) | The model: session, capture rules, editing, history, rendering, config. Pure Rust, no OS, no UI. Most of the tests live here. |
| [app/](app/) | The Tauri shell: clipboard watcher, global shortcuts, tray, windows, persistence, updater. |
| [ui/](ui/) | The four windows (home, stack, history, settings). Plain HTML, CSS and JavaScript. |
| [tools/](tools/) | Packaging and update scripts, Linux dependency setup, icon generator. |

Anything that decides what a session does belongs in `core`, where it can be
tested without a desktop.

## Contributing

Issues and pull requests welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) and
[SECURITY.md](SECURITY.md). macOS and Linux are the thin spots: the platform
table above is the open work.

## License

MIT. See [LICENSE](LICENSE).
