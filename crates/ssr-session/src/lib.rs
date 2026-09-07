//! Recording session orchestration.
//!
//! Everything above the capture engine and below the user interface lives here,
//! so that the command line tool and the desktop application drive recording
//! through exactly the same code rather than each growing their own version of
//! it that drifts.

#![cfg(windows)]

pub mod config;
pub mod library;
pub mod recorder;
pub mod status;

pub use config::{OutputTarget, SessionConfig, SourceRequest};
pub use library::{list_sessions, LibrarySession, LibraryTrack};
pub use recorder::Recorder;
pub use status::{RecorderState, SessionSnapshot, TrackSnapshot};

pub use ssr_timeline::manifest::SessionManifest;
