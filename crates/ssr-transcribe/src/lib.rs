//! Turning recorded tracks into transcripts.
//!
//! The recorder's advantage here is that it already knows which track is which.
//! A meeting captured as a Citrix track and a microphone track transcribes into
//! two transcripts that are already attributed, without any speaker separation
//! having to guess.
//!
//! Recognition itself is done by a speech engine installed on the machine. That
//! keeps recordings on the machine that made them, which matters when the audio
//! is a work call. No engine is bundled; [`engine::detect`] reports what is
//! available and what to install when nothing is.

pub mod audio;
pub mod engine;
pub mod job;
pub mod live;
pub mod notes;
pub mod resident;
pub mod sanity;
pub mod spawn;
pub mod speakers;
pub mod transcript;
pub mod vad;
pub mod whisper;

pub use engine::{detect, Engine, EngineKind};
pub use job::{transcribe_session, Progress, TranscribeRequest, TranscribeResult};
pub use live::{Line, Listener, LiveFormat, Recogniser};
pub use speakers::{Diariser, Speaker, VoicePrint};
pub use transcript::{Segment, Transcript, TranscriptFormat};

/// Where transcripts go when the caller does not say.
///
/// Absolute, and deliberately so. This used to be the relative path
/// `transcripts/`, which meant the file landed wherever the program happened to
/// have been started from. In practice that was a source checkout, so
/// transcripts of real meetings accumulated inside a git repository, one
/// `git add -A` away from being published. A transcript is the searchable
/// content of a conversation and is small enough to slip through review, so
/// where it lands cannot be left to chance.
pub fn default_transcript_dir() -> std::path::PathBuf {
    use std::path::PathBuf;

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);

    match home {
        Some(home) => {
            // Beside the recordings, under Documents, which is where the rest
            // of what this program produces already lives.
            let documents = home.join("Documents");
            let base = if documents.is_dir() { documents } else { home };
            base.join("System Sound Recorder").join("transcripts")
        }
        // No home directory to speak of. The system temporary directory is a
        // poor place for a transcript, but it is at least not a repository.
        None => std::env::temp_dir().join("system-sound-recorder-transcripts"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_default_transcript_directory_is_absolute() {
        // The whole point of it. A relative path here means a transcript lands
        // wherever the program was started from, which has already put real
        // conversations inside a source checkout once.
        let dir = super::default_transcript_dir();
        assert!(dir.is_absolute(), "{} is not absolute", dir.display());
    }

    #[test]
    fn the_default_transcript_directory_is_not_the_working_directory() {
        let dir = super::default_transcript_dir();
        let here = std::env::current_dir().unwrap();
        assert!(
            !dir.starts_with(&here),
            "{} sits inside the working directory",
            dir.display()
        );
    }
}
