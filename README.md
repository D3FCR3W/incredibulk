# Incredibulk

**Copy ten things in a row, paste them all at once.**

[![ci](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml/badge.svg)](https://github.com/D3FCR3W/incredibulk/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

The system clipboard is a register of size one: every copy destroys the last
one, which is why collecting six things means six trips back and forth.
Incredibulk adds a **session**, turning the clipboard into an append-only stack
with an explicit begin and commit.

1. `Ctrl+Alt+C` opens a session.
2. Copy as many fragments as you like, from anywhere: editor, browser, PDF,
   terminal. Nothing is lost, and you keep using `Ctrl+C`.
3. The stack window shows what you have, and you work in it: untick a line to
   leave it out, click a line to edit it, drag lines into the order you want,
   or type one that was never copied at all.
4. `Ctrl+Alt+V` renders what is left as one block and pastes it into whatever
   window you are in.

This is not a clipboard history you pick items back out of one at a time. The
session has a beginning and an end, and the end produces one block.

## Getting to it

incredibulk has no window of its own until you ask for one. Its front door is the
tray icon: click it with no session running and the home screen opens, with one
button to start a session and the way through to history and settings. Click it
during a session and the stack comes up instead.

The first launch opens that home screen as a five step introduction, which can
be skipped at any point and reopened later from the bottom of the same screen.
It offers the choices worth making up front: start on login (on by default),
keep finished sessions, and whether a paste carries images.

## The stack window

It appears when a session opens and stays there while it is running, without
taking focus away from what you are doing.

| In the list | What it does |
| --- | --- |
| Tick box | Leaves a line out of the paste without deleting it. Unticked lines stay visible and can be put back. |
| Number | Where that line will land in the block. Unticked lines have none, and the rest close up behind them. |
| Click the text | Edits it in place. Enter commits, Shift+Enter adds a line, Escape reverts. Emptying it removes the line. |
| Grip handle | Drag a line anywhere in the stack, with a drop line showing where it lands. Focus it and use the arrow keys if you would rather not drag. |
| Close button | Puts the window away for the rest of the session. Capturing carries on; the tray icon still shows the session is live. |
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

Sessions can be named while they run or renamed afterwards, from the button
on each row. Tick one session to paste it again. Tick several and they paste as a single
block, combined in the order they happened, oldest first, and renumbered across
the whole thing. `Copy without pasting` puts the block on the clipboard instead.

Images are kept too, up to 4 MB each; anything larger keeps its dimensions but
not its pixels, so it replays as a description rather than a picture.

## Images

An image cannot go into a text block, it can only be described. So when a
session contains one, the block is put on the clipboard twice: as rich text with
the image embedded, and as plain text where the image is its placeholder.

Applications that ask for formatting get the picture. Editors, terminals and
anything else asking for plain text get exactly the text they would have got
otherwise, so nothing is worse off for this. If you would rather never send
formatting at all, set the paste format to **Plain text only** in settings, and
images will always paste as `[image 1920x1080]`.

## Shortcuts

| Action | Default |
| --- | --- |
| Open a session, or discard the open one | `Ctrl+Alt+C` |
| Flush and paste, or repeat the last session | `Ctrl+Alt+V` |
| Discard without pasting | `Ctrl+Alt+X` |
| Undo the last capture | `Ctrl+Alt+Z` |
| Show or hide the stack window | `Ctrl+Alt+S` |
| Open the history | `Ctrl+Alt+H` |

All five are rebindable in settings, which you reach from the tray icon or the
gear in the stack window. `Ctrl+Alt` rather than `Ctrl+Shift` is deliberate:
`Ctrl+Shift+C` and `Ctrl+Shift+V` are already copy and paste in most terminals.

## Pasting when nothing is collecting

`Ctrl+Alt+V` always pastes something. With a session running it pastes what you
collected. With no session, or one that has caught nothing yet, it falls back to
the history:

- **Sessions ticked in the history list** are pasted, combined, oldest first.
  Ticking is an explicit choice made in a list that says what the shortcut will
  do, so it wins.
- **Nothing ticked** pastes the most recent session again.

Either way it can be pressed as many times as you like, and it says which rule
applied. The shortcut therefore means one thing at all times: put that block
where the cursor is.

The single exception is a live session whose fragments you have all unticked.
That is a decision you just made inside that session, so it says so rather than
quietly pasting a different one behind your back.

## How a copy is noticed

While a session is open the clipboard itself is watched, rather than the
`Ctrl+C` keystroke being intercepted. That way a fragment is captured however
it was copied: the keyboard, a right click, an application with its own copy
binding. On Windows the watcher first checks the clipboard sequence number, so
polling costs nothing until something actually changes.

## Output format

The block is rendered from a template, not a fixed join. Presets cover the
common shapes (one per line, blank line between, numbered, markdown bullets,
fenced block, comma separated) and every field is editable, with a live
preview of your actual session.

Tokens: `{content}`, `{index}`, `{index0}`, `{count}`, `{source}`, `{kind}`,
`{time}`, `{date}`. Image placeholders also take `{width}` and `{height}`;
file paths take `{path}`, `{name}`, `{stem}`, `{ext}` and `{dir}`. An unknown
token is left exactly as typed rather than silently dropped.

## Building

Requires a Rust toolchain and the Tauri system dependencies for your platform.
There is no npm step: the frontend is plain HTML, CSS and JavaScript in
[ui/](ui/), served straight from the bundle.

```sh
cargo test -p incredibulk-core     # the session model, capture rules and templates
cargo run  -p incredibulk          # run it
cargo build -p incredibulk --release
```

### Giving a build to someone else

```powershell
powershell -ExecutionPolicy Bypass -File tools\package.ps1
```

This builds in release and writes `dist/incredibulk-<version>-windows-x64.zip`,
containing the executable and a note for whoever receives it: how to get past
the SmartScreen warning, the two shortcuts, what it needs, and where the crash
log lives. Around 2.3 MB zipped.

A zip rather than an installer because incredibulk genuinely is one file. Asking
someone to run an installer for a single executable is friction with nothing
behind it, and the tray menu already removes what the app leaves behind.

If you do want installers (`.msi` and `.exe` setup on Windows, `.dmg` on macOS,
`.deb` and `.AppImage` on Linux):

```sh
cargo install tauri-cli --version '^2'
cd app && cargo tauri build
```

**Neither is code signed.** Windows will show "Windows protected your PC" on
first run, and the reader has to click "More info" then "Run anyway". Getting
rid of that needs a code signing certificate, and an EV one before SmartScreen
trusts a new publisher immediately.

Icons are generated rather than checked in as opaque binaries. Edit the
geometry at the top of [tools/make_icons.py](tools/make_icons.py) and re-run it
to change the mark.

### Building from this working copy

The project lives on a WSL 9p mount, which cannot take the file locks rustc
needs for incremental compilation. `.cargo/config.toml` (gitignored) points
`target-dir` at a native Windows path so builds work in place. On a normal
checkout, delete it.

## Layout

| Path | What is in it |
| --- | --- |
| [core/](core/) | The model: session, capture policy, editing, history, plain and HTML rendering, config. Pure Rust, no OS, no threads, fully unit tested. |
| [app/](app/) | The Tauri shell: clipboard thread, global shortcuts, tray, windows. |
| [ui/](ui/) | The stack window and the settings window. Plain HTML, CSS and JavaScript. |

The split is the point: everything that decides *what a session does* is in
`core` and can be tested without a desktop, while `app` owns only the parts
that need a real machine. `core` has no filesystem access and no dependency on
the UI toolkit, which is why it carries most of the tests.

**This is not hexagonal architecture**, and calling it that would be a stretch.
There are no ports and no injected adapters: the shell calls the model
directly. A single-user clipboard tool does not earn that ceremony. What it is
is a pure model with a shell around it, and the boundary is real enough that
the model compiles and tests on any machine with a Rust toolchain.

## Platform notes

- **Windows** is the platform this was built and tested on. Everything works:
  shortcuts, watching, paste, tray, autostart, copied files, copied images, and
  a second launch handing over to the instance already running.
- **macOS** needs Accessibility permission before the synthesized paste
  keystroke will do anything; the system prompts on first use. Reading the
  focused window title for `{source}` is not implemented, so that token is
  blank.
- **Linux** works under X11. Under Wayland, global shortcuts depend on the
  compositor and input synthesis is restricted, so `Paste automatically` may
  need to be switched off, leaving the block on the clipboard for you to paste.
  `{source}` is blank there too.
- Copied **file lists** are read on Windows only. Copying files in Explorer
  adds their paths to the session. macOS and Linux would need their own
  pasteboard readers, so a file copy is simply not seen there.
- **Rich paste** is written through the clipboard's HTML flavour, which exists
  on all three platforms, but has only been tested on Windows.

## Removing it

There is no installer, so there is nothing for Windows to uninstall: the program
is one portable file. What it does leave behind is easy to miss, so **Remove
incredibulk** in the tray menu takes care of it: the login entry, your settings and
saved sessions, and the webview cache, which quietly grows to tens of megabytes.

It asks first and says exactly what will go. The executable itself cannot delete
itself while running, and the cache folder is usually held open, so whatever is
left is reported rather than silently skipped, with the paths put on your
clipboard.

## Settings file

`config.json`, alongside `history.json`, in the platform config directory
(`%APPDATA%\dev.incredibulk.app` on Windows, `~/Library/Application Support/dev.incredibulk.app`
on macOS, `$XDG_CONFIG_HOME/dev.incredibulk.app` on Linux).

If the app ever disappears without a word, `crash.log` in the same folder is
where it says why.

Missing fields fall back to defaults and unknown ones are ignored, so a file
written by an older or newer build still loads. A file that cannot be parsed
does not stop the app: it starts on defaults and shows you the reason in
settings, rather than quietly overwriting what you wrote.
