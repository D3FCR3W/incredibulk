//! The clipboard service: one thread that owns the only clipboard handle.
//!
//! Everything that touches the system clipboard happens here, on one thread,
//! for two reasons. On X11 a clipboard owner has to stay alive to answer
//! selection requests, so a handle created and dropped per operation loses the
//! selection the moment it returns. And a single owner means reads, writes and
//! the change-detection baseline cannot interleave and confuse each other.
//!
//! Other threads talk to it by message. The reply channel makes a write
//! synchronous from the caller's point of view, which matters: the paste
//! keystroke must not be sent before the block is actually on the clipboard.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel};
use std::thread;
use std::time::Duration;

use arboard::{Clipboard, ImageData};
use incredibulk_core::{ClipKind, ImagePayload};
use tauri::AppHandle;

use crate::platform;

/// How long to wait between clipboard reads when no session is open. Nothing
/// is being collected, so this only needs to be responsive enough that the
/// first capture after a session opens is not stale.
const IDLE_POLL: Duration = Duration::from_millis(500);

/// Which kinds of content the open session is willing to accept. Reading a
/// kind nobody wants is wasted work, and for images it is a lot of it.
#[derive(Clone, Copy)]
pub struct Wanted {
    pub images: bool,
    pub files: bool,
}

impl Wanted {
    /// Used when the caller asked for a read outright rather than during a
    /// poll: an explicit read should see whatever is actually there.
    fn everything() -> Self {
        Self { images: true, files: true }
    }
}

enum Request {
    /// Read the clipboard right now.
    Read(SyncSender<Option<ClipKind>>),
    /// Put a value on the clipboard.
    Write(ClipKind, SyncSender<Result<(), String>>),
    /// Put a formatted block on the clipboard, with the plain block
    /// alongside it for anything that does not want the formatting.
    WriteRich(String, String, SyncSender<Result<(), String>>),
    /// Adopt the current clipboard as the baseline without capturing it, so
    /// what is already there is not mistaken for a fresh copy.
    Rebase(SyncSender<()>),
}

#[derive(Clone)]
pub struct ClipboardHandle {
    tx: Sender<Request>,
}

impl ClipboardHandle {
    pub fn read(&self) -> Option<ClipKind> {
        let (tx, rx) = sync_channel(0);
        self.tx.send(Request::Read(tx)).ok()?;
        rx.recv().ok().flatten()
    }

    pub fn write(&self, kind: ClipKind) -> Result<(), String> {
        let (tx, rx) = sync_channel(0);
        self.tx
            .send(Request::Write(kind, tx))
            .map_err(|_| "the clipboard service is not running".to_string())?;
        rx.recv().map_err(|_| "the clipboard service stopped while writing".to_string())?
    }

    pub fn write_text(&self, text: String) -> Result<(), String> {
        self.write(ClipKind::Text { text })
    }

    /// Write a rich block. `plain` is what applications that do not want
    /// formatting will receive, and it is written whether or not the
    /// formatted flavour succeeds.
    pub fn write_rich(&self, html: String, plain: String) -> Result<(), String> {
        let (tx, rx) = sync_channel(0);
        self.tx
            .send(Request::WriteRich(html, plain, tx))
            .map_err(|_| "the clipboard service is not running".to_string())?;
        rx.recv().map_err(|_| "the clipboard service stopped while writing".to_string())?
    }

    /// Treat whatever is on the clipboard now as already seen.
    pub fn rebase(&self) {
        let (tx, rx) = sync_channel(0);
        if self.tx.send(Request::Rebase(tx)).is_ok() {
            let _ = rx.recv();
        }
    }
}

/// Start the service. The thread lives for the process.
pub fn spawn(app: AppHandle) -> ClipboardHandle {
    let (tx, rx) = channel();
    thread::Builder::new()
        .name("incredibulk-clipboard".into())
        .spawn(move || service(app, rx))
        .expect("the clipboard thread must start");
    ClipboardHandle { tx }
}

struct Backend {
    inner: Option<Clipboard>,
}

impl Backend {
    fn new() -> Self {
        Self { inner: None }
    }

    /// The handle is created lazily and re-created after a failure. On Linux a
    /// clipboard connection can be lost when the display server restarts, and
    /// that should not kill the service for the rest of the session.
    fn get(&mut self) -> Option<&mut Clipboard> {
        if self.inner.is_none() {
            match Clipboard::new() {
                Ok(c) => self.inner = Some(c),
                Err(e) => {
                    eprintln!("Incredibulk: clipboard unavailable: {e}");
                    return None;
                }
            }
        }
        self.inner.as_mut()
    }

    fn drop_handle(&mut self) {
        self.inner = None;
    }

