<div align="center">

# Incredibulk

**Copy ten things in a row. Paste them all at once.**

[![ci](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml/badge.svg)](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![release](https://img.shields.io/github/v/release/D3FCR3W/incredibulk?include_prereleases&sort=semver)](../../releases)

<!-- TODO: record docs/demo.gif and uncomment the line below.
     Ten seconds, no narration: open a session, copy four things from
     different windows, untick one, drag another, paste into a document.
     <img src="docs/demo.gif" alt="A session collecting four fragments and pasting them as one block" width="640"> -->

</div>

The system clipboard is a register of size one. Every copy destroys the last
one, which is why collecting six things means six trips back and forth.

Incredibulk adds a **session**: an explicit begin, an append-only stack, one
commit.

## How it works

1. **`Ctrl+Alt+C`** opens a session.
2. **Copy as much as you like**, from anywhere. Editor, browser, PDF, terminal.
   Keep using `Ctrl+C`. Nothing is lost.
3. **Work in the stack window.** Untick a line to leave it out, click it to
   edit, drag to reorder, or type one that was never copied at all.
4. **`Ctrl+Alt+V`** renders what is left as one block and pastes it into
   whatever window you are in.

This is not a clipboard history you pick items back out of one at a time. The
session has a beginning and an end, and the end produces one block.

## Install

### Windows

Download the latest zip from [Releases](../../releases), unzip it, run
`incredibulk.exe`. Around 2.3 MB zipped.

A zip rather than an installer because Incredibulk genuinely is one file. Asking
someone to run an installer for a single executable is friction with nothing
behind it, and the tray menu already removes what the app leaves behind.

First run shows **"Windows protected your PC"**. Click *More info*, then *Run
anyway*. The build is not code signed. Getting rid of that warning needs a code
signing certificate, and an EV one before SmartScreen trusts a new publisher
immediately.

### macOS and Linux

No prebuilt binaries yet. [Build from source](#building), then check
[platform support](#platform-support) for what works where.

## Shortcuts

| Action | Default |
| --- | --- |
| Open a session, or discard the open one | `Ctrl+Alt+C` |
| Flush and paste, or repeat the last session | `Ctrl+Alt+V` |
| Discard without pasting | `Ctrl+Alt+X` |
| Undo the last capture | `Ctrl+Alt+Z` |
| Show or hide the stack window | `Ctrl+Alt+S` |
| Open the history | `Ctrl+Alt+H` |

All rebindable in settings, which you reach from the tray icon or the gear in
the stack window.

`Ctrl+Alt` rather than `Ctrl+Shift` is deliberate: `Ctrl+Shift+C` and
`Ctrl+Shift+V` are already copy and paste in most terminals.

## The tray icon is the front door

Incredibulk has no window of its own until you ask for one.

Click the tray icon with no session running and the home screen opens, with one
button to start a session and the way through to history and settings. Click it
during a session and the stack comes up instead.

First launch turns that home screen into a five step introduction. Skippable at
any point, reopened later from the bottom of the same screen. It offers the
choices worth making up front: start on login (on by default), keep finished
sessions, and whether a paste carries images.

## The stack window

It appears when a session opens and stays there while it runs, without taking
focus away from what you are doing.

| In the list | What it does |
| --- | --- |
| Tick box | Leaves a line out of the paste without deleting it. Unticked lines stay visible and can be put back. |
| Number | Where that line will land in the block. Unticked lines have none, and the rest close up behind them. |
| Click the text | Edits it in place. Enter commits, Shift+Enter adds a line, Escape reverts. Emptying it removes the line. |
| Grip handle | Drag a line anywhere in the stack, with a drop line showing where it lands. Focus it and use the arrow keys if you would rather not drag. |
| Close button | Puts the window away for the rest of the session. Capturing carries on, and the tray icon still shows the session is live. |
| The title | Click it to name the session. The name follows it into the history, where sessions are otherwise a wall of timestamps. |
| Bottom field | Types a line that was never on the clipboard. |
| Tick all / Untick all | Flips the whole stack at once. |

The counter reads `3 of 5` when some lines are unticked, and the paste button
always says how many lines are actually going in. Images appear as thumbnails
rather than as a line of text describing them.

The window appears without taking focus, so opening a session never interrupts
what you are typing. The flip side is that it is a background window: the first
click on it activates it, as with any other.

## History

A session is kept when you paste it, up to 50 of them, and survives a restart.
Open it from the tray, from the clock icon in the stack window header, or with
`Ctrl+Alt+H`.

- Name or rename a session while it runs or afterwards, from the button on each
  row.
- Tick one session to paste it again.
- Tick several and they paste as a single block, combined in the order they
  happened, oldest first, renumbered across the whole thing.
- **Copy without pasting** puts the block on the clipboard instead.

Images are kept too, up to 4 MB each. Anything larger keeps its dimensions but
not its pixels, so it replays as a description rather than a picture.

## What `Ctrl+Alt+V` does when nothing is collecting

It always pastes something. With a session running it pastes what you collected.
With no session, or one that has caught nothing yet, it falls back to the
history:

| Situation | What gets pasted |
| --- | --- |
| Sessions ticked in the history list | Those sessions, combined, oldest first |
| Nothing ticked | The most recent session, again |

Ticking is an explicit choice made in a list that says what the shortcut will
do, so it wins. Either way the shortcut can be pressed as many times as you
like, and it says which rule applied. It therefore means one thing at all times:
put that block where the cursor is.

The single exception is a live session whose fragments you have all unticked.
That is a decision you just made inside that session, so it says so rather than
quietly pasting a different one behind your back.

## Images

An image cannot go into a text block, it can only be described. So when a
session contains one, the block is put on the clipboard twice:

- **as rich text**, with the image embedded
- **as plain text**, where the image is its placeholder

Applications that ask for formatting get the picture. Editors, terminals and
anything else asking for plain text get exactly the text they would have got
otherwise, so nothing is worse off for this.

If you would rather never send formatting at all, set the paste format to
**Plain text only** in settings. Images then always paste as `[image 1920x1080]`.

## Output format

The block is rendered from a template, not a fixed join. Presets cover the
common shapes (one per line, blank line between, numbered, markdown bullets,
fenced block, comma separated) and every field is editable, with a live preview
of your actual session.

| Scope | Tokens |
| --- | --- |
| Any line | `{content}` `{index}` `{index0}` `{count}` `{source}` `{kind}` `{time}` `{date}` |
| Image placeholders | `{width}` `{height}` |
| File paths | `{path}` `{name}` `{stem}` `{ext}` `{dir}` |

An unknown token is left exactly as typed rather than silently dropped.

## How a copy is noticed

While a session is open the clipboard itself is watched, rather than the
`Ctrl+C` keystroke being intercepted. That way a fragment is captured however it
was copied: the keyboard, a right click, an application with its own copy
binding.

On Windows the watcher first checks the clipboard sequence number, so polling
costs nothing until something actually changes.

## Platform support

| | Windows | macOS | Linux (X11) | Linux (Wayland) |
| --- | --- | --- | --- | --- |
| Shortcuts, watching, tray, autostart | Yes | Yes | Yes | Compositor dependent |
| Automatic paste | Yes | Needs Accessibility | Yes | Often unavailable |
| Copied images | Yes | Yes | Yes | Yes |
| Copied file lists | Yes | No | No | No |
| `{source}` window title | Yes | No | No | No |
| Rich paste (HTML flavour) | Tested | Untested | Untested | Untested |

**Windows** is the platform this was built and tested on. Everything there
works, including a second launch handing over to the instance already running.

**macOS** prompts for Accessibility permission on first use, and the synthesized
paste keystroke does nothing until it is granted.

**Wayland** restricts input synthesis, so *Paste automatically* may need to be
switched off, leaving the block on the clipboard for you to paste yourself.

**Copied file lists** are read through the Windows pasteboard only. Copying
files in Explorer adds their paths to the session. macOS and Linux would need
their own readers, so a file copy is simply not seen there.

## Files it writes

`config.json` and `history.json`, in the platform config directory:

| OS | Path |
| --- | --- |
| Windows | `%APPDATA%\dev.incredibulk.app` |
| macOS | `~/Library/Application Support/dev.incredibulk.app` |
| Linux | `$XDG_CONFIG_HOME/dev.incredibulk.app` |

Missing fields fall back to defaults and unknown ones are ignored, so a file
written by an older or newer build still loads. A file that cannot be parsed
does not stop the app: it starts on defaults and shows you the reason in
settings, rather than quietly overwriting what you wrote.

If the app ever disappears without a word, `crash.log` in the same folder is
where it says why.

## Uninstalling

There is no installer, so there is nothing for Windows to uninstall. The program
is one portable file.

What it leaves behind is easy to miss, so **Remove Incredibulk** in the tray
menu takes care of it: the login entry, your settings and saved sessions, and
the webview cache, which quietly grows to tens of megabytes.

It asks first and says exactly what will go. The executable cannot delete itself
while running, and the cache folder is usually held open, so whatever is left is
reported rather than silently skipped, with the paths put on your clipboard.

## Building

Requires a Rust toolchain and the Tauri system dependencies for your platform.
There is no npm step: the frontend is plain HTML, CSS and JavaScript in [ui/](ui/),
served straight from the bundle.

```sh
cargo test  -p incredibulk-core     # session model, capture rules, templates
cargo run   -p incredibulk          # run it
cargo build -p incredibulk --release
```

<details>
<summary><strong>Packaging a build for someone else</strong></summary>

```powershell
powershell -ExecutionPolicy Bypass -File tools\package.ps1
```

Builds in release and writes `dist/incredibulk-<version>-windows-x64.zip`,
containing the executable and a note for whoever receives it: how to get past
the SmartScreen warning, the two shortcuts, what it needs, and where the crash
log lives.

If you do want real installers (`.msi` and NSIS setup on Windows, `.dmg` on
macOS, `.deb` and `.AppImage` on Linux):

```sh
cargo install tauri-cli --version '^2'
cd app && cargo tauri build
```

Neither is code signed.

</details>

<details>
<summary><strong>Building from a network or WSL working copy</strong></summary>

A 9p or SMB mount cannot take the file locks rustc needs for incremental
compilation, and the build fails before it starts. Point the output at a local
disk:

```sh
CARGO_TARGET_DIR=/some/local/path cargo build
```

</details>

<details>
<summary><strong>Icons</strong></summary>

Generated rather than checked in as opaque binaries. Edit the geometry at the
top of [tools/make_icons.py](tools/make_icons.py) and re-run it to change the
mark.

</details>

## Layout

| Path | What is in it |
| --- | --- |
| [core/](core/) | The model: session, capture policy, editing, history, plain and HTML rendering, config. Pure Rust, no filesystem, no OS, no UI toolkit. |
| [app/](app/) | The shell: clipboard thread, global shortcuts, tray, windows, persistence. |
| [ui/](ui/) | The four windows. Plain HTML, CSS and JavaScript. |

The split is the point. Everything that decides *what a session does* lives in
`core` and can be tested without a desktop, which is why it carries most of the
tests.

**This is not hexagonal architecture**, and calling it that would be a stretch.
There are no ports and no injected adapters: the shell calls the model directly.
A single-user clipboard tool does not earn that ceremony. What it is is a pure
model with a shell around it, and the boundary is real enough that the model
compiles and tests on any machine with a Rust toolchain.

## Contributing

Issues and pull requests welcome. Anything touching session behaviour, capture
rules or templates belongs in `core`, with a test next to it. See
[CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).

macOS and Linux are the thin spots: the platform table above is a list of open
work as much as a list of caveats.

## License

MIT. See [LICENSE](LICENSE).
