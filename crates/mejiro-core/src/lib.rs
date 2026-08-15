#![cfg_attr(not(test), no_std)]

pub mod mejiro;
mod mejiro_output;
mod mejiro_transform;
mod mejiro_verbs;

#[cfg(test)]
mod tests {
    use super::mejiro::{
        key_action_to_hid, Chord, HidAction, HidKey, HidModifier, KeyAction, MejiroKey,
        MejiroSession, StrokeResult,
    };
    use super::mejiro_verbs::VERB_DICTIONARY;

    fn commit_chord(session: &mut MejiroSession, keys: &[MejiroKey]) -> StrokeResult {
        assert!(!keys.is_empty());
        for &key in keys {
            assert!(session.press(key).is_none());
        }
        let result = session.release(keys[0]).expect("first-up release commits");
        for &key in &keys[1..] {
            assert!(session.release(key).is_none());
        }
        result
    }

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
        assert_eq!(super::mejiro::kana_to_romaji("きゃく").unwrap(), "kyaku");
        assert_eq!(super::mejiro::kana_to_romaji("っか").unwrap(), "kka");
        assert_eq!(super::mejiro::kana_to_romaji("っあ").unwrap(), "xtua");
    }

    #[test]
    fn kana_table_matches_the_qmk_reference_for_external_sounds() {
        assert_eq!(super::mejiro::kana_to_romaji("くぁ").unwrap(), "kwa");
        assert_eq!(super::mejiro::kana_to_romaji("てゅ").unwrap(), "texyu");
    }

    #[test]
    fn stn_without_a_vowel_is_an_extra_sound_only_stroke() {
        assert!(matches!(
            super::mejiro::transform("STN"),
            StrokeResult::Unsupported
        ));
        assert_eq!(super::mejiro::transform("STNn").as_text(), Some("nn"));
    }

    #[test]
    fn full_mejiro_chord_is_a_cancel_noop() {
        assert_eq!(
            super::mejiro::transform("STKNYIAUntk#-STKNYIAUntk*"),
            StrokeResult::Noop
        );
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
        assert_eq!(
            super::mejiro::transform("#-S"),
            StrokeResult::RepeatKey(KeyAction::Escape)
        );
        assert_eq!(super::mejiro::transform("-SYI").as_text(), Some("z("));
        assert_eq!(super::mejiro::transform("#A").as_text(), Some("aa"));
        assert_eq!(
            super::mejiro::transform("#A-STU*").as_text(),
            Some("arukuaruku")
        );
        assert_eq!(
            super::mejiro::transform("#NIAntk-SAn*").as_text(),
            Some("oneesann")
        );
        assert_eq!(
            super::mejiro::transform("-SYA").as_text(),
            Some("\"\"{#Left}")
        );
    }

    #[test]
    fn every_non_text_action_has_an_explicit_hid_mapping() {
        let cases = [
            (KeyAction::Backspace, HidKey::Backspace, HidModifier::None),
            (KeyAction::Delete, HidKey::Delete, HidModifier::None),
            (KeyAction::Escape, HidKey::Escape, HidModifier::None),
            (KeyAction::Left, HidKey::Left, HidModifier::None),
            (KeyAction::Down, HidKey::Down, HidModifier::None),
            (KeyAction::Up, HidKey::Up, HidModifier::None),
            (KeyAction::Right, HidKey::Right, HidModifier::None),
            (KeyAction::Home, HidKey::Home, HidModifier::None),
            (KeyAction::End, HidKey::End, HidModifier::None),
            (KeyAction::ShiftLeft, HidKey::Left, HidModifier::Shift),
            (KeyAction::ShiftDown, HidKey::Down, HidModifier::Shift),
            (KeyAction::ShiftUp, HidKey::Up, HidModifier::Shift),
            (KeyAction::ShiftRight, HidKey::Right, HidModifier::Shift),
            (KeyAction::ShiftHome, HidKey::Home, HidModifier::Shift),
            (KeyAction::ShiftEnd, HidKey::End, HidModifier::Shift),
            (KeyAction::ShiftEnter, HidKey::Enter, HidModifier::Shift),
            (KeyAction::CtrlEnter, HidKey::Enter, HidModifier::Ctrl),
            (KeyAction::Enter, HidKey::Enter, HidModifier::None),
            (KeyAction::Space, HidKey::Space, HidModifier::None),
            (KeyAction::Tab, HidKey::Tab, HidModifier::None),
            (KeyAction::Language1, HidKey::Language1, HidModifier::None),
            (KeyAction::Language2, HidKey::Language2, HidModifier::None),
        ];

        for (action, key, modifier) in cases {
            assert_eq!(key_action_to_hid(action), HidAction { key, modifier });
        }
    }

    #[test]
    fn source_specific_kana_exceptions_are_preserved() {
        assert_eq!(super::mejiro::transform("STKNU").as_text(), Some("vu"));
        assert_eq!(super::mejiro::transform("SKYI").as_text(), Some("wyi"));
        assert_eq!(super::mejiro::transform("TNYA").as_text(), Some("thi"));
    }

    #[test]
    fn particle_combinations_follow_the_qmk_joshi_table() {
        assert_eq!(super::mejiro::transform("ntk-").as_text(), Some("he"));
        assert_eq!(super::mejiro::transform("ntk-k").as_text(), Some("heno"));
        assert_eq!(super::mejiro::transform("ntk-nt").as_text(), Some("heha,"));
        assert_eq!(super::mejiro::transform("n-t").as_text(), Some("ha,"));
        assert_eq!(super::mejiro::transform("n-k").as_text(), Some("ga,"));
        assert_eq!(super::mejiro::transform("t-k").as_text(), Some("noni"));
        assert_eq!(super::mejiro::transform("KA-nt").as_text(), Some("ka."));
        assert_eq!(super::mejiro::transform("KAn-nt").as_text(), Some("ka?"));
    }

    #[test]
    fn abbreviations_and_special_verbs_are_available() {
        assert_eq!(
            super::mejiro::transform("A-SKNIA*").as_text(),
            Some("amerika")
        );
        assert_eq!(super::mejiro::transform("K-SN").as_text(), Some("kannji"));
        assert_eq!(super::mejiro::transform("I-K").as_text(), Some("iku"));
        assert_eq!(super::mejiro::transform("A-").as_text(), Some("a"));
        assert_eq!(super::mejiro::transform("K-").as_text(), Some("ka"));
        assert_eq!(super::mejiro::transform("A-STU*").as_text(), Some("aruku"));
        assert_eq!(
            super::mejiro::transform("A-STUntk*").as_text(),
            Some("aruite")
        );
    }

    #[test]
    fn abstract_abbreviations_apply_qmk_post_processing() {
        assert_eq!(
            super::mejiro::transform("K-TKYIAU*").as_text(),
            Some("konnnafuu")
        );
        assert_eq!(
            super::mejiro::transform("SKYU-AUnt*").as_text(),
            Some("iuomoi.")
        );
    }

    #[test]
    fn inferred_ichidan_verbs_and_auxiliaries_are_preserved() {
        assert_eq!(super::mejiro::transform("-KI").as_text(), Some("kiru"));
        assert_eq!(super::mejiro::transform("A-SI*").as_text(), Some("ajiru"));
        assert_eq!(super::mejiro::transform("A-SKI*").as_text(), Some("airu"));
        assert_eq!(super::mejiro::transform("A-SKIA*").as_text(), Some("aeru"));
        assert_eq!(
            super::mejiro::transform("A-AUtk*").as_text(),
            Some("aorimashita")
        );
        assert_eq!(
            super::mejiro::transform("Ant-STUn*").as_text(),
            Some("arukizurai")
        );
    }

    #[test]
    fn auxiliary_te_ta_voicing_does_not_rewrite_the_following_auxiliary() {
        assert_eq!(
            super::mejiro::transform("AUn-YAUntk*").as_text(),
            Some("oyonndeite")
        );
        assert_eq!(
            super::mejiro::transform("AUn-YAUt*").as_text(),
            Some("oyonndeita")
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
    fn right_sokuon_is_carried_even_after_a_left_particle() {
        assert_eq!(super::mejiro::transform("AUk-AUtk").as_text(), Some("okuo"));
    }

    #[test]
    fn english_or_minor_diphthongs_do_not_leave_a_pending_sokuon() {
        let mut session = MejiroSession::new(true);
        for key in [
            MejiroKey::LeftI,
            MejiroKey::LeftA,
            MejiroKey::LeftU,
            MejiroKey::LeftTStroke,
            MejiroKey::LeftKStroke,
        ] {
            session.press(key);
        }
        assert_eq!(
            session.release(MejiroKey::LeftI).unwrap().as_text(),
            Some("au")
        );
        for key in [
            MejiroKey::LeftA,
            MejiroKey::LeftU,
            MejiroKey::LeftTStroke,
            MejiroKey::LeftKStroke,
        ] {
            assert!(session.release(key).is_none());
        }

        session.press(MejiroKey::LeftA);
        assert_eq!(
            session.release(MejiroKey::LeftA).unwrap().as_text(),
            Some("a")
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
    fn backspace_command_updates_the_last_history_length() {
        let mut session = MejiroSession::new(true);
        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("ka")
        );
        assert!(session.release(MejiroKey::LeftA).is_none());

        session.press(MejiroKey::RightA);
        session.press(MejiroKey::RightU);
        assert!(matches!(
            session.release(MejiroKey::RightA),
            Some(StrokeResult::Key(KeyAction::Backspace))
        ));
        assert_eq!(session.undo_last(), Some(1));
    }

    #[test]
    fn repeat_is_recorded_as_a_new_history_entry() {
        let mut session = MejiroSession::new(true);
        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("ka")
        );
        assert!(session.release(MejiroKey::LeftA).is_none());

        session.press(MejiroKey::LeftHash);
        assert!(matches!(
            session.release(MejiroKey::LeftHash),
            Some(StrokeResult::Repeat)
        ));
        assert_eq!(session.history_len(), 2);
        assert_eq!(session.undo_last(), Some(2));
    }

    #[test]
    fn undo_without_history_keeps_qmk_default_backspace_count() {
        let mut session = MejiroSession::new(true);
        assert_eq!(session.undo_last(), Some(2));
    }

    #[test]
    fn qmk_macro_recording_and_replay_are_stateful() {
        let mut session = MejiroSession::new(true);

        for key in [
            MejiroKey::LeftHash,
            MejiroKey::LeftNStroke,
            MejiroKey::RightStar,
        ] {
            session.press(key);
        }
        assert!(matches!(
            session.release(MejiroKey::LeftHash),
            Some(StrokeResult::Noop)
        ));
        assert!(session.release(MejiroKey::LeftNStroke).is_none());
        assert!(session.release(MejiroKey::RightStar).is_none());

        session.press(MejiroKey::LeftK);
        session.press(MejiroKey::LeftA);
        assert_eq!(
            session.release(MejiroKey::LeftK).unwrap().as_text(),
            Some("ka")
        );
        assert!(session.release(MejiroKey::LeftA).is_none());

        for key in [MejiroKey::LeftHash, MejiroKey::RightStar] {
            session.press(key);
        }
        assert!(matches!(
            session.release(MejiroKey::LeftHash),
            Some(StrokeResult::Noop)
        ));
        assert!(session.release(MejiroKey::RightStar).is_none());

        // Reset clears transient state but preserves recorded macro values.
        session.reset();
        session.press(MejiroKey::LeftHash);
        session.press(MejiroKey::LeftNStroke);
        assert_eq!(
            session.release(MejiroKey::LeftHash).unwrap().as_text(),
            Some("ka")
        );
        assert_eq!(session.history_len(), 1);
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

        let kana = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをんがぎぐげござじずぜぞだぢづでどばびぶべぼぱぴぺぽきゃきゅきょゔぁうぃふぁてぃでぃとぅどぅぁぃぅぇぉゃゅょっゎー、。";
        assert!(matches!(
            super::mejiro::kana_to_romaji(kana),
            Err(super::mejiro::OutputError::CapacityExceeded)
        ));
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

    #[test]
    fn output_capacity_is_reported_instead_of_silently_truncated() {
        let kana = "あ".repeat(super::mejiro::MAX_OUTPUT + 1);
        assert_eq!(
            super::mejiro::kana_to_romaji(&kana),
            Err(super::mejiro::OutputError::CapacityExceeded)
        );
        let oversized = "a".repeat(super::mejiro::MAX_OUTPUT + 1);
        assert!(super::mejiro::text_operations(&oversized).is_err());
    }

    #[test]
    fn text_operations_preserve_left_arrow_order() {
        let operations = super::mejiro::text_operations("a{#Left}b{#Left}c").unwrap();
        assert_eq!(operations.len(), 5);
        assert!(matches!(
            operations.get(1),
            Some(super::mejiro::TextOperation::Left)
        ));
        assert!(matches!(
            operations.get(3),
            Some(super::mejiro::TextOperation::Left)
        ));
        assert!(matches!(
            operations.get(0),
            Some(super::mejiro::TextOperation::Text(value)) if value.as_str() == "a"
        ));
        assert!(matches!(
            operations.get(2),
            Some(super::mejiro::TextOperation::Text(value)) if value.as_str() == "b"
        ));
        assert!(matches!(
            operations.get(4),
            Some(super::mejiro::TextOperation::Text(value)) if value.as_str() == "c"
        ));
    }

    #[test]
    fn history_evicts_the_oldest_entry_after_twenty_entries() {
        let mut session = MejiroSession::new(true);
        for _ in 0..21 {
            assert_eq!(
                commit_chord(&mut session, &[MejiroKey::LeftA]).as_text(),
                Some("a")
            );
        }
        assert_eq!(session.history_len(), 20);
        for _ in 0..20 {
            assert_eq!(session.undo_last(), Some(1));
        }
        assert_eq!(session.undo_last(), Some(2));
    }

    #[test]
    fn left_token_and_space_backspace_history_edges_are_explicit() {
        let mut session = MejiroSession::new(true);
        assert_eq!(
            commit_chord(
                &mut session,
                &[MejiroKey::RightS, MejiroKey::RightY, MejiroKey::RightA],
            )
            .as_text(),
            Some("\"\"{#Left}")
        );
        assert_eq!(session.undo_last(), Some(2));

        assert_eq!(
            commit_chord(&mut session, &[MejiroKey::LeftNStroke]),
            StrokeResult::Key(KeyAction::Space)
        );
        assert_eq!(
            commit_chord(&mut session, &[MejiroKey::RightA, MejiroKey::RightU]),
            StrokeResult::Key(KeyAction::Backspace)
        );
        assert_eq!(session.history_len(), 0);
    }

    #[test]
    fn repeat_and_undo_roll_over_history_in_order() {
        let mut session = MejiroSession::new(true);
        assert_eq!(
            commit_chord(&mut session, &[MejiroKey::LeftA]).as_text(),
            Some("a")
        );
        assert_eq!(
            commit_chord(&mut session, &[MejiroKey::LeftHash]),
            StrokeResult::Repeat
        );
        assert_eq!(session.history_len(), 2);
        assert_eq!(
            commit_chord(&mut session, &[MejiroKey::RightU]),
            StrokeResult::Undo
        );
        assert_eq!(session.undo_last(), Some(1));
        assert_eq!(session.history_len(), 1);
    }

    #[test]
    fn oversized_macros_are_marked_truncated_and_never_replayed_partially() {
        let mut session = MejiroSession::new(true);
        assert_eq!(
            commit_chord(
                &mut session,
                &[
                    MejiroKey::LeftHash,
                    MejiroKey::LeftNStroke,
                    MejiroKey::RightStar
                ],
            ),
            StrokeResult::Noop
        );
        for _ in 0..65 {
            assert_eq!(
                commit_chord(&mut session, &[MejiroKey::LeftK, MejiroKey::LeftA]).as_text(),
                Some("ka")
            );
        }
        assert_eq!(
            commit_chord(&mut session, &[MejiroKey::LeftHash, MejiroKey::LeftNStroke]),
            StrokeResult::Truncated
        );
    }
}
