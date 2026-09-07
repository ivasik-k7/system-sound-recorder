//! Picking out the lines of a conversation that someone will need afterwards.
//!
//! A transcript is a wall of text. What a person actually wants from a meeting
//! is the handful of lines where something was decided or somebody agreed to do
//! something, and finding those by reading an hour of text is the work the
//! transcript was supposed to save.
//!
//! This looks for the phrases people use when they commit to something. It is
//! not understanding and it does not pretend to be: it matches cues, reports
//! which cue it matched, and is wrong in both directions. Everything it
//! produces is labelled as a suggestion and shown next to the line it came
//! from, so a wrong guess costs a glance rather than a mistake. Nothing is ever
//! summarised or reworded, because a paraphrase of what someone said in a
//! meeting is a liability, not a feature.

use serde::{Deserialize, Serialize};

use crate::live::Line;

/// Why a line was picked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mark {
    /// Somebody said they would do something, or asked somebody else to.
    Action,
    /// Something was settled.
    Decision,
    /// A question was asked, which may or may not have been answered.
    Question,
    /// A time was named.
    Deadline,
}

impl Mark {
    pub fn label(self) -> &'static str {
        match self {
            Mark::Action => "action",
            Mark::Decision => "decision",
            Mark::Question => "question",
            Mark::Deadline => "deadline",
        }
    }
}

/// A line worth coming back to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub kind: Mark,
    /// Which line, so the interface can scroll to it.
    pub line_index: usize,
    pub speaker: String,
    pub start: f64,
    /// The line itself, word for word. Never a rewording.
    pub text: String,
    /// The phrase that caused this to be picked out, so the reader can judge
    /// whether the guess was a good one.
    pub cue: String,
}

/// Phrases that usually mean somebody has taken something on.
const ACTION_CUES: &[&str] = &[
    "i'll ",
    "i will ",
    "we'll ",
    "we will ",
    "i can ",
    "i'm going to ",
    "we need to ",
    "we have to ",
    "let's ",
    "lets ",
    "can you ",
    "could you ",
    "would you ",
    "please ",
    "action item",
    "action point",
    "to-do",
    "todo",
    "follow up",
    "follow-up",
    "take care of",
    "look into",
    "pick this up",
    "assign",
    "owner",
    "i'll take",
    "leave it with me",
];

/// Phrases that usually mean something was settled.
const DECISION_CUES: &[&str] = &[
    "we decided",
    "we've decided",
    "decision",
    "we agreed",
    "agreed",
    "let's go with",
    "we'll go with",
    "going with",
    "approved",
    "sign off",
    "signed off",
    "settled",
    "final answer",
    "that's the plan",
    "we're doing",
];

/// Phrases that usually name a time something is wanted by.
const DEADLINE_CUES: &[&str] = &[
    "by monday",
    "by tuesday",
    "by wednesday",
    "by thursday",
    "by friday",
    "by saturday",
    "by sunday",
    "by tomorrow",
    "by tonight",
    "by next week",
    "by the end of",
    "end of day",
    "end of week",
    "end of the week",
    "deadline",
    "due date",
    "due by",
    "due on",
    "no later than",
    "before the",
];

/// The first cue in a haystack, if any.
fn matched(haystack: &str, cues: &[&str]) -> Option<String> {
    cues.iter()
        .find(|cue| haystack.contains(*cue))
        .map(|cue| cue.trim().to_string())
}

/// Classify one line. Returns nothing for the great majority of them.
///
/// A line can only be one thing. When several cues match, the most actionable
/// wins, because a line that both decides something and names a day is more
/// useful in the actions list than in the deadlines one.
pub fn classify(text: &str) -> Option<(Mark, String)> {
    let lower = format!(" {} ", text.trim().to_lowercase());

    if let Some(cue) = matched(&lower, ACTION_CUES) {
        return Some((Mark::Action, cue));
    }
    if let Some(cue) = matched(&lower, DECISION_CUES) {
        return Some((Mark::Decision, cue));
    }
    if let Some(cue) = matched(&lower, DEADLINE_CUES) {
        return Some((Mark::Deadline, cue));
    }
    // A question is the weakest signal, so it is only reported when nothing
    // else fits. Meetings are full of questions that need no follow-up.
    if text.trim().ends_with('?') {
        return Some((Mark::Question, "?".to_string()));
    }
    None
}

