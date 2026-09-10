# Contributing

Thanks for looking. This is a small tool with a small surface, so the bar for
contributing is low and the guidance is short.

## Getting it running

You need a Rust toolchain. There is no npm step: the frontend is plain HTML,
CSS and JavaScript, served as-is.

On Ubuntu 22.04+ or Debian 12+, run `bash tools/setup-linux.sh` first to install
the native desktop dependencies. It asks for your sudo password if needed.

```sh
cargo test              # the model, the store, and the platform helpers
cargo run -p incredibulk
```

## Where things live

| Path | What is in it |
| --- | --- |
| `core/` | The model. Sessions, capture rules, editing, history, rendering. Pure Rust: no filesystem, no OS, no threads. |
| `app/` | The shell. Clipboard, shortcuts, tray, windows, persistence. |
| `ui/` | The four windows (home, stack, history, settings), in plain HTML, CSS and JavaScript. |

**The split is the point.** Anything that decides *what a session does* belongs
in `core`, where it can be tested without a desktop. Anything that touches a
clipboard, a file, a window or a keyboard belongs in `app`. If you find
yourself wanting `std::fs` inside `core`, that is the signal that the logic and
the plumbing have got mixed up.

## Before opening a pull request

```sh
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test
```

CI runs exactly these, and treats warnings as errors.

## What makes a change easy to accept

- **A test for the behaviour, in `core` if it can live there.** The model has
  over a hundred tests and they are the reason it can be changed confidently.
- **A commit message that says why**, not what. The diff already says what.
- **Comments that explain a decision**, not the syntax. If a line looks odd and
  is deliberate, say what would break without it.

## Reporting a bug

Include what you did, what happened, and what you expected. If the app vanished
without a word, `crash.log` in the settings folder is the useful part:

- Windows: `%APPDATA%\dev.incredibulk.app\`
- macOS: `~/Library/Application Support/dev.incredibulk.app/`
- Linux: `$XDG_CONFIG_HOME/dev.incredibulk.app/`

## Platforms

It is built and tested on Windows. It compiles for macOS and Linux and the
model is platform-agnostic, but the shell has gaps there, listed in the README.
Patches closing them are very welcome, especially the pasteboard readers for
copied files and the focused-window title.
