//! What the recorder knows about the applications it might be pointed at.
//!
//! Discovery finds executables; this turns an executable name into something a
//! person recognises. `wfica32.exe` is not a useful thing to show in a list of
//! things to record, and neither is `msrdc.exe`, but "Citrix HDX session" and
//! "WSL / WSLg" are — and the difference matters most for exactly the two
//! processes this tool exists for.
//!
//! The catalogue is a hint, never a filter. Anything making sound can be
//! recorded whether it appears here or not; an unknown executable is presented
//! under its own name with the category the recorder could infer.

use serde::Serialize;

/// Broad kind of application, used for grouping and for choosing an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// A remote session whose audio is redirected to this machine. The reason
    /// this tool exists.
    Remote,
    /// A meeting or telephony client.
    Conferencing,
    /// A web browser, which is where a great deal of audio actually comes from.
    Browser,
    /// A chat client that also makes calls.
    Chat,
    /// A media player.
    Media,
    /// A music or audio production application.
    Audio,
    /// A game.
    Game,
    /// Windows itself: notification sounds, accessibility, the shell.
    System,
    /// Anything the catalogue does not recognise.
    Other,
}

impl Category {
    /// A short plural name for a group heading.
    pub fn group_name(self) -> &'static str {
        match self {
            Category::Remote => "Remote sessions",
            Category::Conferencing => "Meetings and calls",
            Category::Browser => "Browsers",
            Category::Chat => "Chat",
            Category::Media => "Media players",
            Category::Audio => "Audio applications",
            Category::Game => "Games",
            Category::System => "System",
            Category::Other => "Other applications",
        }
    }
}

/// A catalogue entry: one executable the recorder can name.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// Executable name including extension, matched case-insensitively.
    pub exe: &'static str,
    /// Name to show instead of the executable.
    pub label: &'static str,
    pub category: Category,
    /// Offer this one first when it is running.
    ///
    /// Set for the sources this tool was built to separate: a remote session
    /// carries audio that exists nowhere else on the machine, so if it is
    /// running it is almost certainly what the user came here for.
    pub recommended: bool,
}

const fn e(exe: &'static str, label: &'static str, category: Category) -> Entry {
    Entry {
        exe,
        label,
        category,
        recommended: false,
    }
}

const fn r(exe: &'static str, label: &'static str, category: Category) -> Entry {
    Entry {
        exe,
        label,
        category,
        recommended: true,
    }
}

/// Everything the recorder can put a name to.
pub const CATALOG: &[Entry] = &[
    // Remote sessions. The whole point of the tool: audio that originates
    // somewhere else and passes through this machine on its way to the
    // speakers, where it can be intercepted and nowhere else.
    r("wfica32.exe", "Citrix HDX session", Category::Remote),
    r("CDViewer.exe", "Citrix Workspace viewer", Category::Remote),
    e(
        "SelfServicePlugin.exe",
        "Citrix Workspace",
        Category::Remote,
    ),
    r("msrdc.exe", "WSL / WSLg", Category::Remote),
    e("mstsc.exe", "Remote Desktop", Category::Remote),
    e("vmware-view.exe", "VMware Horizon", Category::Remote),
    e("Anydesk.exe", "AnyDesk", Category::Remote),
    e("TeamViewer.exe", "TeamViewer", Category::Remote),
    e("parsecd.exe", "Parsec", Category::Remote),
    // Meetings and calls. Teams has a second executable that exists only
    // inside a VDI session and carries the optimised media path, which is
    // exactly the audio a remote-session recording is after.
    r("ms-teams.exe", "Microsoft Teams", Category::Conferencing),
    r("MsTeamsVdi.exe", "Teams (VDI optimised)", Category::Remote),
    e(
        "msTeamsPluginHost.exe",
        "Teams VDI plug-in host",
        Category::Remote,
    ),
    e(
        "Teams.exe",
        "Microsoft Teams (classic)",
        Category::Conferencing,
    ),
    r("Zoom.exe", "Zoom", Category::Conferencing),
    e("Webex.exe", "Webex", Category::Conferencing),
    e("CiscoCollabHost.exe", "Webex", Category::Conferencing),
    e("lync.exe", "Skype for Business", Category::Conferencing),
    e("Skype.exe", "Skype", Category::Conferencing),
    e("GoTo.exe", "GoTo Meeting", Category::Conferencing),
    e("bluejeans.exe", "BlueJeans", Category::Conferencing),
    // Browsers.
    e("chrome.exe", "Google Chrome", Category::Browser),
    e("msedge.exe", "Microsoft Edge", Category::Browser),
    e("firefox.exe", "Firefox", Category::Browser),
    e("brave.exe", "Brave", Category::Browser),
    e("opera.exe", "Opera", Category::Browser),
    e("vivaldi.exe", "Vivaldi", Category::Browser),
    e("arc.exe", "Arc", Category::Browser),
    e("comet.exe", "Comet", Category::Browser),
    // Embedded browsers. A great deal of desktop application audio is really
    // a web view, and it surfaces under the host process rather than the
    // application the user thinks they are listening to.
    e("msedgewebview2.exe", "Edge WebView2", Category::Browser),
    e("WebView2Loader.exe", "Edge WebView2", Category::Browser),
    e("electron.exe", "Electron application", Category::Browser),
    // Chat.
    e("slack.exe", "Slack", Category::Chat),
    e("Discord.exe", "Discord", Category::Chat),
    e("Telegram.exe", "Telegram", Category::Chat),
    e("WhatsApp.exe", "WhatsApp", Category::Chat),
    e("Signal.exe", "Signal", Category::Chat),
    // Media.
    e("vlc.exe", "VLC", Category::Media),
    e("Spotify.exe", "Spotify", Category::Media),
    e("mpv.exe", "mpv", Category::Media),
    e("wmplayer.exe", "Windows Media Player", Category::Media),
    e("Music.UI.exe", "Media Player", Category::Media),
    e("PotPlayerMini64.exe", "PotPlayer", Category::Media),
    e("foobar2000.exe", "foobar2000", Category::Media),
    e("iTunes.exe", "iTunes", Category::Media),
    // Audio production.
    e("Audacity.exe", "Audacity", Category::Audio),
    e("Reaper.exe", "REAPER", Category::Audio),
    e("Ableton Live.exe", "Ableton Live", Category::Audio),
    e("obs64.exe", "OBS Studio", Category::Audio),
    e("obs32.exe", "OBS Studio", Category::Audio),
    // System.
    e("audiodg.exe", "Windows audio engine", Category::System),
    e("explorer.exe", "Windows shell", Category::System),
    e("Narrator.exe", "Narrator", Category::System),
    e("SystemSettings.exe", "Settings", Category::System),
];

