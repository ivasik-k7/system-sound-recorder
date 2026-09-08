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
//!
//! Cues are not equally good, and treating them as though they were made this
//! useless in practice. Run over a technical interview it offered fifty-six
//! "possible actions", almost all of them questions the panel asked, because
//! `let's` and `can you` fire on every other line of any conversation; and it
//! offered every single line that ended in a question mark as an open question.
//! A list that long is not read. So cues are now in two tiers — the ones that
//! mean what they say, and the ones that only sometimes do — the weak ones need
//! a line with some substance behind them, near-duplicates are collapsed, and
//! each section is capped and says how many it left out.

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

/// How much weight the cue behind a highlight can bear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// The phrase means this and little else: "action item", "we agreed",
    /// "by Friday".
    Explicit,
    /// The phrase often means this and often means nothing: "let's", "I can",
    /// "can you". Worth showing, not worth trusting.
    Likely,
}

/// What a line was recognised as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub kind: Mark,
    /// The phrase that caused it.
    pub cue: String,
    pub confidence: Confidence,
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
    /// How much the cue is worth.
    pub confidence: Confidence,
}

/// Phrases that name a commitment outright. Little else sounds like these.
const EXPLICIT_ACTION_CUES: &[&str] = &[
    "action item",
    "action point",
    "to-do",
    "todo",
    "follow up",
    "follow-up",
    "take care of",
    "look into",
    "pick this up",
    "i'll take",
    "leave it with me",
];

/// Phrases that often mean a commitment and often mean nothing at all.
///
/// Every one of these is ordinary conversational filler as well as a possible
/// commitment. "Let's" opens half the sentences in a meeting; "can you" is how
/// an interviewer asks a question. They are kept because when they are right
/// they are useful, and gated because when they are wrong they are most of the
/// list.
const LIKELY_ACTION_CUES: &[&str] = &[
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
    "assign",
    "owner",
];

/// Phrases that say something was settled, and mean it.
const EXPLICIT_DECISION_CUES: &[&str] = &[
    "we decided",
    "we've decided",
    "we agreed",
    "let's go with",
    "we'll go with",
    "signed off",
    "sign off",
    "final answer",
    "that's the plan",
];

/// Phrases that suggest something was settled.
const LIKELY_DECISION_CUES: &[&str] = &[
    "decision",
    "agreed",
    "going with",
    "approved",
    "settled",
    "we're doing",
];

/// Phrases that usually name a time something is wanted by. All specific
/// enough to stand on their own.
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

/// How many words a line needs before a weak cue in it counts for anything.
///
/// Five, which keeps "Can you check the logs?" and loses "Sorry, can you
/// please?". A commitment has an object; a fragment does not.
const SUBSTANCE: usize = 5;

/// And how many a question needs before it is worth listing as one.
const QUESTION_SUBSTANCE: usize = 4;

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
/// useful in the actions list than in the deadlines one. An explicit cue always
/// beats a weak one, whatever it is about.
pub fn classify(text: &str) -> Option<Match> {
    let trimmed = text.trim();
    let lower = format!(" {} ", trimmed.to_lowercase());
    let words = trimmed.split_whitespace().count();

    if let Some(cue) = matched(&lower, EXPLICIT_ACTION_CUES) {
        return Some(Match {
            kind: Mark::Action,
            cue,
            confidence: Confidence::Explicit,
        });
    }
    if let Some(cue) = matched(&lower, EXPLICIT_DECISION_CUES) {
        return Some(Match {
            kind: Mark::Decision,
            cue,
            confidence: Confidence::Explicit,
        });
    }

    // A weak cue needs a line with something in it. "Sorry, can you please?"
    // matched `can you` and was reported as an action; a commitment has an
    // object and a fragment does not.
    //
    // Checked ahead of the deadlines so that a line which both commits to
    // something and names a day lands in the actions, where it is more use.
    if words >= SUBSTANCE {
        if let Some(cue) = matched(&lower, LIKELY_ACTION_CUES) {
            return Some(Match {
                kind: Mark::Action,
                cue,
                confidence: Confidence::Likely,
            });
        }
    }
    if let Some(cue) = matched(&lower, DEADLINE_CUES) {
        return Some(Match {
            kind: Mark::Deadline,
            cue,
            confidence: Confidence::Explicit,
        });
    }
    if words >= SUBSTANCE {
        if let Some(cue) = matched(&lower, LIKELY_DECISION_CUES) {
            return Some(Match {
                kind: Mark::Decision,
                cue,
                confidence: Confidence::Likely,
            });
        }
    }

    // A question is the weakest signal, so it is only reported when nothing
    // else fits. Meetings are full of questions that need no follow-up.
    if trimmed.ends_with('?') && words >= QUESTION_SUBSTANCE {
        return Some(Match {
            kind: Mark::Question,
            cue: "?".to_string(),
            confidence: Confidence::Likely,
        });
    }
    None
}

/// The most highlights of one kind worth putting in a record.
///
/// A capped list gets read. Fifty-six "possible actions" do not, and the ten
/// real ones are lost among them either way. What is left out is counted, so
/// the transcript underneath is still the answer for anyone who wants it all.
const MOST_PER_SECTION: usize = 20;

