//! Session timeline: turning independently clocked capture streams into a set
//! of files that line up with each other.

pub mod manifest;
pub mod track;
pub mod wav;

pub use manifest::{sanitize_filename, SessionManifest, TrackManifest};
pub use track::{Sink, TimedBlock, Track, TrackStats};
pub use wav::WavWriter;
