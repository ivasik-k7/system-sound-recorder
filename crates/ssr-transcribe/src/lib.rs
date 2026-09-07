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
