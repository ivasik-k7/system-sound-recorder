//! Making sure nothing outlives the program.
//!
//! Transcription spawns child processes: a resident Python worker that holds a
//! speech model in memory, and short-lived engine runs. Killing them in `Drop`
//! covers the polite exit and nothing else. A crash, a kill from Task Manager,
//! or a power-loss of the parent leaves the worker running with a model
//! resident — hundreds of megabytes, sometimes gigabytes, with nothing left to
//! reap it and no window to notice it by.
//!
//! Windows has one mechanism that survives all of those: a job object. A
//! process placed in a job whose limits include `KILL_ON_JOB_CLOSE` is killed
//! when the last handle to that job closes, and the operating system closes
//! handles however the process dies. Children inherit the job, so putting *this
//! process* in one at startup means every child it will ever spawn is covered
//! by the same guarantee, with nothing to remember at each spawn site.

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::GetCurrentProcess;

/// Holds the job open for the life of the program.
///
/// Dropping it closes the handle, which is what kills the job's processes — so
/// it is kept alive deliberately rather than being a guard anyone should scope.
pub struct KillOnExit {
    job: HANDLE,
}

impl KillOnExit {
    /// Put this process, and therefore its children, in a job that dies with it.
    ///
    /// Best effort. A machine that refuses — an environment that has already
    /// placed the process in a job that forbids nesting, most likely — gets the
    /// old behaviour rather than a failure to start, because an orphaned
    /// worker is a worse outcome than no recorder only if the recorder runs.
    pub fn install() -> Option<Self> {
        unsafe {
            let job = CreateJobObjectW(None, None).ok()?;

            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if set.is_err() {
                let _ = CloseHandle(job);
                return None;
            }

            if AssignProcessToJobObject(job, GetCurrentProcess()).is_err() {
                let _ = CloseHandle(job);
                return None;
            }

            crate::trace!("child processes will be killed with this one");
            Some(KillOnExit { job })
        }
    }
}

impl Drop for KillOnExit {
    fn drop(&mut self) {
        // Closing the last handle is what triggers the kill, which is the
        // intended behaviour at exit.
        unsafe {
            let _ = CloseHandle(self.job);
        }
    }
}
