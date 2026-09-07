//! Finding a speech engine on the machine.
//!
//! Nothing is bundled. Speech models are large, licensing them is awkward, and
//! a recording of a work call should not have to leave the machine to be read.
//! So the recorder uses an engine the user installed, and says clearly what to
//! install when it cannot find one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Which family of engine an executable belongs to.
///
/// They differ in how they are invoked and what they emit, so the distinction
/// has to survive discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineKind {
    /// whisper.cpp, which runs on the processor with no Python involved and is
    /// the one worth recommending.
    WhisperCpp,
    /// The reference Python implementation, or faster-whisper, which share a
    /// command line and an output format.
    WhisperPython,
}

impl EngineKind {
    pub fn label(self) -> &'static str {
        match self {
            EngineKind::WhisperCpp => "whisper.cpp",
            EngineKind::WhisperPython => "whisper (Python)",
        }
    }

    /// Whether this engine needs a model file passed to it explicitly.
    ///
    /// whisper.cpp takes a path to a `.bin`; the Python one downloads and
    /// caches models by name on first use.
    pub fn needs_model_file(self) -> bool {
        matches!(self, EngineKind::WhisperCpp)
    }
}

/// A usable speech engine that was found.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    pub kind: EngineKind,
    /// Name to show in a chooser.
    pub label: String,
    /// The executable to run.
    pub executable: PathBuf,
    /// Model files found nearby, for engines that need one named.
    pub models: Vec<PathBuf>,
}

impl Engine {
    /// The model to use when the caller did not choose one.
    ///
    /// Prefers the largest available, since transcription is usually run after
    /// the fact where accuracy matters more than speed.
    pub fn default_model(&self) -> Option<&Path> {
        const PREFERENCE: [&str; 6] = ["large", "medium", "small", "base", "tiny", ""];
        for want in PREFERENCE {
            if let Some(found) = self.models.iter().find(|m| {
                m.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.to_lowercase().contains(want))
                    .unwrap_or(false)
            }) {
                return Some(found.as_path());
            }
        }
        self.models.first().map(|p| p.as_path())
    }

    /// Whether this engine can actually run as configured.
    pub fn is_usable(&self) -> bool {
        !self.kind.needs_model_file() || !self.models.is_empty()
    }
}

/// Executable names to look for, in order of preference.
const WHISPER_CPP_NAMES: [&str; 4] = ["whisper-cli", "whisper-cpp", "whisper.cpp", "main"];
const WHISPER_PY_NAMES: [&str; 2] = ["whisper", "faster-whisper"];

/// Directories worth searching besides those on `PATH`.
fn extra_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(explicit) = std::env::var("SSR_WHISPER_DIR") {
        dirs.push(PathBuf::from(explicit));
    }
    for base in [
        "LOCALAPPDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "USERPROFILE",
    ] {
        if let Ok(root) = std::env::var(base) {
            let root = PathBuf::from(root);
            dirs.push(root.join("whisper.cpp"));
            dirs.push(
                root.join("whisper.cpp")
                    .join("build")
                    .join("bin")
                    .join("Release"),
            );
            dirs.push(root.join("whisper"));
            dirs.push(root.join("Programs").join("whisper.cpp"));
        }
    }
    dirs
}

/// Find an executable by name on `PATH` or in the extra directories.
fn find_executable(names: &[&str]) -> Option<PathBuf> {
    let suffixes: &[&str] = if cfg!(windows) { &[".exe", ""] } else { &[""] };

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        roots.extend(std::env::split_paths(&path));
    }
    roots.extend(extra_directories());

    for name in names {
        for root in &roots {
            for suffix in suffixes {
                let candidate = root.join(format!("{name}{suffix}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Model files sitting near an executable, or in the usual model directories.
fn find_models(executable: &Path) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(explicit) = std::env::var("SSR_WHISPER_MODELS") {
        roots.push(PathBuf::from(explicit));
    }
    if let Some(dir) = executable.parent() {
        roots.push(dir.to_path_buf());
        roots.push(dir.join("models"));
        // A build tree puts the executable several levels below the models.
        for up in [1, 2, 3] {
            let mut d = dir.to_path_buf();
            for _ in 0..up {
                d = match d.parent() {
                    Some(p) => p.to_path_buf(),
                    None => break,
                };
            }
            roots.push(d.join("models"));
        }
    }

    let mut found = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_model = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| {
                    let n = n.to_lowercase();
                    n.ends_with(".bin") && n.contains("ggml")
                })
                .unwrap_or(false);
            if is_model && !found.contains(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Every speech engine that could be found, best first.
pub fn detect() -> Vec<Engine> {
    let mut engines = Vec::new();

    if let Some(executable) = find_executable(&WHISPER_CPP_NAMES) {
        let models = find_models(&executable);
        engines.push(Engine {
            kind: EngineKind::WhisperCpp,
            label: EngineKind::WhisperCpp.label().to_string(),
            executable,
            models,
        });
    }

    if let Some(executable) = find_executable(&WHISPER_PY_NAMES) {
        engines.push(Engine {
            kind: EngineKind::WhisperPython,
            label: EngineKind::WhisperPython.label().to_string(),
            executable,
            models: Vec::new(),
        });
    }

    // An engine that cannot run without a model it does not have is still worth
    // reporting, so the interface can say what is missing rather than claiming
    // nothing is installed.
    engines.sort_by_key(|e| !e.is_usable());
    engines
}

/// What to tell someone who has no engine installed.
///
/// Deliberately specific. "Install whisper" is not actionable; a command that
/// can be pasted is.
pub fn installation_advice() -> &'static str {
    "No speech engine was found on this machine.\n\n\
     The recommended one is whisper.cpp, which runs locally and keeps recordings \
     on this machine:\n\n\
     \x20 winget install ggerganov.whisper.cpp\n\n\
     Then download a model, for example:\n\n\
     \x20 curl -L -o ggml-base.bin \\\n\
     \x20   https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin\n\n\
     Point the recorder at them with SSR_WHISPER_DIR and SSR_WHISPER_MODELS if \
     they are not on PATH.\n\n\
     The Python implementation also works if it is already installed:\n\n\
     \x20 pip install -U openai-whisper"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_with(models: &[&str]) -> Engine {
        Engine {
            kind: EngineKind::WhisperCpp,
            label: "whisper.cpp".into(),
            executable: PathBuf::from("whisper-cli"),
            models: models.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn the_largest_model_available_is_preferred() {
        let e = engine_with(&["ggml-tiny.bin", "ggml-medium.bin", "ggml-base.bin"]);
        assert_eq!(e.default_model().unwrap(), Path::new("ggml-medium.bin"));
    }

    #[test]
    fn any_model_is_better_than_none() {
        let e = engine_with(&["ggml-something-unusual.bin"]);
        assert!(e.default_model().is_some());
    }

    #[test]
    fn an_engine_without_a_model_it_needs_is_not_usable() {
        assert!(!engine_with(&[]).is_usable());
        assert!(engine_with(&["ggml-base.bin"]).is_usable());
    }

    #[test]
    fn the_python_engine_needs_no_model_file() {
        let e = Engine {
            kind: EngineKind::WhisperPython,
            label: "whisper".into(),
            executable: PathBuf::from("whisper"),
            models: Vec::new(),
        };
        assert!(e.is_usable());
    }

    #[test]
    fn the_advice_names_something_runnable() {
        let advice = installation_advice();
        assert!(advice.contains("winget install"));
        assert!(advice.contains("SSR_WHISPER_MODELS"));
    }
}