    fn read(&mut self, want: Wanted) -> Option<ClipKind> {
        // Files come first. Copying files in Explorer also puts their names on
        // the clipboard as text in some shells, and a list of bare filenames
        // is strictly less than the paths themselves.
        if want.files
            && let Some(paths) = platform::clipboard_files()
        {
            return Some(ClipKind::Files { paths });
        }

        let cb = self.get()?;
        // Then text: it is what a session is almost always collecting, and it
        // is far cheaper to fetch than a bitmap.
        if let Ok(text) = cb.get_text()
            && !text.is_empty()
        {
            return Some(ClipKind::Text { text });
        }
        if !want.images {
            return None;
        }
        match cb.get_image() {
            Ok(img) => {
                let mut image = ImagePayload::raw(img.width, img.height, img.bytes.into_owned());
                // Encode once, here. The list needs a thumbnail and a rich
                // paste needs the PNG, and neither should be redone on
                // every redraw.
                crate::imaging::prepare(&mut image);
                Some(ClipKind::Image { image })
            }
            Err(_) => None,
        }
    }

    /// Write the formatted flavour and the plain one together.
    ///
    /// The platform layer writes the plain text first and then adds the
    /// formatted flavour, so a plain-text target is never worse off for
    /// this having been a rich paste.
    fn write_rich(&mut self, html: &str, plain: &str) -> Result<(), String> {
        let cb = match self.get() {
            Some(c) => c,
            None => return Err("the system clipboard could not be opened".into()),
        };
        let result =
            cb.set().html(html.to_string(), Some(plain.to_string())).map_err(|e| e.to_string());
        if result.is_err() {
            self.drop_handle();
        }
        result
    }

    fn write(&mut self, kind: &ClipKind) -> Result<(), String> {
        let cb = match self.get() {
            Some(c) => c,
            None => return Err("the system clipboard could not be opened".into()),
        };
        let result = match kind {
            ClipKind::Text { text } => cb.set_text(text.clone()).map_err(|e| e.to_string()),
            ClipKind::Image { image } => cb
                .set_image(ImageData {
                    width: image.width,
                    height: image.height,
                    bytes: std::borrow::Cow::Borrowed(&image.rgba),
                })
                .map_err(|e| e.to_string()),
            // Nothing puts a file list back on the clipboard yet, and pretending
            // otherwise would silently drop the paste.
            ClipKind::Files { .. } => {
                Err("file lists cannot be written back to the clipboard".into())
            }
        };
        if result.is_err() {
            self.drop_handle();
        }
        result
    }
}

fn service(app: AppHandle, rx: Receiver<Request>) {
    let mut backend = Backend::new();
    // Digest of the last value this service saw or wrote. A capture only
    // happens when the clipboard differs from this, which is what stops the
    // flushed block from being captured back into the next session.
    let mut last_digest: Option<u64> = None;
    let mut last_generation: Option<u64> = platform::clipboard_generation();

    loop {
        let (active, interval, want) = crate::actions::watch_parameters(&app);
        let wait = if active { interval } else { IDLE_POLL };

        match rx.recv_timeout(wait) {
            Ok(Request::Read(reply)) => {
                let value = backend.read(Wanted::everything());
                let _ = reply.send(value);
            }
            Ok(Request::Write(kind, reply)) => {
                let digest = kind.digest();
                let result = backend.write(&kind);
                if result.is_ok() {
                    // Our own write must never come back as a capture.
                    last_digest = Some(digest);
                    last_generation = platform::clipboard_generation();
                }
                let _ = reply.send(result);
            }
            Ok(Request::WriteRich(html, plain, reply)) => {
                let digest = ClipKind::Text { text: plain.clone() }.digest();
                let result = backend.write_rich(&html, &plain);
                if result.is_ok() {
                    last_digest = Some(digest);
                    last_generation = platform::clipboard_generation();
                }
                let _ = reply.send(result);
            }
            Ok(Request::Rebase(reply)) => {
                last_digest = backend.read(want).map(|k| k.digest());
                last_generation = platform::clipboard_generation();
                let _ = reply.send(());
            }
            Err(RecvTimeoutError::Timeout) => {
                if !active {
                    continue;
                }
                // On platforms that expose a change counter, skip the read
                // entirely when nothing has been copied. This is what keeps a
                // 180 ms poll from costing anything measurable.
                if let Some(generation) = platform::clipboard_generation() {
                    if last_generation == Some(generation) {
                        continue;
                    }
                    last_generation = Some(generation);
                }
                let Some(kind) = backend.read(want) else {
                    continue;
                };
                let digest = kind.digest();
                if last_digest == Some(digest) {
                    continue;
                }
                last_digest = Some(digest);
                crate::actions::on_clipboard_changed(&app, kind);
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}
