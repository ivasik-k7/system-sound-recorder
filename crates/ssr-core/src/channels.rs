//! Channel layout discovery.
//!
//! An endpoint's mix format carries a channel mask: a bitfield of speaker
//! positions. Two endpoints can both report six channels and mean different
//! things by it, so the mask is what actually says where the audio is meant to
//! go. Decoding it is what lets the interface say "5.1" and name the channels
//! rather than showing a bare count, and it is what the WAV writer stamps into
//! the file so an editor opens the tracks with the speakers in the right order.

/// Speaker position bits, as defined by `ksmedia.h`.
///
/// Declared here rather than imported because windows-rs scatters them across
/// feature-gated modules, and the values are frozen by the file format.
pub mod speaker {
    pub const FRONT_LEFT: u32 = 0x1;
    pub const FRONT_RIGHT: u32 = 0x2;
    pub const FRONT_CENTER: u32 = 0x4;
    pub const LOW_FREQUENCY: u32 = 0x8;
    pub const BACK_LEFT: u32 = 0x10;
    pub const BACK_RIGHT: u32 = 0x20;
    pub const FRONT_LEFT_OF_CENTER: u32 = 0x40;
    pub const FRONT_RIGHT_OF_CENTER: u32 = 0x80;
    pub const BACK_CENTER: u32 = 0x100;
    pub const SIDE_LEFT: u32 = 0x200;
    pub const SIDE_RIGHT: u32 = 0x400;
    pub const TOP_CENTER: u32 = 0x800;
    pub const TOP_FRONT_LEFT: u32 = 0x1000;
    pub const TOP_FRONT_CENTER: u32 = 0x2000;
    pub const TOP_FRONT_RIGHT: u32 = 0x4000;
    pub const TOP_BACK_LEFT: u32 = 0x8000;
    pub const TOP_BACK_CENTER: u32 = 0x10000;
    pub const TOP_BACK_RIGHT: u32 = 0x20000;
}

/// Common masks, in the order `ksmedia.h` names them.
pub mod layout {
    use super::speaker::*;

    pub const MONO: u32 = FRONT_CENTER;
    pub const STEREO: u32 = FRONT_LEFT | FRONT_RIGHT;
    pub const QUAD: u32 = FRONT_LEFT | FRONT_RIGHT | BACK_LEFT | BACK_RIGHT;
    pub const SURROUND: u32 = FRONT_LEFT | FRONT_RIGHT | FRONT_CENTER | BACK_CENTER;
    pub const FIVE_POINT_ONE: u32 =
        FRONT_LEFT | FRONT_RIGHT | FRONT_CENTER | LOW_FREQUENCY | BACK_LEFT | BACK_RIGHT;
    pub const FIVE_POINT_ONE_SURROUND: u32 =
        FRONT_LEFT | FRONT_RIGHT | FRONT_CENTER | LOW_FREQUENCY | SIDE_LEFT | SIDE_RIGHT;
    pub const SEVEN_POINT_ONE: u32 = FIVE_POINT_ONE | FRONT_LEFT_OF_CENTER | FRONT_RIGHT_OF_CENTER;
    pub const SEVEN_POINT_ONE_SURROUND: u32 = FIVE_POINT_ONE_SURROUND | BACK_LEFT | BACK_RIGHT;
}

/// Every speaker bit paired with its short label and its full name, in the
/// order the bits are laid out, which is also the order the samples arrive in.
const POSITIONS: &[(u32, &str, &str)] = &[
    (speaker::FRONT_LEFT, "L", "Front left"),
    (speaker::FRONT_RIGHT, "R", "Front right"),
    (speaker::FRONT_CENTER, "C", "Front centre"),
    (speaker::LOW_FREQUENCY, "LFE", "Low frequency"),
    (speaker::BACK_LEFT, "Ls", "Back left"),
    (speaker::BACK_RIGHT, "Rs", "Back right"),
    (speaker::FRONT_LEFT_OF_CENTER, "Lc", "Front left of centre"),
    (
        speaker::FRONT_RIGHT_OF_CENTER,
        "Rc",
        "Front right of centre",
    ),
    (speaker::BACK_CENTER, "Cs", "Back centre"),
    (speaker::SIDE_LEFT, "Lss", "Side left"),
    (speaker::SIDE_RIGHT, "Rss", "Side right"),
    (speaker::TOP_CENTER, "Tc", "Top centre"),
    (speaker::TOP_FRONT_LEFT, "Tfl", "Top front left"),
    (speaker::TOP_FRONT_CENTER, "Tfc", "Top front centre"),
    (speaker::TOP_FRONT_RIGHT, "Tfr", "Top front right"),
    (speaker::TOP_BACK_LEFT, "Tbl", "Top back left"),
    (speaker::TOP_BACK_CENTER, "Tbc", "Top back centre"),
    (speaker::TOP_BACK_RIGHT, "Tbr", "Top back right"),
];

/// One channel of a stream, as the endpoint describes it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInfo {
    /// Position in the interleaved frame, from zero.
    pub index: u16,
    /// Short label, such as `LFE`.
    pub label: String,
    /// Full name, such as `Low frequency`.
    pub name: String,
    /// The speaker bit this channel carries, or zero when the endpoint gave no
    /// mask and the position had to be assumed.
    pub mask_bit: u32,
}

