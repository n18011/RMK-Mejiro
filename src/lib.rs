#![cfg_attr(not(test), no_std)]

pub mod mejiro;
mod mejiro_verbs;

#[cfg(test)]
mod tests {
    use super::mejiro::{Chord, KeyAction, MejiroKey, MejiroSession, StrokeResult};
    use super::mejiro_verbs::VERB_DICTIONARY;

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
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("ka")
        );
        assert!(session.release(MejiroKey::LeftA).is_none());
    }

    #[test]
    fn a_failed_stroke_is_explicitly_reported_for_passthrough() {
        let result = super::mejiro::transform("-A");

        assert!(matches!(result, StrokeResult::Key(_)));
    }

    #[test]
    fn source_commands_keep_their_key_semantics() {
        assert_eq!(
            super::mejiro::transform("#n-n"),
            StrokeResult::Key(KeyAction::ShiftEnter)
        );
        assert_eq!(
            super::mejiro::transform("#-nk"),
            StrokeResult::Key(KeyAction::CtrlEnter)
        );
        assert_eq!(super::mejiro::transform("-SYI").as_text(), Some("z("));
        assert_eq!(super::mejiro::transform("#A").as_text(), Some("aa"));
        assert_eq!(
            super::mejiro::transform("-SYA").as_text(),
            Some("\"\"{#Left}")
        );
    }

    #[test]
    fn source_specific_kana_exceptions_are_preserved() {
        assert_eq!(super::mejiro::transform("STKNU").as_text(), Some("vu"));
        assert_eq!(super::mejiro::transform("SKYI").as_text(), Some("wyi"));
        assert_eq!(super::mejiro::transform("TNYA").as_text(), Some("thi"));
    }

    #[test]
    fn particle_combinations_follow_the_qmk_joshi_table() {
        assert_eq!(super::mejiro::transform("n-t").as_text(), Some("ha,"));
        assert_eq!(super::mejiro::transform("n-k").as_text(), Some("ga,"));
        assert_eq!(super::mejiro::transform("t-k").as_text(), Some("noni"));
    }

    #[test]
    fn abbreviations_and_special_verbs_are_available() {
        assert_eq!(
            super::mejiro::transform("A-SKNIA*").as_text(),
            Some("amerika")
        );
        assert_eq!(super::mejiro::transform("K-SN").as_text(), Some("kannji"));
        assert_eq!(super::mejiro::transform("I-K").as_text(), Some("iku"));
        assert_eq!(super::mejiro::transform("A-STU*").as_text(), Some("aruku"));
        assert_eq!(
            super::mejiro::transform("A-STUntk*").as_text(),
            Some("aruite")
        );
    }

    #[test]
    fn pending_sokuon_is_carried_to_the_next_first_up_stroke() {
        let mut session = MejiroSession::new(true);
        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        session.press(MejiroKey::LeftTStroke);
        session.press(MejiroKey::LeftKStroke);
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("ka")
        );

        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("kka")
        );
    }

    #[test]
    fn undo_removes_the_last_committed_output_length() {
        let mut session = MejiroSession::new(true);
        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("ka")
        );
        assert_eq!(session.history_len(), 1);
        assert_eq!(session.undo_last(), Some(2));
        assert_eq!(session.history_len(), 0);
    }

    #[test]
    fn every_qmk_command_pattern_has_a_rust_result() {
        let commands = [
            "#-", "-U", "#n-n", "#-nk", "#-t", "#-k", "-AU", "-IU", "-S", "-A", "-N", "-Y", "-K",
            "-I", "-T", "-An", "-Nn", "-Yn", "-Kn", "-In", "-Tn", "-n", "n-", "n-n", "-YA", "-NI",
            "-TK", "-IA", "-NY", "-KY", "-SKA", "-YI", "-TY", "-SYI", "-STY", "-NA", "-KN", "-SNA",
            "-SKN", "-NYIA", "-TKNY", "-SNYIA", "-STKNY", "-SYA", "-SNI", "-TYI", "-STYI", "-KNA",
            "-SKNA", "-TKNYIA", "-STKNYIA", "-TKIA", "-KA", "-TNI", "-KYA", "-IAU", "-STK", "-nt",
            "-nk", "n-nt", "n-nk",
        ];
        for command in commands {
            assert!(
                super::mejiro::transform(command).is_supported(),
                "{command}"
            );
        }
    }

    #[test]
    fn consonant_vowel_matrix_and_diphthongs_are_exercised() {
        let consonants = [
            "", "S", "T", "K", "N", "ST", "SK", "TK", "SN", "TN", "KN", "TKN", "STK", "STN", "SKN",
            "STKN",
        ];
        let vowels = ["A", "I", "U", "IA", "AU", "YA", "YU", "YAU"];
        for consonant in consonants {
            for vowel in vowels {
                let stroke = std::format!("{consonant}{vowel}");
                let _ = super::mejiro::transform(&stroke);
            }
        }

        for stroke in [
            "IAUt", "YIt", "YIUt", "YIAt", "YIAUt", "IAUk", "YIk", "YIUk", "YIAk", "YIAUk",
            "IAUnt", "YInt", "YIUnt", "YIAnt", "YIAUnt", "IAUnk", "YInk", "YIUnk", "YIAnk",
            "YIAUnk", "IAUntk", "YIntk", "YIUntk", "YIAntk", "YIAUntk", "IAUtk", "YItk", "YIUtk",
            "YIAtk", "YIAUtk", "STKNU", "SKYA", "TNYIAU",
        ] {
            let _ = super::mejiro::transform(stroke);
        }

        let kana = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをんがぎぐげござじずぜぞだぢづでどばびぶべぼぱぴぷぺぽきゃきゅきょしゃしゅしょちゃちゅちょにゃにゅにょひゃひゅひょみゃみゅみょりゃりゅりょぎゃぎゅぎょじゃじゅじょぢゃぢゅぢょびゃびゅびょぴゃぴゅぴょゔぁうぃふぁてぃでぃとぅどぅぁぃぅぇぉゃゅょっゎー、。";
        assert!(!super::mejiro::kana_to_romaji(kana).is_empty());
    }

    #[test]
    fn all_reference_abbreviation_and_verb_entries_are_callable() {
        let user = [
            "A-SKNIA*",
            "KNUntk-KNU*",
            "KAUn-TAntk*",
            "SKNIA-SAUtk*",
            "TNIA-SNI*",
            "SU-STKNAU*",
            "SU-TKAU*",
            "STKU-STA*",
            "KI-TKNAU*",
            "In-STA*",
            "KAUn-TKNIn*",
            "SI-KNAUt*",
            "SI-SU*",
            "SAU-TKUt*",
            "SAU-SKIA*",
            "TKA-SKIA*",
            "In-NIAtk*",
            "In-STKNAU*",
            "KAU-SKNYU*",
            "SI-SKNYU*",
            "IU-STU*",
            "KAU-IU*",
            "SIn-KNAt*",
            "A-STI*",
            "AU-NIA*",
            "YAU-STAU*",
            "TA-TAU*",
            "KNU-TY*",
            "#NIAntk-SAn*",
        ];
        for stroke in user {
            assert!(super::mejiro::transform(stroke).is_supported(), "{stroke}");
        }

        for entry in VERB_DICTIONARY {
            let stroke = std::format!("{}*", entry.stroke);
            let result = super::mejiro::transform(&stroke);
            assert!(result.is_supported(), "{}", entry.stroke);
        }
    }

    #[test]
    fn non_first_up_sessions_commit_only_after_all_keys_are_released() {
        let mut session = MejiroSession::new(false);
        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        assert!(session.release(MejiroKey::LeftK).is_none());
        assert_eq!(
            session.release(MejiroKey::LeftA).unwrap().as_text(),
            Some("ka")
        );
        session.reset();
        assert_eq!(session.history_len(), 0);
    }

    #[test]
    fn public_key_and_session_boundaries_are_stable() {
        for index in 0..24 {
            assert!(MejiroKey::from_index(index).is_some(), "{index}");
        }
        assert!(MejiroKey::from_index(24).is_none());

        let mut chord = Chord::new();
        assert!(chord.is_empty());
        assert_eq!(chord.bits(), 0);
        chord.press(MejiroKey::LeftS);
        assert!(chord.contains(MejiroKey::LeftS));
        assert!(!chord.contains(MejiroKey::RightS));
        assert_ne!(chord.bits(), 0);
        chord.release(MejiroKey::LeftS);
        assert!(chord.is_empty());

        assert_eq!(StrokeResult::Key(KeyAction::Enter).as_text(), None);

        let mut session = MejiroSession::new(true);
        assert_eq!(session.last_text(), None);
        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("ka")
        );
        assert_eq!(session.last_text(), Some("ka"));
    }
}
