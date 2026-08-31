# Incredibulk

**Copy ten things in a row, paste them all at once.**

[![ci](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml/badge.svg)](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Your clipboard holds one thing at a time, so gathering six of them means six
trips back and forth. Incredibulk adds a **session**: while one is open, every
copy is added to a stack instead of replacing the last.

1. **`Ctrl+Alt+C`** opens a session.
2. **Copy as usual**, from anywhere, as much as you like.
3. **`Ctrl+Alt+V`** pastes all of it as one block.

A window lists what you have collected while you work. In it you can untick a
line to leave it out, click one to edit it, drag them into order, or type a line
that was never copied at all. Finished sessions are kept, so you can paste one
again later.

This is not a clipboard history you pick items out of one at a time. A session
has a beginning and an end, and the end produces a single block.

## Shortcuts

| | |
| --- | --- |
| `Ctrl+Alt+C` | Open a session, or discard the open one |
| `Ctrl+Alt+V` | Paste everything, or repeat the last session |
| `Ctrl+Alt+X` | Discard without pasting |
| `Ctrl+Alt+Z` | Undo the last capture |
| `Ctrl+Alt+S` | Show or hide the stack window |
| `Ctrl+Alt+H` | Open the history |

All rebindable in settings. `Ctrl+Alt` rather than `Ctrl+Shift` is deliberate:
`Ctrl+Shift+C` and `Ctrl+Shift+V` are already copy and paste in most terminals.

## Getting it

Grab the zip from [releases](../../releases), unzip, run `incredibulk.exe`.
Nothing installs; it is one file. The app has no window of its own and lives in
the system tray, where clicking its icon opens the front door.

Windows is the platform it is built and tested on. It compiles for macOS and
Linux with gaps, listed in [the manual](docs/manual.md#platform-notes).

## Building

Needs a Rust toolchain and the Tauri system dependencies for your platform.
There is no npm step: the frontend is plain HTML, CSS and JavaScript.

```sh
cargo test              # the model, the store, the platform helpers
cargo run -p incredibulk
```

To hand a build to someone else, `tools/package.ps1` writes a zip with the
executable and a note for whoever receives it.

## How it is put together

| | |
| --- | --- |
| [core/](core/) | The model: sessions, capture rules, editing, history, rendering. Pure Rust, no filesystem, no OS, no UI toolkit. |
| [app/](app/) | The shell: clipboard, shortcuts, tray, windows, persistence. |
| [ui/](ui/) | Three windows, in plain HTML, CSS and JavaScript. |

Everything deciding *what a session does* lives in `core`, where it can be
tested on any machine, which is why it carries most of the tests. **This is not
hexagonal architecture**: there are no ports and no injected adapters, because a
single-user clipboard tool does not earn that ceremony. It is a pure model with
a shell around it.

## More

[Manual](docs/manual.md) for the details: the stack window, history, images,
output templates, platform notes and removal. [Contributing](CONTRIBUTING.md),
[security](SECURITY.md), [changelog](CHANGELOG.md).

MIT.
