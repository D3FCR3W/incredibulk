# Changelog

Notable changes, newest first. Follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) loosely and
[semantic versioning](https://semver.org/).

## [Unreleased]

## [0.1.1] - 2026-09-07

### Added

- A Windows installer build script that runs checks and produces a checksum.
- A one-click Windows update launcher with verified backups, silent update mode,
  data restoration on failure, and checks before restarting the application.
- A native dependency installer for Ubuntu/Debian and full desktop checks in
  Linux CI.
- History search across session names, full text, file paths and source windows,
  with keyboard shortcuts and a selection that can span several searches.
- A deselect-all button and a count of selected sessions hidden by the search.

### Fixed

- Windows update packages hash the executable inside the installer, including
  Tauri's bundle marker, instead of the unpatched build output.
- Undo removes the latest capture even after the stack has been reordered.
- Out-of-order preview responses no longer show a previous history selection.
- Select-all in history text fields keeps its normal text-editing behavior.

## [0.1.0]

First release.

### Added

- Copy sessions: `Ctrl+Alt+C` opens one, every copy appends instead of
  overwriting, `Ctrl+Alt+V` pastes the whole thing as one block.
- A stack window you work in: untick a line to leave it out without losing it,
  click a line to edit it, drag lines into order, type a line by hand, and name
  the session.
- Templates for the pasted block, with presets and a live preview.
- History of finished sessions, kept across restarts. Tick several and they
  paste combined, oldest first. With nothing collecting, `Ctrl+Alt+V` pastes the
  ticked sessions, or the most recent one if none are ticked.
- Images: shown as thumbnails in the list, and carried into a rich paste, with
  the plain text block always written alongside.
- Copied file lists, on Windows.
- A first-run introduction that can be skipped, and a home window reachable from
  the tray at any time.
- Removal from the tray menu, covering the login entry, the settings and the
  webview cache.