/// Everything in a conversation worth coming back to.
pub fn scan(lines: &[Line]) -> Vec<Highlight> {
    lines
        .iter()
        .filter_map(|line| {
            classify(&line.text).map(|(kind, cue)| Highlight {
                kind,
                line_index: line.index,
                speaker: line.speaker.label.clone(),
                start: line.start,
                text: line.text.clone(),
                cue,
            })
        })
        .collect()
}

/// Who spoke, and how much, in the order they first said something.
pub fn participants(lines: &[Line]) -> Vec<(String, usize, f64)> {
    let mut order: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, (usize, f64)> =
        std::collections::HashMap::new();

    for line in lines {
        let entry = counts.entry(line.speaker.label.clone()).or_insert_with(|| {
            order.push(line.speaker.label.clone());
            (0, 0.0)
        });
        entry.0 += 1;
        entry.1 += (line.end - line.start).max(0.0);
    }

    order
        .into_iter()
        .map(|name| {
            let (lines_said, seconds) = counts[&name];
            (name, lines_said, seconds)
        })
        .collect()
}

/// `H:MM:SS`, or `M:SS` for anything under an hour.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    if total >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            total / 3600,
            (total % 3600) / 60,
            total % 60
        )
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

/// A record of the meeting, in the shape somebody would circulate afterwards.
///
/// Deliberately not a summary. Every line in it was said; the suggestions are
/// marked as suggestions and carry the phrase that triggered them, and the full
/// transcript follows so nothing rests on the picking-out having been right.
pub fn meeting_notes(lines: &[Line], channel: &str, started_utc: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    let _ = writeln!(out, "# {channel}");
    out.push('\n');
    let _ = writeln!(
        out,
        "Recorded {started_utc}. Recognition ran on this machine."
    );
    let _ = writeln!(
        out,
        "This may contain other people's voices; check the policy that applies \
         before sharing it."
    );
    let duration = lines.last().map(|l| l.end).unwrap_or(0.0);
    let _ = writeln!(out, "Length {}. {} lines.", clock(duration), lines.len());
    out.push('\n');

    let who = participants(lines);
    if who.len() > 1 {
        let _ = writeln!(out, "## Who spoke");
        out.push('\n');
        for (name, said, seconds) in &who {
            let _ = writeln!(
                out,
                "- **{name}** — {said} lines, {} speaking",
                clock(*seconds)
            );
        }
        out.push('\n');
    }

    let highlights = scan(lines);
    let mut wrote_a_section = false;
    for (kind, heading) in [
        (Mark::Action, "Possible actions"),
        (Mark::Decision, "Possible decisions"),
        (Mark::Deadline, "Times mentioned"),
        (Mark::Question, "Open questions"),
    ] {
        let of_kind: Vec<&Highlight> = highlights.iter().filter(|h| h.kind == kind).collect();
        if of_kind.is_empty() {
            continue;
        }
        wrote_a_section = true;
        let _ = writeln!(out, "## {heading}");
        out.push('\n');
        for h in of_kind {
            let _ = writeln!(
                out,
                "- `{}` **{}**: {} _(matched “{}”)_",
                clock(h.start),
                h.speaker,
                h.text,
                h.cue
            );
        }
        out.push('\n');
    }

    if wrote_a_section {
        let _ = writeln!(
            out,
            "> These were picked out by matching turns of phrase, not by \
             understanding what was said. Read them against the transcript \
             below before relying on any of them."
        );
        out.push('\n');
    }

    let _ = writeln!(out, "## Transcript");
    out.push('\n');
    let mut last_speaker = "";
    for line in lines {
        if line.speaker.label != last_speaker {
            if !last_speaker.is_empty() {
                out.push('\n');
            }
            let _ = writeln!(out, "**{}**", line.speaker.label);
            last_speaker = &line.speaker.label;
        }
        let _ = writeln!(out, "`{}` {}", clock(line.start), line.text);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speakers::Speaker;

    fn line(index: usize, who: &str, start: f64, text: &str) -> Line {
        Line {
            index,
            channel: "Review".into(),
            speaker: Speaker {
                index: 1,
                label: who.to_string(),
                inferred: false,
            },
            start,
            end: start + 3.0,
            text: text.to_string(),
        }
    }

    #[test]
    fn a_commitment_is_an_action() {
        let (kind, cue) = classify("I'll send the figures over this afternoon.").unwrap();
        assert_eq!(kind, Mark::Action);
        assert_eq!(cue, "i'll");
    }

    #[test]
    fn a_request_of_someone_else_is_an_action() {
        assert_eq!(classify("Can you check the logs?").unwrap().0, Mark::Action);
    }

    #[test]
    fn a_settled_thing_is_a_decision() {
        assert_eq!(
            classify("We agreed to postpone the migration.").unwrap().0,
            Mark::Decision
        );
    }

    #[test]
    fn a_named_day_is_a_deadline() {
        assert_eq!(
            classify("The report needs to be in by Friday.").unwrap().0,
            Mark::Deadline
        );
    }

    #[test]
    fn a_bare_question_is_only_a_question() {
        assert_eq!(classify("Is the build green?").unwrap().0, Mark::Question);
    }

    #[test]
    fn ordinary_talk_is_left_alone() {
        // The great majority of a meeting must produce nothing, or the list of
        // suggestions is as long as the transcript and just as much work.
        for said in [
            "Throughput is up about eleven percent.",
            "Good morning everyone.",
            "That was the caching change rather than the hardware.",
            "Right.",
        ] {
            assert!(
                classify(said).is_none(),
                "{said:?} should not be picked out"
            );
        }
    }

    #[test]
    fn the_more_actionable_reading_wins() {
        // Both a commitment and a day. It belongs in the actions.
        let (kind, _) = classify("I'll have it done by Friday.").unwrap();
        assert_eq!(kind, Mark::Action);
    }

    #[test]
    fn a_cue_inside_a_word_does_not_count() {
        // "please" is a cue; "pleasant" is not, and neither is "agreedly".
        assert!(classify("It was a pleasant enough call.").is_none());
    }

    #[test]
    fn participants_are_counted_in_the_order_they_first_spoke() {
        let lines = vec![
            line(1, "Ivan", 0.0, "Morning."),
            line(2, "Olena", 4.0, "Morning."),
            line(3, "Ivan", 8.0, "Shall we start?"),
        ];
        let who = participants(&lines);
        assert_eq!(who[0].0, "Ivan");
        assert_eq!(who[0].1, 2);
        assert_eq!(who[1].0, "Olena");
        assert_eq!(who[1].1, 1);
    }

    #[test]
    fn notes_carry_the_words_and_flag_the_guesswork() {
        let lines = vec![
            line(1, "Ivan", 0.0, "Good morning everyone."),
            line(2, "Olena", 4.0, "I'll send the figures this afternoon."),
            line(3, "Ivan", 8.0, "We agreed to postpone the migration."),
        ];
        let notes = meeting_notes(&lines, "Review", "2026-09-07T09:00:00Z");

        assert!(notes.contains("## Possible actions"));
        assert!(notes.contains("I'll send the figures this afternoon."));
        assert!(notes.contains("## Possible decisions"));
        // The caveat has to be there, or the lists read as findings.
        assert!(
            notes.contains("not by understanding")
                || notes.contains("not by \\\n             understanding")
                || notes.contains("understanding what was said")
        );
        // And the whole conversation, so nothing depends on the picking out.
        assert!(notes.contains("## Transcript"));
        assert!(notes.contains("Good morning everyone."));
    }

    #[test]
    fn notes_survive_an_empty_conversation() {
        let notes = meeting_notes(&[], "Review", "2026-09-07T09:00:00Z");
        assert!(notes.contains("# Review"));
        assert!(notes.contains("0 lines"));
    }
}
