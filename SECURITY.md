# Security

## Reporting a vulnerability

Please report privately through GitHub's
[security advisories](../../security/advisories/new) rather than opening a
public issue. I will acknowledge within a week.

## What this program can see

Worth stating plainly, because a clipboard tool sits somewhere sensitive.

- **While a session is open, everything you copy is held in memory**, including
  anything you copy by accident: a password out of a manager, a token, a
  private message. Discard the session and it is gone.
- **Finished sessions are written to disk unencrypted**, in the settings folder,
  in plain JSON, and kept until they fall off the end of the history. If that is
  not acceptable for your machine, switch off `Keep finished sessions` in the
  settings, or lower the limit.
- **The window title of whatever you copied from is recorded** for the
  `{source}` token, and titles often contain document or customer names. Switch
  off `Remember which window each fragment came from` if that matters.
- **Nothing is ever sent anywhere.** There is no network code in this program
  and no telemetry. The only thing that leaves it is what you paste.

## Scope

Reports about the items above being *by design* are welcome as feature requests
rather than vulnerabilities. What I would treat as a vulnerability: captured
content reaching a place the user did not paste it, the program running code
from clipboard contents, or privilege escalation through the removal flow.
