//! How much room is left to record into.
//!
//! Multitrack float audio is large and predictable: one stereo track at 48 kHz
//! is about 23 megabytes a minute, so four tracks fill a gigabyte in about
//! eleven minutes. A recorder that discovers this only when the write fails has
//! already lost the recording, so the space is checked before starting and
//! watched while running.

use std::path::Path;

use serde::Serialize;

/// Free and total space on the volume holding a path.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpace {
    /// Bytes available to this user, which is what actually matters: a quota
    /// can make it smaller than the volume's own free space.
    pub free_bytes: u64,
    pub total_bytes: u64,
}

impl DiskSpace {
    /// Seconds of recording the free space allows, for the given format.
    ///
    /// Float audio has a fixed byte rate, so this is arithmetic rather than an
    /// estimate. Zero when the parameters do not describe a real stream.
    pub fn seconds_remaining(&self, sample_rate: u32, channels: u16, tracks: usize) -> f64 {
        let per_second = sample_rate as f64 * channels as f64 * 4.0 * tracks as f64;
        if per_second <= 0.0 {
            return 0.0;
        }
        self.free_bytes as f64 / per_second
    }
}

/// Read the free space on the volume holding `path`.
///
/// The path does not have to exist: the nearest existing ancestor is used, so
/// this works before the session folder has been created, which is exactly when
/// the answer is wanted.
pub fn space_for(path: &Path) -> Option<DiskSpace> {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut probe = path;
    loop {
        if probe.exists() {
            break;
        }
        probe = probe.parent()?;
    }

    let (_buf, wide) = crate::device::wide(&probe.display().to_string());
    let mut free_to_caller = 0u64;
    let mut total = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide,
            Some(&mut free_to_caller),
            Some(&mut total),
            // The third value is the volume's free space ignoring quotas. The
            // caller's own allowance is the one that limits a recording.
            None,
        )
    };
    ok.ok()?;
    Some(DiskSpace {
        free_bytes: free_to_caller,
        total_bytes: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gigabyte_holds_about_eleven_minutes_of_four_stereo_tracks() {
        let space = DiskSpace {
            free_bytes: 1_000_000_000,
            total_bytes: 2_000_000_000,
        };
        let secs = space.seconds_remaining(48_000, 2, 4);
        assert!((secs - 651.0).abs() < 2.0, "got {secs}");
    }

    #[test]
    fn a_session_with_no_tracks_has_no_time_rather_than_infinite_time() {
        let space = DiskSpace {
            free_bytes: 1_000_000_000,
            total_bytes: 1_000_000_000,
        };
        assert_eq!(space.seconds_remaining(48_000, 2, 0), 0.0);
        assert_eq!(space.seconds_remaining(0, 2, 1), 0.0);
    }

    #[test]
    fn the_volume_holding_the_temp_directory_can_be_read() {
        // The one part of this that needs Windows, kept to a directory that
        // always exists.
        let space = space_for(&std::env::temp_dir()).expect("temp directory has a volume");
        assert!(space.total_bytes > 0);
        assert!(space.free_bytes <= space.total_bytes);
    }

    #[test]
    fn a_path_that_does_not_exist_yet_still_resolves_to_its_volume() {
        let future = std::env::temp_dir().join("ssr-not-created-yet/nested/deeper");
        let space = space_for(&future).expect("an unwritten path still has a volume");
        assert!(space.total_bytes > 0);
    }
}