/// Decode a channel mask into one entry per channel.
///
/// The mask and the channel count can disagree, and both directions happen. An
/// endpoint may report a mask with more bits set than it has channels, or no
/// mask at all. The channel count is the authority on how many channels arrive,
/// so it wins: extra bits are ignored, and channels the mask does not cover are
/// numbered rather than left out.
pub fn channels_of(mask: u32, channels: u16) -> Vec<ChannelInfo> {
    let mut out = Vec::with_capacity(channels as usize);
    for (bit, label, name) in POSITIONS {
        if out.len() >= channels as usize {
            break;
        }
        if mask & bit != 0 {
            out.push(ChannelInfo {
                index: out.len() as u16,
                label: (*label).to_string(),
                name: (*name).to_string(),
                mask_bit: *bit,
            });
        }
    }

    // No mask, or a mask too sparse for the channel count. Fall back to the
    // conventional interpretation: one channel is mono, two are a stereo pair,
    // and anything beyond that is numbered honestly rather than guessed at.
    while out.len() < channels as usize {
        let index = out.len() as u16;
        let (label, name) = match (channels, index) {
            (1, 0) => ("M".to_string(), "Mono".to_string()),
            (2, 0) => ("L".to_string(), "Front left".to_string()),
            (2, 1) => ("R".to_string(), "Front right".to_string()),
            _ => (format!("{}", index + 1), format!("Channel {}", index + 1)),
        };
        out.push(ChannelInfo {
            index,
            label,
            name,
            mask_bit: 0,
        });
    }
    out
}

/// A short name for a whole layout, such as `5.1` or `Stereo`.
///
/// Falls back to the channel count when the mask is not one of the named
/// arrangements, because "6 channels" is honest where "5.1" would be a guess.
pub fn layout_name(mask: u32, channels: u16) -> String {
    match mask {
        layout::MONO => return "Mono".to_string(),
        layout::STEREO => return "Stereo".to_string(),
        layout::QUAD => return "Quad".to_string(),
        layout::SURROUND => return "Surround".to_string(),
        layout::FIVE_POINT_ONE | layout::FIVE_POINT_ONE_SURROUND => return "5.1".to_string(),
        layout::SEVEN_POINT_ONE | layout::SEVEN_POINT_ONE_SURROUND => return "7.1".to_string(),
        _ => {}
    }
    match channels {
        0 => "no channels".to_string(),
        1 => "Mono".to_string(),
        2 => "Stereo".to_string(),
        n => format!("{n} channels"),
    }
}

/// The default mask for a channel count, for the streams where the engine
/// chooses the format instead of reading it off an endpoint.
pub fn default_mask(channels: u16) -> u32 {
    match channels {
        1 => layout::MONO,
        2 => layout::STEREO,
        4 => layout::QUAD,
        6 => layout::FIVE_POINT_ONE_SURROUND,
        8 => layout::SEVEN_POINT_ONE_SURROUND,
        // An unusual count gets no mask rather than a wrong one; a zero mask is
        // a legal way of saying "unspecified".
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_is_named_and_ordered() {
        let ch = channels_of(layout::STEREO, 2);
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].label, "L");
        assert_eq!(ch[1].label, "R");
        assert_eq!(layout_name(layout::STEREO, 2), "Stereo");
    }

    #[test]
    fn five_one_puts_lfe_in_the_wasapi_position() {
        let ch = channels_of(layout::FIVE_POINT_ONE_SURROUND, 6);
        let labels: Vec<&str> = ch.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, vec!["L", "R", "C", "LFE", "Lss", "Rss"]);
        assert_eq!(layout_name(layout::FIVE_POINT_ONE_SURROUND, 6), "5.1");
    }

    #[test]
    fn a_missing_mask_still_produces_one_entry_per_channel() {
        let ch = channels_of(0, 2);
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].label, "L");
        assert_eq!(ch[1].label, "R");
        assert_eq!(channels_of(0, 3).len(), 3);
        assert_eq!(channels_of(0, 1)[0].label, "M");
    }

    #[test]
    fn the_channel_count_wins_over_a_mask_that_disagrees() {
        // A 5.1 mask on a stereo stream: only two channels really arrive.
        let ch = channels_of(layout::FIVE_POINT_ONE_SURROUND, 2);
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[1].label, "R");

        // And a stereo mask on a four channel stream: the two the mask names,
        // then two numbered.
        let ch = channels_of(layout::STEREO, 4);
        assert_eq!(ch.len(), 4);
        assert_eq!(ch[2].label, "3");
        assert_eq!(ch[3].name, "Channel 4");
    }

    #[test]
    fn an_unnamed_mask_falls_back_to_the_count() {
        assert_eq!(layout_name(0, 6), "6 channels");
        assert_eq!(layout_name(0, 1), "Mono");
    }

    #[test]
    fn default_masks_round_trip_through_the_decoder() {
        for n in [1u16, 2, 4, 6, 8] {
            let mask = default_mask(n);
            assert_eq!(channels_of(mask, n).len() as u16, n, "count for {n}");
        }
        assert_eq!(layout_name(default_mask(8), 8), "7.1");
    }
}
