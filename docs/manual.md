# Manual

Everything the [README](../README.md) leaves out.

## Getting to it

Incredibulk has no window of its own until you ask for one. Its front door is
the tray icon: click it with no session running and the home screen opens, with
one button to start a session and the way through to history and settings.
Click it during a session and the stack comes up instead.

The first launch opens that home screen as a five step introduction, which can
be skipped at any point and reopened later from the bottom of the same screen.
It offers the choices worth making up front: start on login (on by default),
keep finished sessions, and whether a paste carries images.

## The stack window

It appears when a session opens and stays while it runs, without taking focus
from what you are doing.

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
rather than a line of text describing them.

The window appears without taking focus, so opening a session never interrupts
what you are typing. The flip side is that it is a background window: the first
click on it activates it, as with any other.

## History

A session is kept when you paste it, up to 50 of them, surviving a restart.
Open it from the tray, from the clock icon in the stack window header, or with
`Ctrl+Alt+H`.

Sessions can be named while they run, or renamed afterwards from the button on
each row. Tick one to paste it again. Tick several and they paste as a single
block, combined in the order they happened, oldest first, and renumbered across
the whole thing. `Copy without pasting` puts the block on the clipboard instead.

Images are kept too, up to 4 MB each. Anything larger keeps its dimensions but
not its pixels, so it replays as a description rather than a picture.

## Pasting when nothing is collecting

`Ctrl+Alt+V` always pastes something. With a session running it pastes what you
collected. With no session, or one that has caught nothing yet, it falls back to
the history:

- **Sessions ticked in the history list** are pasted, combined, oldest first.
  Ticking is an explicit choice made in a list that says what the shortcut will
  do, so it wins.
- **Nothing ticked** pastes the most recent session again.

Either way it can be pressed as often as you like, and it says which rule
applied. The shortcut therefore means one thing at all times: put that block
where the cursor is.

The single exception is a live session whose fragments you have all unticked.
That is a decision you just made inside that session, so it says so rather than
quietly pasting a different one behind your back.

## How a copy is noticed

While a session is open the clipboard itself is watched, rather than the
`Ctrl+C` keystroke being intercepted. That way a fragment is captured however it
was copied: the keyboard, a right click, an application with its own binding. On
Windows the watcher first checks the clipboard sequence number, so polling costs
nothing until something actually changes.

## Images

An image cannot go into a text block, it can only be described. So when a
session contains one, the block goes onto the clipboard twice: as rich text with
the image embedded, and as plain text where the image is its placeholder.

Applications that ask for formatting get the picture. Editors, terminals and
anything else asking for plain text get exactly the text they would have got
otherwise, so nothing is worse off for this. To never send formatting at all,
set the paste format to **Plain text only** in settings, and images always paste
as `[image 1920x1080]`.

## Output format

The block is rendered from a template, not a fixed join. Presets cover the
common shapes (one per line, blank line between, numbered, markdown bullets,
fenced block, comma separated) and every field is editable, with a live preview
of your actual session.

Tokens: `{content}`, `{index}`, `{index0}`, `{count}`, `{source}`, `{kind}`,
`{time}`, `{date}`. Image placeholders also take `{width}` and `{height}`; file
paths take `{path}`, `{name}`, `{stem}`, `{ext}` and `{dir}`. An unknown token
is left exactly as typed rather than silently dropped.

## Settings file

`config.json`, alongside `history.json`, in the platform config directory:

| | |
| --- | --- |
| Windows | `%APPDATA%\dev.incredibulk.app` |
| macOS | `~/Library/Application Support/dev.incredibulk.app` |
| Linux | `$XDG_CONFIG_HOME/dev.incredibulk.app` |

Missing fields fall back to defaults and unknown ones are ignored, so a file
written by an older or newer build still loads. A file that cannot be parsed
does not stop the app: it starts on defaults and shows the reason in settings,
rather than quietly overwriting what you wrote.

If the app ever disappears without a word, `crash.log` in that same folder says
why.

## Removing it

There is no installer, so there is nothing for Windows to uninstall. What the
program leaves behind is easy to miss, so **Remove Incredibulk** in the tray
menu takes care of it: the login entry, your settings and saved sessions, and
the webview cache, which quietly grows to tens of megabytes.

It asks first and says exactly what will go. The executable cannot delete itself
while running and the cache folder is usually held open, so whatever is left is
reported rather than silently skipped, with the paths put on your clipboard.

## Platform notes

- **Windows** is what this is built and tested on. Everything works: shortcuts,
  watching, paste, tray, autostart, copied files, copied images, and a second
  launch handing over to the instance already running.
- **macOS** needs Accessibility permission before the synthesized paste
  keystroke does anything; the system prompts on first use. Reading the focused
  window title for `{source}` is not implemented, so that token is blank.
- **Linux** works under X11. Under Wayland, global shortcuts depend on the
  compositor and input synthesis is restricted, so `Paste automatically` may
  need switching off, leaving the block on the clipboard for you to paste.
  `{source}` is blank there too.
- Copied **file lists** are read on Windows only. macOS and Linux would need
  their own pasteboard readers, so a file copy is simply not seen there.
- **Rich paste** goes through the clipboard's HTML flavour, which exists on all
  three platforms, but has only been tested on Windows.

## Packaging

```powershell
powershell -ExecutionPolicy Bypass -File tools\package.ps1
```

Builds in release and writes `dist/incredibulk-<version>-windows-x64.zip`: the
executable and a note for whoever receives it, covering the SmartScreen warning,
the shortcuts, what it needs and where the crash log lives. Around 2.3 MB.

A zip rather than an installer because the program genuinely is one file. Asking
someone to run an installer for a single executable is friction with nothing
behind it, and the tray menu already removes what it leaves behind.

For real installers (`.msi` and NSIS setup on Windows, `.dmg` on macOS, `.deb`
and `.AppImage` on Linux):

```sh
cargo install tauri-cli --version '^2'
cd app && cargo tauri build
```

**Neither is code signed.** Windows shows "Windows protected your PC" on first
run, and the reader has to click "More info" then "Run anyway". Removing that
needs a code signing certificate, and an EV one before SmartScreen trusts a new
publisher immediately.

Icons are generated rather than checked in as opaque binaries. Edit the geometry
at the top of [tools/make_icons.py](../tools/make_icons.py) and re-run it.

## Building on a network path

If the working copy sits on a WSL or network mount, rustc cannot take the file
locks it needs for incremental compilation. Point the build output at a local
disk instead:

```sh
CARGO_TARGET_DIR=/some/local/path cargo build
```
