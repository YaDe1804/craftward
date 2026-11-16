//! Last-change undo offers for desktop (and other) callers.
//!
//! An offer remembers the bytes from before a successful write and the
//! fingerprint of the bytes that were written. Undo restores the previous
//! bytes through the same fingerprint check used for edits. A later change
//! replaces the offer. The visible window is eight seconds on the injected
//! clock.

use std::path::PathBuf;

use crate::edit::{
    ContentFingerprint, EditError, remove_file_if_unchanged, restore_file_if_unchanged,
};

/// How long an undo offer stays actionable, in seconds.
pub const UNDO_VISIBLE_SECS: u64 = 8;

/// One file to restore when the offer is undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoSnapshot {
    /// Path that was written.
    pub path: PathBuf,
    /// Bytes from before the change. [`None`] means the file did not exist
    /// (undo deletes it when the post-change fingerprint still matches).
    pub previous_bytes: Option<Vec<u8>>,
    /// Fingerprint of the bytes after the change.
    pub expected: ContentFingerprint,
}

/// A single undoable change shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoOffer {
    /// Short status line, for example `Status of Ship set to In progress`.
    pub message: String,
    /// Files to restore, in order.
    pub snapshots: Vec<UndoSnapshot>,
    /// Absolute clock seconds when the offer expires.
    pub expires_at_secs: u64,
}

impl UndoOffer {
    /// Builds an offer that expires [`UNDO_VISIBLE_SECS`] after `now_secs`.
    #[must_use]
    pub fn new(message: impl Into<String>, snapshots: Vec<UndoSnapshot>, now_secs: u64) -> Self {
        Self {
            message: message.into(),
            snapshots,
            expires_at_secs: now_secs.saturating_add(UNDO_VISIBLE_SECS),
        }
    }

    /// Whether `now_secs` is at or past the expiry instant.
    #[must_use]
    pub const fn is_expired(&self, now_secs: u64) -> bool {
        now_secs >= self.expires_at_secs
    }

    /// Restores every snapshot when none has expired and none has drifted.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::Conflict`] when a file changed outside the offer,
    /// or a load/write error from the restore path.
    pub fn undo(&self) -> Result<(), EditError> {
        for snapshot in &self.snapshots {
            match &snapshot.previous_bytes {
                Some(bytes) => {
                    restore_file_if_unchanged(&snapshot.path, &snapshot.expected, bytes)?;
                }
                None => {
                    remove_file_if_unchanged(&snapshot.path, &snapshot.expected)?;
                }
            }
        }
        Ok(())
    }
}

/// Holds at most one live undo offer.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UndoState {
    offer: Option<UndoOffer>,
}

impl UndoState {
    /// Empty state with no offer.
    #[must_use]
    pub const fn new() -> Self {
        Self { offer: None }
    }

    /// Replaces any previous offer with `offer`.
    pub fn push(&mut self, offer: UndoOffer) {
        self.offer = Some(offer);
    }

    /// Clears the offer when it has expired on `now_secs`.
    pub fn tick(&mut self, now_secs: u64) {
        if self
            .offer
            .as_ref()
            .is_some_and(|offer| offer.is_expired(now_secs))
        {
            self.offer = None;
        }
    }

    /// The live offer, if any and not expired.
    #[must_use]
    pub fn current(&self, now_secs: u64) -> Option<&UndoOffer> {
        self.offer
            .as_ref()
            .filter(|offer| !offer.is_expired(now_secs))
    }

    /// Dismisses the offer without restoring.
    pub fn dismiss(&mut self) {
        self.offer = None;
    }

    /// Runs undo when an unexpired offer exists, then clears it on success.
    ///
    /// # Errors
    ///
    /// Returns the restore error. The offer stays so the UI can show the conflict.
    pub fn undo(&mut self, now_secs: u64) -> Result<(), EditError> {
        let Some(offer) = self.offer.clone() else {
            return Ok(());
        };
        if offer.is_expired(now_secs) {
            self.offer = None;
            return Ok(());
        }
        offer.undo()?;
        self.offer = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{UndoOffer, UndoSnapshot, UndoState};
    use crate::edit::{content_fingerprint, replace_file_if_unchanged};

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "craftward-undo-{}-{}",
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn undo_restores_previous_bytes() {
        let root = scratch("restore");
        let path = root.join("file.json");
        fs::write(&path, b"before\n").unwrap();
        let before = content_fingerprint(b"before\n");
        let after = replace_file_if_unchanged(&path, &before, b"after\n").unwrap();
        let mut state = UndoState::new();
        state.push(UndoOffer::new(
            "changed file",
            vec![UndoSnapshot {
                path: path.clone(),
                previous_bytes: Some(b"before\n".to_vec()),
                expected: after,
            }],
            100,
        ));
        state.undo(101).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"before\n");
        assert!(state.current(102).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn offer_expires_after_eight_seconds() {
        let offer = UndoOffer::new("msg", Vec::new(), 50);
        assert!(!offer.is_expired(57));
        assert!(offer.is_expired(58));
        let mut state = UndoState::new();
        state.push(offer);
        state.tick(57);
        assert!(state.current(57).is_some());
        state.tick(58);
        assert!(state.current(58).is_none());
    }

    #[test]
    fn conflict_when_file_changed_outside() {
        let root = scratch("conflict");
        let path = root.join("file.json");
        fs::write(&path, b"before\n").unwrap();
        let before = content_fingerprint(b"before\n");
        let after = replace_file_if_unchanged(&path, &before, b"after\n").unwrap();
        fs::write(&path, b"external\n").unwrap();
        let mut state = UndoState::new();
        state.push(UndoOffer::new(
            "changed file",
            vec![UndoSnapshot {
                path: path.clone(),
                previous_bytes: Some(b"before\n".to_vec()),
                expected: after,
            }],
            10,
        ));
        assert!(state.undo(11).unwrap_err().is_conflict());
        assert!(state.current(11).is_some());
        assert_eq!(fs::read(&path).unwrap(), b"external\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn new_offer_replaces_the_previous() {
        let mut state = UndoState::new();
        state.push(UndoOffer::new("first", Vec::new(), 1));
        state.push(UndoOffer::new("second", Vec::new(), 2));
        assert_eq!(state.current(3).unwrap().message, "second");
    }
}
