/* Turning an accelerator into something a person reads.
 *
 * Shared by every window that shows a shortcut. It was written three times
 * before this file existed, which is two times too many for eight lines that
 * have to agree with each other.
 *
 * No modules here on purpose: the frontend is plain files with no build step,
 * so this defines one global and every page loads it before its own script. */

const IS_MAC = navigator.userAgent.includes("Mac");

/* `CmdOrCtrl` is what the backend stores, because one binding has to mean
 * Command on a Mac and Control everywhere else. Nobody wants to read that. */
function formatShortcut(accelerator, fallback = "the shortcut") {
  if (!accelerator) return fallback;
  return accelerator
    .replace(/CmdOrCtrl|CommandOrControl/gi, IS_MAC ? "Cmd" : "Ctrl")
    .replace(/\bSuper\b/gi, IS_MAC ? "Cmd" : "Win");
}