/// Look up an executable name in the catalogue.
pub fn lookup(exe: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|c| c.exe.eq_ignore_ascii_case(exe))
}

/// The label to show for an executable, catalogued or not.
///
/// An unknown executable gets its own name with the extension removed, which
/// reads better than the raw filename and is still unambiguous.
pub fn label_for(exe: &str) -> String {
    if let Some(entry) = lookup(exe) {
        return entry.label.to_string();
    }
    let stem = exe
        .strip_suffix(".exe")
        .or_else(|| exe.strip_suffix(".EXE"))
        .unwrap_or(exe);
    if stem.is_empty() {
        return exe.to_string();
    }
    // Capitalise a lowercase name, since most unknown executables are written
    // in lower case and a list of them otherwise looks unfinished.
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) if first.is_lowercase() => {
            first.to_uppercase().collect::<String>() + chars.as_str()
        }
        _ => stem.to_string(),
    }
}

/// The category for an executable, guessed from the name when it is not
/// catalogued.
pub fn category_for(exe: &str) -> Category {
    if let Some(entry) = lookup(exe) {
        return entry.category;
    }
    let lower = exe.to_lowercase();
    // A weak guess, but a useful one: these substrings are load-bearing in
    // practice and a wrong group heading costs nothing.
    for (needle, category) in [
        ("chrome", Category::Browser),
        ("browser", Category::Browser),
        ("edge", Category::Browser),
        ("firefox", Category::Browser),
        ("teams", Category::Conferencing),
        ("meet", Category::Conferencing),
        ("zoom", Category::Conferencing),
        ("phone", Category::Conferencing),
        ("citrix", Category::Remote),
        ("wfica", Category::Remote),
        ("vmware", Category::Remote),
        ("rdp", Category::Remote),
        ("player", Category::Media),
        ("music", Category::Media),
        ("audio", Category::Audio),
    ] {
        if lower.contains(needle) {
            return category;
        }
    }
    Category::Other
}

/// Whether an executable is one of the sources worth offering first.
pub fn is_recommended(exe: &str) -> bool {
    lookup(exe).is_some_and(|e| e.recommended)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_sources_this_tool_exists_for_are_named_and_recommended() {
        assert_eq!(label_for("wfica32.exe"), "Citrix HDX session");
        assert_eq!(label_for("msrdc.exe"), "WSL / WSLg");
        assert!(is_recommended("wfica32.exe"));
        assert!(is_recommended("msrdc.exe"));
        assert_eq!(category_for("wfica32.exe"), Category::Remote);
    }

    #[test]
    fn matching_ignores_case_because_windows_does() {
        assert_eq!(label_for("WFICA32.EXE"), "Citrix HDX session");
        assert_eq!(label_for("Chrome.Exe"), "Google Chrome");
    }

    #[test]
    fn an_unknown_executable_is_still_presentable() {
        assert_eq!(label_for("someapp.exe"), "Someapp");
        assert_eq!(label_for("SomeApp.exe"), "SomeApp");
        assert_eq!(category_for("someapp.exe"), Category::Other);
        assert!(!is_recommended("someapp.exe"));
    }

    #[test]
    fn an_uncatalogued_name_can_still_be_grouped_by_a_substring() {
        assert_eq!(category_for("chrome_proxy.exe"), Category::Browser);
        assert_eq!(category_for("citrixhelper.exe"), Category::Remote);
    }

    #[test]
    fn no_executable_is_catalogued_twice() {
        let mut seen: Vec<String> = Vec::new();
        for entry in CATALOG {
            let lower = entry.exe.to_lowercase();
            assert!(!seen.contains(&lower), "{} appears twice", entry.exe);
            seen.push(lower);
        }
    }
}
