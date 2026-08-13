#![cfg_attr(not(test), no_std)]

pub mod mejiro;

#[cfg(test)]
mod tests {
    use super::mejiro::{Chord, MejiroKey, MejiroSession, StrokeResult};

    #[test]
    fn chord_id_keeps_mejiro_left_right_order() {
        let mut chord = Chord::new();
        chord.press(MejiroKey::LeftS);
        chord.press(MejiroKey::LeftA);
        chord.press(MejiroKey::RightT);

        assert_eq!(chord.id(), "SA-T");
    }

    #[test]
    fn basic_strokes_transform_to_hepburn_romaji() {
        assert_eq!(super::mejiro::transform("A").as_text(), Some("a"));
        assert_eq!(super::mejiro::transform("KA").as_text(), Some("ka"));
        assert_eq!(super::mejiro::transform("KYA").as_text(), Some("kya"));
    }

    #[test]
    fn kana_conversion_uses_longest_match_and_sokuon_rules() {
        assert_eq!(super::mejiro::kana_to_romaji("きゃく"), "kyaku");
        assert_eq!(super::mejiro::kana_to_romaji("っか"), "kka");
        assert_eq!(super::mejiro::kana_to_romaji("っあ"), "xtua");
    }

    #[test]
    fn first_up_commits_once_and_reseeds_held_keys() {
        let mut session = MejiroSession::new(true);

        assert!(session.press(MejiroKey::LeftK).is_none());
        assert!(session.press(MejiroKey::LeftA).is_none());
        assert_eq!(session.release(MejiroKey::LeftK).unwrap().as_text(), Some("ka"));
        assert!(session.release(MejiroKey::LeftA).is_none());
    }

    #[test]
    fn a_failed_stroke_is_explicitly_reported_for_passthrough() {
        let result = super::mejiro::transform("-A");

        assert!(matches!(result, StrokeResult::Key(_)));
    }
}
