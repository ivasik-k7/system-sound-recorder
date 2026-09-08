//! Catching the engine when it starts talking to itself.
//!
//! Speech models decode one token at a time, conditioned on the tokens already
//! produced. Occasionally that feedback closes into a circle and the model
//! emits the same phrase until it runs out of window: forty copies of `it will
//! update the argos dmg nd`, sixty of `we have used lambdas. So,`, a minute of
//! bare full stops over silence. It is not a rare curiosity. In a forty-five
//! minute interview recorded with this tool, a third of the words were loop and
//! not speech, and nothing downstream could tell the difference.
//!
//! The repair here is textual on purpose. The obvious test — nobody says two
//! hundred words in six tenths of a second — turns out to be unusable, because
//! the timings coming back from the model are themselves unreliable: real, short
//! lines routinely carry a duration far shorter than the words in them took to
//! say. Judging speech by those timings throws away good lines. A phrase
//! repeated verbatim ten times in a row, on the other hand, needs no clock to
//! recognise, and is not something people do.
//!
//! What is left after a loop is collapsed is kept rather than discarded. When
//! the model stuttered on `mission of the` seventy-four times, it did hear
//! something; three words of it are honest, and the reader can see them.

/// The longest phrase looked at when hunting for a repeat. Loops observed in
/// practice are short — a few words at most — and searching longer ones costs
/// time for nothing.
const LONGEST_PHRASE: usize = 10;

/// How many times in a row a phrase has to appear before it is treated as a
/// loop rather than as speech.
///
/// Three, which is deliberately not two. People do repeat themselves twice, for
/// emphasis and while thinking, and collapsing that would be rewriting what
/// somebody said. Three identical copies in a row is already unusual, and by
/// the time it is a real loop it is forty.
const LOOP_REPEATS: usize = 3;

/// Loops nest, so collapsing is repeated until nothing changes. The cap only
/// exists so that a bug here cannot hang the recognition thread.
const MAX_PASSES: usize = 64;

/// One repeat that was collapsed, kept so it can be counted and reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loop {
    /// The phrase that was repeating.
    pub phrase: String,
    /// How many times it appeared in a row, including the one that was kept.
    pub times: usize,
}

/// The result of looking at one piece of recognised text.
#[derive(Debug, Clone)]
pub struct Repair {
    /// The text with any loops collapsed to a single occurrence.
    pub text: String,
    /// What was collapsed, in the order it was found.
    pub loops: Vec<Loop>,
}

impl Repair {
    /// Nothing was wrong with it.
    pub fn is_clean(&self) -> bool {
        self.loops.is_empty()
    }

    /// There is nothing left worth showing anybody.
    pub fn is_empty(&self) -> bool {
        is_speechless(&self.text)
    }

    /// How many words the collapse removed.
    pub fn removed_words(&self) -> usize {
        self.loops
            .iter()
            .map(|l| l.phrase.split_whitespace().count() * (l.times - 1))
            .sum()
    }
}

/// True when a string carries no words — empty, or only the punctuation a model
/// emits when it is asked to transcribe silence.
pub fn is_speechless(text: &str) -> bool {
    !text
        .chars()
        .any(|c| c.is_alphanumeric() || matches!(c, '\'' | '’'))
}

/// Collapse any repeated phrase in `text` down to a single occurrence.
pub fn repair(text: &str) -> Repair {
    let mut words: Vec<&str> = text.split_whitespace().collect();
    let mut loops = Vec::new();

    for _ in 0..MAX_PASSES {
        match collapse_longest_run(&mut words) {
            Some(found) => loops.push(found),
            None => break,
        }
    }

    Repair {
        text: words.join(" "),
        loops,
    }
}

/// Find the run of a repeated phrase that covers the most words, and cut it
/// down to one copy. Returns what was cut, or nothing if there was no run.
///
/// The longest run is taken first rather than the first one found, because a
/// loop of `nd it will update` sitting inside a loop of `it will update the
/// argos dmg nd` should be recognised as the bigger thing it is part of.
fn collapse_longest_run(words: &mut Vec<&str>) -> Option<Loop> {
    let mut best: Option<(usize, usize, usize)> = None; // (at, phrase length, repeats)

    for at in 0..words.len() {
        for length in 1..=LONGEST_PHRASE {
            if at + length * 2 > words.len() {
                break;
            }
            let phrase = &words[at..at + length];
            let mut repeats = 1;
            let mut next = at + length;
            while words.get(next..next + length) == Some(phrase) {
                repeats += 1;
                next += length;
            }
            if repeats < LOOP_REPEATS {
                continue;
            }
            let span = repeats * length;
            if best.map(|(_, l, r)| span > r * l).unwrap_or(true) {
                best = Some((at, length, repeats));
            }
        }
    }

    let (at, length, repeats) = best?;
    let phrase = words[at..at + length].join(" ");
    // Keep the first copy, drop the rest.
    words.drain(at + length..at + length * repeats);
    Some(Loop {
        phrase,
        times: repeats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_speech_is_left_alone() {
        let text = "So we run the pipeline nightly and it deploys to staging.";
        let out = repair(text);
        assert!(out.is_clean());
        assert_eq!(out.text, text);
    }

    #[test]
    fn saying_something_twice_is_speech_not_a_loop() {
        // People do this. Collapsing it would be rewriting what they said.
        let out = repair("no no I mean the other one");
        assert!(out.is_clean());
        assert_eq!(out.text, "no no I mean the other one");
    }

    #[test]
    fn a_single_word_repeated_is_collapsed() {
        let out = repair("Okay. Okay. Okay. Okay. Okay. Okay.");
        assert_eq!(out.text, "Okay.");
        assert_eq!(out.loops[0].times, 6);
    }

    #[test]
    fn a_phrase_repeated_is_collapsed_to_one_copy() {
        // Shortened from a real recording: the model stalled on four words and
        // emitted them until the window ran out.
        let looped = "we have used lambdas. So, ".repeat(60);
        let out = repair(&looped);
        assert_eq!(out.text, "we have used lambdas. So,");
        assert_eq!(out.loops.len(), 1);
        assert_eq!(out.loops[0].times, 60);
        assert_eq!(out.removed_words(), 59 * 5);
    }

    #[test]
    fn what_the_model_actually_heard_survives_the_collapse() {
        let out = repair("Okay, it will update the argos dmg nd it will update the argos dmg nd it will update the argos dmg nd");
        assert!(out
            .text
            .starts_with("Okay, it will update the argos dmg nd"));
        assert!(!out.is_empty());
    }

    #[test]
    fn a_loop_in_the_middle_leaves_the_speech_either_side() {
        let out = repair("first yes yes yes yes last");
        assert_eq!(out.text, "first yes last");
    }

    #[test]
    fn nested_loops_are_collapsed_completely() {
        let out = repair("a b a b a b c c c c d");
        assert_eq!(out.text, "a b c d");
        assert_eq!(out.loops.len(), 2);
    }

    #[test]
    fn punctuation_over_silence_is_recognised_as_nothing() {
        let out = repair(&". ".repeat(200));
        assert!(out.is_empty());
    }

    #[test]
    fn silence_and_emptiness_count_as_speechless() {
        assert!(is_speechless(""));
        assert!(is_speechless("   "));
        assert!(is_speechless(". . . ."));
        assert!(is_speechless("-- ... ,"));
        assert!(!is_speechless("no"));
        assert!(!is_speechless("I'm"));
    }

    #[test]
    fn repair_terminates_on_pathological_input() {
        // Every window a loop, nested as deeply as the cap allows.
        let out = repair(&"a ".repeat(4096));
        assert_eq!(out.text, "a");
    }
}