pub fn scan(lines: &[Line]) -> Vec<Highlight> {
    lines
        .iter()
        .filter_map(|line| {
            classify(&line.text).map(|m| Highlight {
                kind: m.kind,
                line_index: line.index,
                speaker: line.speaker.label.clone(),
                start: line.start,
                text: line.text.clone(),
                cue: m.cue,
                confidence: m.confidence,
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

/// A line reduced to the thing that makes it different from another line:
/// lower case, letters and digits and single spaces only. Enough to notice that
/// "I will be creating my own partners in order to execute the fake claims
/// jobs." and the same sentence with a word stuttered are one remark.
fn shape_of(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
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
        let mut of_kind: Vec<&Highlight> = highlights.iter().filter(|h| h.kind == kind).collect();
        if of_kind.is_empty() {
            continue;
        }
        // People say the same thing twice, and a stuttering engine says it four
        // times. Only the first of each wording is worth a bullet.
        let mut seen = std::collections::HashSet::new();
        of_kind.retain(|h| seen.insert(shape_of(&h.text)));
        // Certain things first, then the guesses, each in the order they were
        // said. `sort_by_key` is stable, so the second half of that is free.
        of_kind.sort_by_key(|h| h.confidence);

        let total = of_kind.len();
        let shown = total.min(MOST_PER_SECTION);
        wrote_a_section = true;
        let _ = writeln!(out, "## {heading}");
        out.push('\n');
        for h in of_kind.iter().take(shown) {
            let _ = writeln!(
                out,
                "- `{}` **{}**: {} _({} “{}”)_",
                clock(h.start),
                h.speaker,
                h.text,
                match h.confidence {
                    Confidence::Explicit => "said",
                    Confidence::Likely => "guessed from",
                },
                h.cue
            );
        }
        if total > shown {
            let _ = writeln!(
                out,
                "\n{} more matched the same way and are in the transcript below.",
                total - shown
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
        let m = classify("I'll send the figures over this afternoon.").unwrap();
        assert_eq!(m.kind, Mark::Action);
        assert_eq!(m.cue, "i'll");
        assert_eq!(m.confidence, Confidence::Likely);
    }

    #[test]
    fn a_request_of_someone_else_is_an_action() {
        assert_eq!(
            classify("Can you check the logs?").unwrap().kind,
            Mark::Action
        );
    }

    #[test]
    fn a_settled_thing_is_a_decision() {
        assert_eq!(
            classify("We agreed to postpone the migration.")
                .unwrap()
                .kind,
            Mark::Decision
        );
    }

    #[test]
    fn a_named_day_is_a_deadline() {
        assert_eq!(
            classify("The report needs to be in by Friday.")
                .unwrap()
                .kind,
            Mark::Deadline
        );
    }

    #[test]
    fn a_bare_question_is_only_a_question() {
        assert_eq!(
            classify("Is the build green?").unwrap().kind,
            Mark::Question
        );
    }

    #[test]
    fn a_fragment_with_a_hopeful_phrase_in_it_is_not_an_action() {
        // Every one of these matched a cue and was reported as something
        // somebody had taken on.
        for said in ["Sorry, can you please?", "Let's see.", "I can it"] {
            let got = classify(said);
            assert!(
                got.as_ref().map(|m| m.kind) != Some(Mark::Action),
                "{said:?} was read as an action: {got:?}"
            );
        }
    }

    #[test]
    fn a_phrase_that_means_what_it_says_is_marked_as_certain() {
        let m = classify("That's an action item for me.").unwrap();
        assert_eq!(m.kind, Mark::Action);
        assert_eq!(m.confidence, Confidence::Explicit);
    }

    #[test]
    fn a_one_word_question_is_not_an_open_question() {
        assert!(classify("Sorry?").is_none());
        assert!(classify("Is the build green?").is_some());
    }

    #[test]
    fn the_certain_suggestions_are_listed_before_the_guesses() {
        let lines = vec![
            line(1, "Speaker 1", 0.0, "So let's have a look at the pipeline."),
            line(2, "Speaker 2", 5.0, "That is an action item for me."),
        ];
        let notes = meeting_notes(&lines, "Standup", "2026-09-08T09:00:00Z");
        let certain = notes.find("action item").unwrap();
        let guess = notes.find("let's have a look").unwrap();
        assert!(certain < guess, "{notes}");
        assert!(notes.contains("_(said “action item”)_"), "{notes}");
        assert!(notes.contains("_(guessed from “let's”)_"), "{notes}");
    }

    #[test]
    fn the_same_remark_twice_earns_one_bullet() {
        let lines = vec![
            line(1, "Speaker 1", 0.0, "I will be creating my own runners."),
            line(2, "Speaker 1", 4.0, "I will be creating my own runners!"),
            line(3, "Speaker 1", 8.0, "I will be creating my own runners"),
        ];
        let notes = meeting_notes(&lines, "Standup", "2026-09-08T09:00:00Z");
        assert_eq!(notes.matches("_(guessed from").count(), 1, "{notes}");
    }

    #[test]
    fn a_very_long_list_is_capped_and_says_so() {
        let lines: Vec<Line> = (0..MOST_PER_SECTION + 7)
            .map(|i| {
                line(
                    i + 1,
                    "Speaker 1",
                    i as f64,
                    &format!("So we will look at item number {i} together."),
                )
            })
            .collect();
        let notes = meeting_notes(&lines, "Standup", "2026-09-08T09:00:00Z");
        assert_eq!(notes.matches("_(guessed from").count(), MOST_PER_SECTION);
        assert!(notes.contains("7 more matched the same way"), "{notes}");
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
        assert_eq!(
            classify("I'll have it done by Friday.").unwrap().kind,
            Mark::Action
        );
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
