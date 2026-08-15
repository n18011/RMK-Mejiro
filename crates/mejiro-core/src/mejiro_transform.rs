//! Mejiro language tables and stateful transformation pipeline.

use heapless::String;

use crate::mejiro::{KeyAction, OutputError, StrokeResult, Text, TextOperation, MAX_CHORD_ID};
use crate::mejiro_output::text_operations;
use crate::mejiro_verbs::{VerbType, VERB_DICTIONARY};

const MAX_KANA: usize = 512;
type KanaText = String<MAX_KANA>;

#[derive(Clone, Debug)]
pub(crate) struct TransformState {
    pub(crate) last_vowel: String<16>,
    pub(crate) pending_tsu: bool,
    pub(crate) previous_particle: String<16>,
}

impl Default for TransformState {
    fn default() -> Self {
        let mut last_vowel = String::new();
        let _ = last_vowel.push_str("A");
        Self {
            last_vowel,
            pending_tsu: false,
            previous_particle: String::new(),
        }
    }
}

const CONSONANTS: [(&str, &str); 16] = [
    ("", ""),
    ("S", "s"),
    ("T", "t"),
    ("K", "k"),
    ("N", "n"),
    ("ST", "r"),
    ("SK", "w"),
    ("TK", "h"),
    ("SN", "z"),
    ("TN", "d"),
    ("KN", "g"),
    ("TKN", "b"),
    ("STK", "p"),
    ("STN", "l"),
    ("SKN", "m"),
    ("STKN", "f"),
];

const ROMA_ORDER: [&str; 16] = [
    "", "k", "s", "t", "n", "h", "m", "r", "w", "g", "z", "d", "b", "p", "f", "l",
];

const VOWELS: [(&str, &str, usize); 8] = [
    ("A", "a", 0),
    ("I", "i", 1),
    ("U", "u", 2),
    ("IA", "e", 3),
    ("AU", "o", 4),
    ("YA", "ya", 5),
    ("YU", "yu", 6),
    ("YAU", "yo", 7),
];

const DIPHTHONGS: [(&str, &str, &str); 7] = [
    ("Y", "a", "い"),
    ("YI", "yo", "う"),
    ("YIA", "e", "い"),
    ("YIU", "yu", "う"),
    ("YIAU", "u", "う"),
    ("IU", "u", "い"),
    ("IAU", "o", "う"),
];

const MINOR_DIPHTHONGS: [(&str, &str, &str); 5] = [
    ("IAUtk", "a", "う"),
    ("YItk", "i", "い"),
    ("YIUtk", "o", "い"),
    ("YIAtk", "a", "え"),
    ("YIAUtk", "o", "お"),
];

const ENGLISH_DIPHTHONGS: [(&str, &str, &str); 25] = [
    ("IAUt", "a", "す"),
    ("YIt", "i", "す"),
    ("YIUt", "u", "す"),
    ("YIAt", "e", "す"),
    ("YIAUt", "o", "す"),
    ("IAUk", "a", "る"),
    ("YIk", "i", "る"),
    ("YIUk", "u", "る"),
    ("YIAk", "e", "る"),
    ("YIAUk", "o", "る"),
    ("IAUnt", "e", "ーしょん"),
    ("YInt", "i", "しょん"),
    ("YIUnt", "u", "しょん"),
    ("YIAnt", "e", "んしょん"),
    ("YIAUnt", "o", "ーしょん"),
    ("IAUnk", "a", "いんど"),
    ("YInk", "i", "んぐ"),
    ("YIUnk", "a", "んど"),
    ("YIAnk", "e", "んど"),
    ("YIAUnk", "o", "んぐ"),
    ("IAUntk", "a", "ーん"),
    ("YIntk", "i", "ーん"),
    ("YIUntk", "u", "ーん"),
    ("YIAntk", "e", "ーん"),
    ("YIAUntk", "o", "ーん"),
];

const EXCEPTION_KANA: &[(&str, &str)] = &[
    ("STKNU", "ゔ"),
    ("STKNYA", "ゔぁ"),
    ("STKNYI", "ゔぃ"),
    ("STKNYU", "ふゅ"),
    ("STKNYIU", "ゔぇ"),
    ("STKNYAU", "ゔぉ"),
    ("STKNYIAU", "じぇい"),
    ("STKNIAU", "じぇ"),
    ("STKNIU", "ゔゅ"),
    ("SKU", "ゎ"),
    ("SKYA", "うぁ"),
    ("SKYI", "ゐ"),
    ("SKYU", "いう"),
    ("SKYIU", "ゑ"),
    ("SKYAU", "を"),
    ("SKYIAU", "ちぇい"),
    ("SKIAU", "ちぇ"),
    ("SKIU", "ゆい"),
    ("TNYA", "てぃ"),
    ("TNYI", "とぅ"),
    ("TNYU", "でゅ"),
    ("TNYIU", "どぅ"),
    ("TNYAU", "でぃ"),
    ("TNYIAU", "いぇ"),
    ("TNIU", "てゅ"),
    ("STNYA", "すた"),
    ("STNYI", "すち"),
    ("STNYU", "すてぃ"),
    ("STNYIU", "すて"),
    ("STNYAU", "すと"),
    ("STNYIAU", "しぇい"),
    ("STNIAU", "しぇ"),
    ("STNIU", "くす"),
    ("STNY", "すたい"),
    ("STNYIA", "すてい"),
];

const USER_ABBREVIATIONS: &[(&str, &str)] = &[
    ("A-SKNIA*", "あめりか"),
    ("KNUntk-KNU*", "ぐーぐる"),
    ("KAUn-TAntk*", "こんぴゅーたー"),
    ("SKNIA-SAUtk*", "めそっど"),
    ("TNIA-SNI*", "でじたる"),
    ("SU-STKNAU*", "すまーとふぉん"),
    ("SU-TKAU*", "すまほ"),
    ("STKU-STA*", "ぷらすちっく"),
    ("KI-TKNAU*", "きーぼーど"),
    ("In-STA*", "いんふら"),
    ("KAUn-TKNIn*", "こんびに"),
    ("SI-KNAUt*", "しごと"),
    ("SI-SU*", "しすてむ"),
    ("SAU-TKUt*", "そふと"),
    ("SAU-SKIA*", "そふとうぇあ"),
    ("TKA-SKIA*", "はーどうぇあ"),
    ("In-NIAtk*", "いんたーねっと"),
    ("In-STKNAU*", "いんふぉめーしょん"),
    ("KAU-SKNYU*", "こみゅにけーしょん"),
    ("SI-SKNYU*", "しみゅれーしょん"),
    ("IU-STU*", "ういるす"),
    ("KAU-IU*", "ころなういるす"),
    ("SIn-KNAt*", "しんがた"),
    ("A-STI*", "ありがとう"),
    ("AU-NIA*", "おねがい"),
    ("YAU-STAU*", "よろしく"),
    ("TA-TAU*", "たとえば"),
    ("KNU-TY*", "ぐたいてき"),
    ("NIAntk-SAn*", "ねえさん"),
    ("#NIAntk-SAn*", "おねえさん"),
    ("NIntk-SAn*", "にいさん"),
    ("#NIntk-SAn*", "おにいさん"),
    ("KAntk-SAn*", "かあさん"),
    ("#KAntk-SAn*", "おかあさん"),
    ("TAUntk-SAn*", "とうさん"),
    ("#TAUntk-SAn*", "おとうさん"),
    ("TKNAntk-SAn*", "ばあさん"),
    ("#TKNAntk-SAn*", "おばあさん"),
    ("#TKNA-SAn*", "おばさん"),
    ("SNIntk-SAn*", "じいさん"),
    ("#SNIntk-SAn*", "おじいさん"),
    ("#SNI-SAn*", "おじさん"),
];

const ABSTRACT_ABBREVIATIONS: &[(&str, &str)] = &[
    ("-STIA", "あれ"),
    ("-KAU", "あそこ"),
    ("-TI", "あっち"),
    ("-STA", "あちら"),
    ("ST-", "から"),
    ("K-N", "かん"),
    ("K-SN", "かんじ"),
    ("K-T", "こと"),
    ("KN-T", "ごと"),
    ("K-TKN", "ことば"),
    ("K-ST", "ころ"),
    ("K-STIA", "これ"),
    ("K-KAU", "ここ"),
    ("K-TI", "こっち"),
    ("K-STA", "こちら"),
    ("S-STIA", "それ"),
    ("S-KAU", "そこ"),
    ("S-TI", "そっち"),
    ("S-STA", "そちら"),
    ("T-TN", "ただ"),
    ("TN-K", "だけ"),
    ("TN-ST", "だから"),
    ("T-KI", "てき"),
    ("T-K", "とき"),
    ("T-ST", "ところ"),
    ("T-KAU", "とこ"),
    ("TN-STIA", "どれ"),
    ("TN-KAU", "どこ"),
    ("TN-TI", "どっち"),
    ("TN-STA", "どちら"),
    ("N-N", "なに"),
    ("NA-N", "なん"),
    ("TK-T", "ひと"),
    ("SKN-N", "もの"),
    ("SKNAU-N", "もん"),
    ("SK-K", "わけ"),
];

const ABSTRACT_LEFT: &[(&str, &str)] = &[
    ("STN", ""),
    ("", "あの"),
    ("K", "この"),
    ("S", "その"),
    ("TN", "どの"),
    ("T", "との"),
    ("N", "なんの"),
    ("SKYU", "いう"),
    ("IU", "ああいう"),
    ("KIU", "こういう"),
    ("SIU", "そういう"),
    ("TNIU", "どういう"),
    ("TIU", "という"),
    ("NIU", "なんていう"),
];

const ABSTRACT_RIGHT: &[(&str, &str)] = &[
    ("STN", ""),
    ("AU", "おもい"),
    ("KA", "かんじ"),
    ("KNA", "かんがえ"),
    ("TI", "きもち"),
    ("KAU", "こと"),
    ("TKNA", "ことば"),
    ("STAU", "ころ"),
    ("SY", "さい"),
    ("SA", "さま"),
    ("SI", "しごと"),
    ("SYIA", "せい"),
    ("TA", "ため"),
    ("KNY", "ちがい"),
    ("TU", "つもり"),
    ("TYIA", "てい"),
    ("KI", "とき"),
    ("TAU", "ところ"),
    ("TKA", "はなし"),
    ("TKI", "ひと"),
    ("TKYIAU", "ふう"),
    ("TKIAU", "ほう"),
    ("SKNAU", "もの"),
    ("KNAU", "ものごと"),
    ("YI", "よう"),
];

const KANA: [[&str; 8]; 16] = [
    ["あ", "い", "う", "え", "お", "や", "ゆ", "よ"],
    ["か", "き", "く", "け", "こ", "きゃ", "きゅ", "きょ"],
    ["さ", "し", "す", "せ", "そ", "しゃ", "しゅ", "しょ"],
    ["た", "ち", "つ", "て", "と", "ちゃ", "ちゅ", "ちょ"],
    ["な", "に", "ぬ", "ね", "の", "にゃ", "にゅ", "にょ"],
    ["は", "ひ", "ふ", "へ", "ほ", "ひゃ", "ひゅ", "ひょ"],
    ["ま", "み", "む", "め", "も", "みゃ", "みゅ", "みょ"],
    ["ら", "り", "る", "れ", "ろ", "りゃ", "りゅ", "りょ"],
    ["わ", "うぃ", "う", "うぇ", "うぉ", "うぁ", "う", "を"],
    ["が", "ぎ", "ぐ", "げ", "ご", "ぎゃ", "ぎゅ", "ぎょ"],
    ["ざ", "じ", "ず", "ぜ", "ぞ", "じゃ", "じゅ", "じょ"],
    ["だ", "ぢ", "づ", "で", "ど", "ぢゃ", "ぢゅ", "ぢょ"],
    ["ば", "び", "ぶ", "べ", "ぼ", "びゃ", "びゅ", "びょ"],
    ["ぱ", "ぴ", "ぷ", "ぺ", "ぽ", "ぴゃ", "ぴゅ", "ぴょ"],
    ["ふぁ", "ふぃ", "ふ", "ふぇ", "ふぉ", "ふゃ", "ふゅ", "ふょ"],
    ["ぁ", "ぃ", "ぅ", "ぇ", "ぉ", "ゃ", "ゅ", "ょ"],
];

const KANA_ROMAJI: &[(&str, &str)] = &[
    ("あ", "a"),
    ("い", "i"),
    ("う", "u"),
    ("え", "e"),
    ("お", "o"),
    ("か", "ka"),
    ("き", "ki"),
    ("く", "ku"),
    ("け", "ke"),
    ("こ", "ko"),
    ("さ", "sa"),
    ("し", "shi"),
    ("す", "su"),
    ("せ", "se"),
    ("そ", "so"),
    ("た", "ta"),
    ("ち", "chi"),
    ("つ", "tsu"),
    ("て", "te"),
    ("と", "to"),
    ("な", "na"),
    ("に", "ni"),
    ("ぬ", "nu"),
    ("ね", "ne"),
    ("の", "no"),
    ("は", "ha"),
    ("ひ", "hi"),
    ("ふ", "fu"),
    ("へ", "he"),
    ("ほ", "ho"),
    ("ま", "ma"),
    ("み", "mi"),
    ("む", "mu"),
    ("め", "me"),
    ("も", "mo"),
    ("や", "ya"),
    ("ゆ", "yu"),
    ("よ", "yo"),
    ("ら", "ra"),
    ("り", "ri"),
    ("る", "ru"),
    ("れ", "re"),
    ("ろ", "ro"),
    ("わ", "wa"),
    ("ゐ", "wyi"),
    ("ゑ", "wye"),
    ("を", "wo"),
    ("ん", "nn"),
    ("が", "ga"),
    ("ぎ", "gi"),
    ("ぐ", "gu"),
    ("げ", "ge"),
    ("ご", "go"),
    ("ざ", "za"),
    ("じ", "ji"),
    ("ず", "zu"),
    ("ぜ", "ze"),
    ("ぞ", "zo"),
    ("だ", "da"),
    ("ぢ", "di"),
    ("づ", "du"),
    ("で", "de"),
    ("ど", "do"),
    ("ば", "ba"),
    ("び", "bi"),
    ("ぶ", "bu"),
    ("べ", "be"),
    ("ぼ", "bo"),
    ("ぱ", "pa"),
    ("ぴ", "pi"),
    ("ぷ", "pu"),
    ("ぺ", "pe"),
    ("ぽ", "po"),
    ("きゃ", "kya"),
    ("きゅ", "kyu"),
    ("きょ", "kyo"),
    ("しゃ", "sha"),
    ("しゅ", "shu"),
    ("しょ", "sho"),
    ("ちゃ", "cha"),
    ("ちゅ", "chu"),
    ("ちょ", "cho"),
    ("にゃ", "nya"),
    ("にゅ", "nyu"),
    ("にょ", "nyo"),
    ("ひゃ", "hya"),
    ("ひゅ", "hyu"),
    ("ひょ", "hyo"),
    ("みゃ", "mya"),
    ("みゅ", "myu"),
    ("みょ", "myo"),
    ("りゃ", "rya"),
    ("りゅ", "ryu"),
    ("りょ", "ryo"),
    ("ぎゃ", "gya"),
    ("ぎゅ", "gyu"),
    ("ぎょ", "gyo"),
    ("じゃ", "ja"),
    ("じゅ", "ju"),
    ("じょ", "jo"),
    ("ぢゃ", "dya"),
    ("ぢゅ", "dyu"),
    ("ぢょ", "dyo"),
    ("びゃ", "bya"),
    ("びゅ", "byu"),
    ("びょ", "byo"),
    ("ぴゃ", "pya"),
    ("ぴゅ", "pyu"),
    ("ぴょ", "pyo"),
    ("ゔ", "vu"),
    ("ゔぁ", "va"),
    ("ゔぃ", "vi"),
    ("ゔぇ", "ve"),
    ("ゔぉ", "vo"),
    ("ゔゅ", "vyu"),
    ("うぁ", "wha"),
    ("うぃ", "wi"),
    ("うぇ", "we"),
    ("うぉ", "who"),
    ("ふぁ", "fa"),
    ("ふぃ", "fi"),
    ("ふぇ", "fe"),
    ("ふぉ", "fo"),
    ("ふゃ", "fya"),
    ("ふゅ", "fyu"),
    ("ふょ", "fyo"),
    ("くぁ", "kwa"),
    ("くぃ", "kwi"),
    ("くぇ", "kwe"),
    ("くぉ", "kwo"),
    ("いぇ", "ye"),
    ("しぇ", "she"),
    ("じぇ", "je"),
    ("ちぇ", "che"),
    ("てぃ", "thi"),
    ("でぃ", "dhi"),
    ("でゅ", "dhu"),
    ("とぅ", "twu"),
    ("どぅ", "dwu"),
    ("ぁ", "xa"),
    ("ぃ", "xi"),
    ("ぅ", "xu"),
    ("ぇ", "xe"),
    ("ぉ", "xo"),
    ("ゃ", "xya"),
    ("ゅ", "xyu"),
    ("ょ", "xyo"),
    ("っ", "xtu"),
    ("ゎ", "xwa"),
    ("ー", "-"),
    ("、", ","),
    ("。", "."),
    ("!", "!"),
    ("?", "?"),
];

/// Convert hiragana to the same Hepburn-ish ASCII sequence used by QMK.
pub fn kana_to_romaji(input: &str) -> Result<Text, OutputError> {
    let mut output = Text::new();
    let mut rest = input;

    while !rest.is_empty() {
        if rest.starts_with('っ') {
            let after = &rest['っ'.len_utf8()..];
            if let Some((_, roma)) = longest_kana_match(after) {
                let first = roma.as_bytes().first().copied().unwrap_or_default();
                if matches!(first, b'a' | b'e' | b'i' | b'o' | b'u') {
                    output
                        .push_str("xtu")
                        .map_err(|_| OutputError::CapacityExceeded)?;
                } else if first.is_ascii_alphabetic() {
                    output
                        .push(first as char)
                        .map_err(|_| OutputError::CapacityExceeded)?;
                } else {
                    output
                        .push_str("xtu")
                        .map_err(|_| OutputError::CapacityExceeded)?;
                }
            } else {
                output
                    .push_str("xtu")
                    .map_err(|_| OutputError::CapacityExceeded)?;
            }
            rest = after;
            continue;
        }

        if let Some((kana, roma)) = longest_kana_match(rest) {
            output
                .push_str(roma)
                .map_err(|_| OutputError::CapacityExceeded)?;
            rest = &rest[kana.len()..];
            continue;
        }

        if let Some(ch) = rest.chars().next() {
            if ch.is_ascii() {
                output.push(ch).map_err(|_| OutputError::CapacityExceeded)?;
            }
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }

    Ok(output)
}

fn longest_kana_match(input: &str) -> Option<(&'static str, &'static str)> {
    KANA_ROMAJI
        .iter()
        .filter(|(kana, _)| input.starts_with(kana))
        .max_by_key(|(kana, _)| kana.len())
        .copied()
}

/// Transform a stateless stroke. Stateful carry-over (last vowel and っ) is
/// maintained by [`MejiroSession`].
pub fn transform(id: &str) -> StrokeResult {
    let mut state = TransformState::default();
    transform_with_state(id, &mut state)
}

pub(crate) fn transform_with_state(id: &str, state: &mut TransformState) -> StrokeResult {
    if id == "STKNYIAUntk#-STKNYIAUntk*" {
        return StrokeResult::Noop;
    }

    match id {
        "#-" => return StrokeResult::Repeat,
        "-U" => {
            state.pending_tsu = false;
            return StrokeResult::Undo;
        }
        "-AU" => {
            state.pending_tsu = false;
            return StrokeResult::Key(KeyAction::Backspace);
        }
        "-IU" => {
            state.pending_tsu = false;
            return StrokeResult::Key(KeyAction::Delete);
        }
        "-S" => return StrokeResult::Key(KeyAction::Escape),
        "-A" => return StrokeResult::Key(KeyAction::Left),
        "-N" => return StrokeResult::Key(KeyAction::Down),
        "-Y" => return StrokeResult::Key(KeyAction::Up),
        "-K" => return StrokeResult::Key(KeyAction::Right),
        "-I" => return StrokeResult::Key(KeyAction::Home),
        "-T" => return StrokeResult::Key(KeyAction::End),
        "-An" => return StrokeResult::Key(KeyAction::ShiftLeft),
        "-Nn" => return StrokeResult::Key(KeyAction::ShiftDown),
        "-Yn" => return StrokeResult::Key(KeyAction::ShiftUp),
        "-Kn" => return StrokeResult::Key(KeyAction::ShiftRight),
        "-In" => return StrokeResult::Key(KeyAction::ShiftHome),
        "-Tn" => return StrokeResult::Key(KeyAction::ShiftEnd),
        "-n" => return StrokeResult::Key(KeyAction::Enter),
        "n-" => return StrokeResult::Key(KeyAction::Space),
        "n-n" => return StrokeResult::Key(KeyAction::Tab),
        "#n-n" => return StrokeResult::Key(KeyAction::ShiftEnter),
        "#-nk" => return StrokeResult::Key(KeyAction::CtrlEnter),
        "#-t" => return StrokeResult::Key(KeyAction::Language1),
        "#-k" => return StrokeResult::Key(KeyAction::Language2),
        "-YA" => return text_result("\""),
        "-NI" => return text_result("'"),
        "-TK" => return text_result("|"),
        "-IA" => return text_result(":"),
        "-NY" => return text_result("z/"),
        "-KY" => return text_result("z*"),
        "-SKA" => return text_result("~"),
        "-YI" => return text_result("("),
        "-TY" => return text_result(")"),
        "-SYI" => return text_result("z("),
        "-STY" => return text_result("z)"),
        "-NA" => return text_result("["),
        "-KN" => return text_result("]"),
        "-SNA" => return text_result("z["),
        "-SKN" => return text_result("z]"),
        "-NYIA" => return text_result("<"),
        "-TKNY" => return text_result(">"),
        "-SNYIA" => return text_result("z<"),
        "-STKNY" => return text_result("z>"),
        "-SYA" => return text_result("\"\"{#Left}"),
        "-SNI" => return text_result("''{#Left}"),
        "-TYI" => return text_result("(){#Left}"),
        "-STYI" => return text_result("z(z){#Left}"),
        "-KNA" => return text_result("[]{#Left}"),
        "-SKNA" => return text_result("z[z]{#Left}"),
        "-TKNYIA" => return text_result("<>{#Left}"),
        "-STKNYIA" => return text_result("z<z>{#Left}"),
        "-TKIA" => return text_result("z|"),
        "-KA" => return text_result("z."),
        "-TNI" => return text_result("zj"),
        "-KYA" => return text_result("zk"),
        "-IAU" => return text_result("zh"),
        "-STK" => return text_result("zl"),
        "-nt" => return text_result("."),
        "-nk" => return text_result(","),
        "n-nt" => return text_result("?"),
        "n-nk" => return text_result("!"),
        _ => {}
    }

    let has_asterisk = split_id(id).1.ends_with('*');
    let is_user_abbreviation = has_asterisk && USER_ABBREVIATIONS.iter().any(|(key, _)| *key == id);

    // QMK treats `#` as a repeat modifier for every non-user-abbreviation
    // stroke, including starred verb strokes.  The explicit `#...*` user
    // abbreviations are the sole exception.
    if id.contains('#') && !is_user_abbreviation {
        let mut normalized = String::<MAX_CHORD_ID>::new();
        for ch in id.chars().filter(|ch| *ch != '#') {
            if normalized.push(ch).is_err() {
                return StrokeResult::Truncated;
            }
        }
        let result = transform_with_state(normalized.as_str(), state);
        return repeat_result(result);
    }

    let stroke = id.strip_suffix('*').unwrap_or(id);

    if let Some((_, output)) = USER_ABBREVIATIONS.iter().find(|(key, _)| *key == id) {
        return text_from_kana_str(output);
    }

    if !has_asterisk {
        if let Some((_, output)) = ABSTRACT_ABBREVIATIONS
            .iter()
            .find(|(key, _)| *key == stroke)
        {
            return text_from_kana_str(output);
        }
    } else {
        let (left_raw, right_raw) = split_id(stroke);
        let left_part = parse_part(left_raw);
        let right_part = parse_part(right_raw);
        let mut left_stroke = String::<32>::new();
        let mut right_stroke = String::<32>::new();
        let _ = left_stroke.push_str(left_part.conso.as_str());
        let _ = left_stroke.push_str(left_part.vowel.as_str());
        let _ = right_stroke.push_str(right_part.conso.as_str());
        let _ = right_stroke.push_str(right_part.vowel.as_str());

        if let (Some(left_output), Some(right_output)) = (
            abbreviation_side(ABSTRACT_LEFT, left_stroke.as_str()),
            abbreviation_side(ABSTRACT_RIGHT, right_stroke.as_str()),
        ) {
            let mut pair = KanaText::new();
            let _ = pair.push_str(left_output);
            let _ = pair.push_str(right_output);
            let mut output = replace_nofuu_with_nnafuu(pair.as_str());

            if !left_part.particle.is_empty() || !right_part.particle.is_empty() {
                let mut particle = KanaText::new();
                match (left_part.particle.as_str(), right_part.particle.as_str()) {
                    ("n", "") => {
                        let _ = particle.push_str("である");
                    }
                    ("", "n") => {
                        let _ = particle.push_str("だ");
                    }
                    ("n", "n") => {
                        let _ = particle.push_str("だった");
                    }
                    ("", "nt") => {
                        let _ = particle.push('.');
                    }
                    ("", "nk") => {
                        let _ = particle.push('、');
                    }
                    ("n", "nt") => {
                        let _ = particle.push('?');
                    }
                    ("n", "nk") => {
                        let _ = particle.push('!');
                    }
                    _ => transform_particles(
                        left_part.particle.as_str(),
                        right_part.particle.as_str(),
                        &mut particle,
                        &mut state.previous_particle,
                    ),
                }
                if !matches!(
                    (left_part.particle.as_str(), right_part.particle.as_str()),
                    ("", "nt") | ("", "nk") | ("n", "nt") | ("n", "nk")
                ) {
                    replace_he_with_ka(&mut particle);
                }
                let _ = output.push_str(particle.as_str());
            }
            return text_from_kana(output);
        }
    }

    if let Some(result) = transform_verb(stroke, has_asterisk) {
        return result;
    }

    let (left, right) = split_id(id);
    let left_part = parse_part(left);
    let right_part = parse_part(right);

    let left_has_sound = !left_part.conso.is_empty() || !left_part.vowel.is_empty();
    let right_has_sound = !right_part.conso.is_empty() || !right_part.vowel.is_empty();
    let left_has_particle = !left_part.particle.is_empty();
    let right_has_particle = !right_part.particle.is_empty();

    if !left_has_sound && right_has_sound && !left_has_particle {
        return StrokeResult::Unsupported;
    }

    if !left_has_sound && !right_has_sound && (left_has_particle || right_has_particle) {
        let mut kana = KanaText::new();
        transform_particles(
            left_part.particle.as_str(),
            right_part.particle.as_str(),
            &mut kana,
            &mut state.previous_particle,
        );
        return text_from_kana(kana);
    }

    let mut left_vowel = left_part.vowel.clone();
    if left_vowel.is_empty() && !left_part.conso.is_empty() && left_part.conso.as_str() != "STN" {
        left_vowel = state.last_vowel.clone();
    }
    let mut right_vowel = right_part.vowel.clone();
    if right_vowel.is_empty() && !right_part.conso.is_empty() && right_part.conso.as_str() != "STN"
    {
        right_vowel = if !left_vowel.is_empty() {
            left_vowel.clone()
        } else {
            state.last_vowel.clone()
        };
    }
    if !right_vowel.is_empty() {
        state.last_vowel = right_vowel.clone();
    } else if !left_vowel.is_empty() {
        state.last_vowel = left_vowel.clone();
    }

    let left_diphthong =
        has_english_or_minor_diphthong(left_vowel.as_str(), left_part.particle.as_str());
    let right_diphthong =
        has_english_or_minor_diphthong(right_vowel.as_str(), right_part.particle.as_str());
    let left_final_tsu = left_has_sound
        && left_part.particle.as_str() == "tk"
        && !right_has_sound
        && !right_has_particle
        && !left_diphthong;
    let right_final_tsu =
        right_has_sound && right_part.particle.as_str() == "tk" && !right_diphthong;
    let left_plus_particle = left_has_sound && !right_has_sound && right_has_particle;
    let ntk_n = left_has_sound
        && left_part.particle.as_str() == "ntk"
        && !right_has_sound
        && right_part.particle.as_str() == "n";

    let mut kana = KanaText::new();
    if state.pending_tsu {
        let _ = kana.push_str("っ");
        state.pending_tsu = false;
    }

    if left_has_sound {
        let left_kana = convert_to_kana(
            left_part.conso.as_str(),
            left_vowel.as_str(),
            left_part.particle.as_str(),
            !left_plus_particle && !left_final_tsu,
        );
        let _ = kana.push_str(left_kana.as_str());
    }

    if left_plus_particle {
        let mut particles = KanaText::new();
        if let Some(command) =
            particle_command(left_part.particle.as_str(), right_part.particle.as_str())
        {
            let _ = particles.push_str(command);
        } else {
            transform_particles(
                left_part.particle.as_str(),
                right_part.particle.as_str(),
                &mut particles,
                &mut state.previous_particle,
            );
            replace_he_with_ka(&mut particles);
        }
        let _ = kana.push_str(particles.as_str());
    }

    if !left_has_sound && left_has_particle && right_has_sound {
        let _ = kana.push_str(second_sound(left_part.particle.as_str()));
    }

    if right_has_sound {
        let right_kana = convert_to_kana(
            right_part.conso.as_str(),
            right_vowel.as_str(),
            right_part.particle.as_str(),
            !right_final_tsu,
        );
        let _ = kana.push_str(right_kana.as_str());
    }

    if ntk_n {
        let _ = kana.push_str("ん");
    }

    if left_final_tsu || right_final_tsu {
        if left_part.conso.as_str() == "STN"
            && left_vowel.is_empty()
            && left_part.particle.as_str() == "tk"
        {
            kana.clear();
            let _ = kana.push_str("っ");
        } else {
            state.pending_tsu = true;
            if kana.as_str().ends_with('っ') {
                let _ = kana.pop();
            }
        }
    }

    if kana.is_empty() {
        if state.pending_tsu {
            return StrokeResult::Text {
                text: Text::new(),
                kana_length: 0,
            };
        }
        return StrokeResult::Unsupported;
    }

    text_from_kana(kana)
}

fn text_result(value: &str) -> StrokeResult {
    let mut text = Text::new();
    if text.push_str(value).is_err() {
        return StrokeResult::Truncated;
    }
    StrokeResult::Text {
        text,
        kana_length: 0,
    }
}

fn transform_verb(stroke: &str, has_asterisk: bool) -> Option<StrokeResult> {
    let (left_raw, right_raw) = split_id(stroke);
    let left = parse_part(left_raw);
    let right = parse_part(right_raw);
    let mut base = String::<64>::new();
    let _ = base.push_str(left.conso.as_str());
    let _ = base.push_str(left.vowel.as_str());
    let _ = base.push('-');
    let _ = base.push_str(right.conso.as_str());
    let _ = base.push_str(right.vowel.as_str());

    let left_kana = convert_to_kana(left.conso.as_str(), left.vowel.as_str(), "", false);
    let right_kana = convert_to_kana(right.conso.as_str(), right.vowel.as_str(), "", false);
    let left_syllable = convert_to_syllable(
        left.conso.as_str(),
        left.vowel.as_str(),
        left.particle.as_str(),
    );

    let left_aux = left_auxiliary(left.particle.as_str());
    let right_aux = right_auxiliary(right.particle.as_str());
    let (form, suffix) = if let Some(aux) = left_aux {
        (aux.form, "")
    } else {
        conjugation_info(left.particle.as_str(), right.particle.as_str())
    };

    if right.conso.as_str() == "TN" && right.vowel.is_empty() {
        if let Some(desu) = desu_conjugate(right.particle.as_str()) {
            let mut output = left_syllable.clone();
            let _ = output.push_str(desu);
            return Some(text_from_kana(output));
        }
    }

    if stroke.contains('-') {
        match base.as_str() {
            "I-K" => {
                let mut output = KanaText::new();
                let _ = output.push_str(IKU_FORMS[form]);
                finish_verb(&mut output, left_aux, right_aux, suffix);
                return Some(text_from_kana(output));
            }
            "A-" if has_asterisk => {
                let mut output = KanaText::new();
                let _ = output.push_str(ARU_FORMS[form]);
                finish_verb(&mut output, left_aux, right_aux, suffix);
                if output.as_str() == "ず" {
                    output.clear();
                    let _ = output.push_str("あらず");
                }
                return Some(text_from_kana(output));
            }
            "K-" if has_asterisk => {
                let mut output = KanaText::new();
                let _ = output.push_str(KAHEN_FORMS[form]);
                finish_verb(&mut output, left_aux, right_aux, suffix);
                return Some(text_from_kana(output));
            }
            _ => {}
        }
    }

    let left_has_sound = !left.conso.is_empty() || !left.vowel.is_empty();
    let right_has_sound = !right.conso.is_empty() || !right.vowel.is_empty();

    // These inference branches are intentionally available without `*`, just
    // as in the QMK implementation. They must run before the right-only
    // stroke is rejected by the general Mejiro transform.
    if right_has_sound && !right.conso.is_empty() && right.vowel.is_empty() {
        if let Some(row) =
            kana_row(right_kana.as_str()).or_else(|| consonant_row(right.conso.as_str()))
        {
            if matches!(row, 'k' | 'g' | 's' | 't' | 'n' | 'b' | 'm' | 'r' | 'w') {
                let mut output = left_kana.clone();
                let _ = output.push_str(godan_ending(row, form));
                finish_verb(&mut output, left_aux, right_aux, suffix);
                if form == CONJ_TE_TA && matches!(row, 'g' | 'n' | 'b' | 'm') {
                    apply_godan_te_ta_voicing(&mut output, left_kana.len());
                }
                return Some(text_from_kana(output));
            }
        }
    }

    if !left_has_sound && left.particle.is_empty() && right.vowel.as_str() == "I" {
        if let Some(row) = kana_row(right_kana.as_str()) {
            if is_kami_row(row) {
                let mut output = KanaText::new();
                let _ = output.push_str(kami_ending(row, form));
                finish_verb(&mut output, left_aux, right_aux, suffix);
                return Some(text_from_kana(output));
            }
        }
    }

    if !left_has_sound && left.particle.is_empty() && right.vowel.as_str() == "IA" {
        if let Some(row) = kana_row(right_kana.as_str()) {
            let mut output = KanaText::new();
            let _ = output.push_str(simo_ending(row, form));
            finish_verb(&mut output, left_aux, right_aux, suffix);
            return Some(text_from_kana(output));
        }
    }

    if !has_asterisk {
        return None;
    }

    if right.conso.is_empty() && right.vowel.as_str() == "IU" {
        let mut output = left_kana.clone();
        let _ = output.push_str("い");
        let _ = output.push_str(godan_ending('w', form));
        let _ = output.push_str(suffix);
        return Some(text_from_kana(output));
    }

    if let Some(entry) = VERB_DICTIONARY
        .iter()
        .find(|entry| entry.stroke == base.as_str())
    {
        let mut output = KanaText::new();
        match entry.kind {
            VerbType::Special => match entry.stroke {
                "I-K" => {
                    let _ = output.push_str(IKU_FORMS[form]);
                }
                "A-" => {
                    let _ = output.push_str(ARU_FORMS[form]);
                }
                _ => return None,
            },
            VerbType::Godan => {
                let _ = output.push_str(entry.stem);
                let _ = output.push_str(godan_ending(entry.row, form));
            }
            VerbType::Kami => {
                let _ = output.push_str(entry.stem);
                let _ = output.push_str(kami_ending(entry.row, form));
            }
            VerbType::Simo => {
                let _ = output.push_str(entry.stem);
                let _ = output.push_str(simo_ending(entry.row, form));
            }
            VerbType::Kahen => {
                let _ = output.push_str(entry.stem);
                let _ = output.push_str(KAHEN_FORMS[form]);
            }
        }
        finish_verb(&mut output, left_aux, right_aux, suffix);
        if form == CONJ_TE_TA
            && matches!(entry.kind, VerbType::Godan)
            && matches!(entry.row, 'g' | 'n' | 'b' | 'm')
        {
            apply_godan_te_ta_voicing(&mut output, entry.stem.len());
        }
        if matches!(entry.kind, VerbType::Godan) {
            apply_kudasari_correction(&mut output);
        }
        if entry.kind == VerbType::Special && entry.stroke == "A-" && output.as_str() == "ず" {
            output.clear();
            let _ = output.push_str("あらず");
        }
        return Some(text_from_kana(output));
    }

    // Asterisked strokes not in the dictionary follow the same fallback
    // rules as QMK: sa-hen for a missing right syllable, then inferred rows,
    // and finally the generic r-row fallback.
    if right_kana.is_empty() {
        let mut output = left_kana.clone();
        let _ = output.push_str(sahen_ending(form));
        finish_verb(&mut output, left_aux, right_aux, suffix);
        apply_sahen_negative_zu(&mut output, form, suffix);
        return Some(text_from_kana(output));
    }

    if !right.conso.is_empty() && right.vowel.is_empty() {
        if let Some(row) =
            kana_row(right_kana.as_str()).or_else(|| consonant_row(right.conso.as_str()))
        {
            if matches!(row, 'k' | 'g' | 's' | 't' | 'n' | 'b' | 'm' | 'r' | 'w') {
                let mut output = left_kana.clone();
                let _ = output.push_str(godan_ending(row, form));
                finish_verb(&mut output, left_aux, right_aux, suffix);
                if form == CONJ_TE_TA && matches!(row, 'g' | 'n' | 'b' | 'm') {
                    apply_godan_te_ta_voicing(&mut output, left_kana.len());
                }
                return Some(text_from_kana(output));
            }
        }
    }

    if right.vowel.as_str() == "I" {
        if let Some(row) = kana_row(right_kana.as_str()) {
            if is_kami_row(row) {
                let mut output = left_kana.clone();
                let _ = output.push_str(kami_ending(row, form));
                finish_verb(&mut output, left_aux, right_aux, suffix);
                return Some(text_from_kana(output));
            }
        }
    }

    if right.vowel.as_str() == "IA" {
        if let Some(row) = kana_row(right_kana.as_str()) {
            let mut output = left_kana.clone();
            let _ = output.push_str(simo_ending(row, form));
            finish_verb(&mut output, left_aux, right_aux, suffix);
            return Some(text_from_kana(output));
        }
    }

    if right_has_sound {
        let mut output = left_kana.clone();
        let _ = output.push_str(right_kana.as_str());
        let _ = output.push_str(godan_ending('r', form));
        finish_verb(&mut output, left_aux, right_aux, suffix);
        replace_first(&mut output, "ござり", "ござい");
        replace_first(&mut output, "なさり", "なさい");
        return Some(text_from_kana(output));
    }

    None
}

fn convert_to_syllable(conso: &str, vowel: &str, particle: &str) -> KanaText {
    convert_to_kana(conso, vowel, particle, true)
}

fn is_kami_row(row: char) -> bool {
    matches!(
        row,
        'k' | 'g' | 's' | 'z' | 't' | 'n' | 'b' | 'm' | 'r' | 'w'
    )
}

#[derive(Clone, Copy)]
struct LeftAuxiliary {
    form: usize,
    stem: &'static str,
    kind: VerbType,
    row: char,
}

fn left_auxiliary(particle: &str) -> Option<LeftAuxiliary> {
    Some(match particle {
        "n" => LeftAuxiliary {
            form: CONJ_TE_TA,
            stem: "て",
            kind: VerbType::Kami,
            row: 'w',
        },
        "t" => LeftAuxiliary {
            form: CONJ_SHIEKI,
            stem: "",
            kind: VerbType::Simo,
            row: 's',
        },
        "k" => LeftAuxiliary {
            form: CONJ_UKEMI,
            stem: "",
            kind: VerbType::Simo,
            row: 'r',
        },
        "nk" => LeftAuxiliary {
            form: CONJ_TE_TA,
            stem: "てしま",
            kind: VerbType::Godan,
            row: 'w',
        },
        _ => return None,
    })
}

fn right_auxiliary(particle: &str) -> Option<(usize, &'static str)> {
    Some(match particle {
        "" => (CONJ_JISHO, ""),
        "n" => (CONJ_NAI, "ない"),
        "t" => (CONJ_TE_TA, "た"),
        "k" => (CONJ_MASU, "ます"),
        "nt" => (CONJ_NAI, "なかった"),
        "nk" => (CONJ_MASU, "ません"),
        "tk" => (CONJ_MASU, "ました"),
        "ntk" => (CONJ_TE_TA, "て"),
        _ => return None,
    })
}

fn desu_conjugate(particle: &str) -> Option<&'static str> {
    Some(match particle {
        "" => "です",
        "n" => "ですね",
        "t" => "でした",
        "k" => "でしょう",
        "nt" => "です.",
        "nk" => "ですが,",
        "tk" => "ですか?",
        "ntk" => "でして,",
        _ => return None,
    })
}

fn conjugation_info(left_particle: &str, right_particle: &str) -> (usize, &'static str) {
    match (left_particle, right_particle) {
        ("nt", "") => (CONJ_MASU, "やすい"),
        ("nt", "k") => (CONJ_MASU, "やすく"),
        ("nt", "n") => (CONJ_MASU, "ずらい"),
        ("nt", "nk") => (CONJ_MASU, "ずらく"),
        ("nt", "t") => (CONJ_MASU, "たい"),
        ("nt", "tk") => (CONJ_MASU, "たく"),
        ("nt", "nt") => (CONJ_TE_TA, "てほしい"),
        ("nt", "ntk") => (CONJ_TE_TA, "てください"),
        ("tk", "") => (CONJ_KANOU, "る"),
        ("tk", "n") => (CONJ_KANOU, "ない"),
        ("tk", "t") => (CONJ_KANOU, "た"),
        ("tk", "k") => (CONJ_KANOU, "ます"),
        ("tk", "nt") => (CONJ_KANOU, "なかった"),
        ("tk", "nk") => (CONJ_KANOU, "ません"),
        ("tk", "tk") => (CONJ_KANOU, "ました"),
        ("tk", "ntk") => (CONJ_KANOU, "て"),
        ("ntk", "") => (CONJ_MASU, ""),
        ("ntk", "n") => (CONJ_NAI, "ず"),
        ("ntk", "t") => (CONJ_KATEI, "ば"),
        ("ntk", "k") => (CONJ_MASU, "ましょう"),
        ("ntk", "nt") => (CONJ_NAI, "なければ"),
        ("ntk", "nk") => (CONJ_NAI, "なく"),
        ("ntk", "tk") => (CONJ_MASU, "ながら"),
        ("ntk", "ntk") => (CONJ_IKOU, ""),
        (_, _) => right_auxiliary(right_particle).unwrap_or((CONJ_JISHO, "")),
    }
}

fn finish_verb(
    output: &mut KanaText,
    left_aux: Option<LeftAuxiliary>,
    right_aux: Option<(usize, &'static str)>,
    suffix: &str,
) {
    if let Some(aux) = left_aux {
        let _ = output.push_str(aux.stem);
        let aux_form = right_aux.map(|(form, _)| form).unwrap_or(CONJ_JISHO);
        let ending = match aux.kind {
            VerbType::Godan => godan_ending(aux.row, aux_form),
            VerbType::Kami => kami_ending(aux.row, aux_form),
            VerbType::Simo => simo_ending(aux.row, aux_form),
            VerbType::Kahen | VerbType::Special => "",
        };
        let _ = output.push_str(ending);
        if let Some((_, right_suffix)) = right_aux {
            let _ = output.push_str(right_suffix);
        }
    } else {
        let _ = output.push_str(suffix);
    }
}

fn sahen_ending(form: usize) -> &'static str {
    [
        "し",
        "さ",
        "さ",
        "し",
        "する",
        "し",
        "しよう",
        "すれ",
        "でき",
        "しろ",
    ][form.min(9)]
}

fn replace_first(output: &mut KanaText, from: &str, to: &str) {
    let source = output.as_str();
    let Some(index) = source.find(from) else {
        return;
    };
    let mut replaced = KanaText::new();
    let _ = replaced.push_str(&source[..index]);
    let _ = replaced.push_str(to);
    let _ = replaced.push_str(&source[index + from.len()..]);
    *output = replaced;
}

fn replace_at(output: &mut KanaText, start: usize, from: &str, to: &str) {
    let source = output.as_str();
    let Some(suffix) = source.get(start..) else {
        return;
    };
    if !suffix.starts_with(from) {
        return;
    }
    let mut replaced = KanaText::new();
    let _ = replaced.push_str(&source[..start]);
    let _ = replaced.push_str(to);
    let _ = replaced.push_str(&suffix[from.len()..]);
    *output = replaced;
}

fn apply_godan_te_ta_voicing(output: &mut KanaText, stem_len: usize) {
    replace_at(output, stem_len, "んて", "んで");
    replace_at(output, stem_len, "いて", "いで");
    replace_at(output, stem_len, "んた", "んだ");
    replace_at(output, stem_len, "いた", "いだ");
}

fn apply_sahen_negative_zu(output: &mut KanaText, form: usize, suffix: &str) {
    if form == CONJ_NAI && suffix == "ず" && output.as_str().ends_with("しず") {
        let len = output.len();
        let mut replaced = KanaText::new();
        let _ = replaced.push_str(&output.as_str()[..len - "しず".len()]);
        let _ = replaced.push_str("せず");
        *output = replaced;
    }
}

fn apply_kudasari_correction(output: &mut KanaText) {
    replace_first(output, "くださり", "ください");
}

const IKU_FORMS: [&str; 10] = [
    "いか",
    "いか",
    "いか",
    "いき",
    "いく",
    "いっ",
    "いこう",
    "いけ",
    "いけ",
    "いけ",
];
const ARU_FORMS: [&str; 10] = [
    "",
    "あら",
    "あら",
    "あり",
    "ある",
    "あっ",
    "あろう",
    "あれ",
    "ありえ",
    "あれ",
];
const KAHEN_FORMS: [&str; 10] = [
    "こ",
    "こさ",
    "こら",
    "き",
    "くる",
    "き",
    "こよう",
    "くれ",
    "これ",
    "こい",
];

const CONJ_NAI: usize = 0;
const CONJ_SHIEKI: usize = 1;
const CONJ_UKEMI: usize = 2;
const CONJ_MASU: usize = 3;
const CONJ_JISHO: usize = 4;
const CONJ_TE_TA: usize = 5;
const CONJ_IKOU: usize = 6;
const CONJ_KATEI: usize = 7;
const CONJ_KANOU: usize = 8;

fn godan_ending(row: char, form: usize) -> &'static str {
    const ENDINGS: &[(char, [&str; 10])] = &[
        (
            'k',
            ["か", "か", "か", "き", "く", "い", "こう", "け", "け", "け"],
        ),
        (
            'g',
            ["が", "が", "が", "ぎ", "ぐ", "い", "ごう", "げ", "げ", "げ"],
        ),
        (
            's',
            ["さ", "さ", "さ", "し", "す", "し", "そう", "せ", "せ", "せ"],
        ),
        (
            't',
            ["た", "た", "た", "ち", "つ", "っ", "とう", "て", "て", "て"],
        ),
        (
            'n',
            ["な", "な", "な", "に", "ぬ", "ん", "のう", "ね", "ね", "ね"],
        ),
        (
            'b',
            ["ば", "ば", "ば", "び", "ぶ", "ん", "ぼう", "べ", "べ", "べ"],
        ),
        (
            'm',
            ["ま", "ま", "ま", "み", "む", "ん", "もう", "め", "め", "め"],
        ),
        (
            'r',
            ["ら", "ら", "ら", "り", "る", "っ", "ろう", "れ", "れ", "れ"],
        ),
        (
            'w',
            ["わ", "わ", "わ", "い", "う", "っ", "おう", "え", "え", "え"],
        ),
    ];
    ENDINGS
        .iter()
        .find(|(candidate, _)| *candidate == row || (row == 'z' && *candidate == 's'))
        .map(|(_, endings)| endings[form.min(9)])
        .unwrap_or("")
}

fn kami_ending(row: char, form: usize) -> &'static str {
    const ENDINGS: &[(char, [&str; 10])] = &[
        (
            'k',
            [
                "き",
                "きさ",
                "きら",
                "き",
                "きる",
                "き",
                "きよう",
                "きれ",
                "きれ",
                "きろ",
            ],
        ),
        (
            'g',
            [
                "ぎ",
                "ぎさ",
                "ぎら",
                "ぎ",
                "ぎる",
                "ぎ",
                "ぎよう",
                "ぎれ",
                "ぎれ",
                "ぎろ",
            ],
        ),
        (
            'z',
            [
                "じ",
                "じさ",
                "じら",
                "じ",
                "じる",
                "じ",
                "じよう",
                "じれ",
                "じれ",
                "じろ",
            ],
        ),
        (
            't',
            [
                "ち",
                "ちさ",
                "ちら",
                "ち",
                "ちる",
                "ち",
                "ちよう",
                "ちれ",
                "ちれ",
                "ちろ",
            ],
        ),
        (
            'n',
            [
                "に",
                "にさ",
                "にら",
                "に",
                "にる",
                "に",
                "によう",
                "にれ",
                "にれ",
                "にろ",
            ],
        ),
        (
            'b',
            [
                "び",
                "びさ",
                "びら",
                "び",
                "びる",
                "び",
                "びよう",
                "びれ",
                "びれ",
                "びろ",
            ],
        ),
        (
            'm',
            [
                "み",
                "みさ",
                "みら",
                "み",
                "みる",
                "み",
                "みよう",
                "みれ",
                "みれ",
                "みろ",
            ],
        ),
        (
            'r',
            [
                "り",
                "りさ",
                "りら",
                "り",
                "りる",
                "り",
                "りよう",
                "りれ",
                "りれ",
                "りろ",
            ],
        ),
        (
            'w',
            [
                "い",
                "いさ",
                "いら",
                "い",
                "いる",
                "い",
                "いよう",
                "いれ",
                "いれ",
                "いろ",
            ],
        ),
    ];
    ENDINGS
        .iter()
        .find(|(candidate, _)| *candidate == row || (row == 's' && *candidate == 'z'))
        .map(|(_, endings)| endings[form.min(9)])
        .unwrap_or("")
}

fn simo_ending(row: char, form: usize) -> &'static str {
    const ENDINGS: &[(char, [&str; 10])] = &[
        (
            'k',
            [
                "け",
                "けさ",
                "けら",
                "け",
                "ける",
                "け",
                "けよう",
                "けれ",
                "けれ",
                "けろ",
            ],
        ),
        (
            'g',
            [
                "げ",
                "げさ",
                "げら",
                "げ",
                "げる",
                "げ",
                "げよう",
                "げれ",
                "げれ",
                "げろ",
            ],
        ),
        (
            's',
            [
                "せ",
                "せさ",
                "せら",
                "せ",
                "せる",
                "せ",
                "せよう",
                "せれ",
                "せれ",
                "せろ",
            ],
        ),
        (
            'z',
            [
                "ぜ",
                "ぜさ",
                "ぜら",
                "ぜ",
                "ぜる",
                "ぜ",
                "ぜよう",
                "ぜれ",
                "ぜれ",
                "ぜろ",
            ],
        ),
        (
            't',
            [
                "て",
                "てさ",
                "てら",
                "て",
                "てる",
                "て",
                "てよう",
                "てれ",
                "てれ",
                "てろ",
            ],
        ),
        (
            'd',
            [
                "で",
                "でさ",
                "でら",
                "で",
                "でる",
                "で",
                "でよう",
                "でれ",
                "でれ",
                "でろ",
            ],
        ),
        (
            'n',
            [
                "ね",
                "ねさ",
                "ねら",
                "ね",
                "ねる",
                "ね",
                "ねよう",
                "ねれ",
                "ねれ",
                "ねろ",
            ],
        ),
        (
            'h',
            [
                "へ",
                "へさ",
                "へら",
                "へ",
                "へる",
                "へ",
                "へよう",
                "へれ",
                "へれ",
                "へろ",
            ],
        ),
        (
            'b',
            [
                "べ",
                "べさ",
                "べら",
                "べ",
                "べる",
                "べ",
                "べよう",
                "べれ",
                "べれ",
                "べろ",
            ],
        ),
        (
            'm',
            [
                "め",
                "めさ",
                "めら",
                "め",
                "める",
                "め",
                "めよう",
                "めれ",
                "めれ",
                "めろ",
            ],
        ),
        (
            'r',
            [
                "れ",
                "れさ",
                "れら",
                "れ",
                "れる",
                "れ",
                "れよう",
                "れれ",
                "れれ",
                "れろ",
            ],
        ),
        (
            'w',
            [
                "え",
                "えさ",
                "えら",
                "え",
                "える",
                "え",
                "えよう",
                "えれ",
                "えれ",
                "えろ",
            ],
        ),
    ];
    ENDINGS
        .iter()
        .find(|(candidate, _)| *candidate == row)
        .map(|(_, endings)| endings[form.min(9)])
        .unwrap_or("")
}

fn kana_row(kana: &str) -> Option<char> {
    [
        ("かきくけこ", 'k'),
        ("がぎぐげご", 'g'),
        ("さしすせそ", 's'),
        ("ざじずぜぞ", 'z'),
        ("たちつてと", 't'),
        ("だぢづでど", 'd'),
        ("なにぬねの", 'n'),
        ("はひふへほ", 'h'),
        ("ばびぶべぼ", 'b'),
        ("まみむめも", 'm'),
        ("らりるれろ", 'r'),
        ("わをあいうえお", 'w'),
    ]
    .iter()
    .find(|(row, _)| kana.chars().next().is_some_and(|ch| row.contains(ch)))
    .map(|(_, row)| *row)
}

fn consonant_row(conso: &str) -> Option<char> {
    match consonant_roma(conso)?.chars().next()? {
        'k' => Some('k'),
        's' => Some('s'),
        't' => Some('t'),
        'n' => Some('n'),
        'h' => Some('h'),
        'm' => Some('m'),
        'r' => Some('r'),
        'w' => Some('w'),
        'g' => Some('g'),
        'z' => Some('z'),
        'd' => Some('d'),
        'b' => Some('b'),
        _ => None,
    }
}

fn repeat_result(result: StrokeResult) -> StrokeResult {
    match result {
        StrokeResult::Text { text, kana_length } => {
            let mut repeated = Text::new();
            if repeated.push_str(text.as_str()).is_err()
                || repeated.push_str(text.as_str()).is_err()
            {
                return StrokeResult::Truncated;
            }
            StrokeResult::Text {
                text: repeated,
                kana_length: kana_length.saturating_mul(2),
            }
        }
        StrokeResult::Key(action) => StrokeResult::RepeatKey(action),
        other => other,
    }
}

fn text_from_kana_str(value: &str) -> StrokeResult {
    text_from_kana(value)
}

fn text_from_kana<S: AsRef<str>>(kana: S) -> StrokeResult {
    let kana = kana.as_ref();
    let kana_length = kana.chars().count().min(u8::MAX as usize) as u8;
    match kana_to_romaji(kana) {
        Ok(text) => StrokeResult::Text { text, kana_length },
        Err(OutputError::CapacityExceeded) => StrokeResult::Truncated,
    }
}

fn replace_nofuu_with_nnafuu(input: &str) -> KanaText {
    let mut output = KanaText::new();
    let mut rest = input;
    while let Some(index) = rest.find("のふう") {
        let _ = output.push_str(&rest[..index]);
        let _ = output.push_str("んなふう");
        rest = &rest[index + "のふう".len()..];
    }
    let _ = output.push_str(rest);
    output
}

pub(crate) fn emitted_text_len(input: &str) -> usize {
    text_operations(input)
        .unwrap_or_default()
        .into_iter()
        .map(|operation| match operation {
            TextOperation::Text(text) => text.len(),
            TextOperation::Left => 0,
        })
        .sum()
}

pub(crate) fn emitted_text_ends_with_space(input: &str) -> bool {
    let mut last = None;
    for operation in text_operations(input).unwrap_or_default() {
        if let TextOperation::Text(text) = operation {
            last = text.bytes().last().or(last);
        }
    }
    last == Some(b' ')
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Part {
    pub(crate) conso: String<16>,
    pub(crate) vowel: String<16>,
    pub(crate) particle: String<16>,
}

pub(crate) fn split_id(id: &str) -> (&str, &str) {
    id.split_once('-').unwrap_or((id, ""))
}

fn abbreviation_side(
    table: &'static [(&'static str, &'static str)],
    stroke: &str,
) -> Option<&'static str> {
    table
        .iter()
        .find(|(key, _)| *key == stroke)
        .map(|(_, value)| *value)
}

pub(crate) fn parse_part(input: &str) -> Part {
    let mut part = Part::default();
    let mut phase = 0u8;
    for ch in input.chars() {
        match ch {
            'S' | 'T' | 'K' | 'N' if phase == 0 => {
                let _ = part.conso.push(ch);
            }
            'Y' | 'I' | 'A' | 'U' if phase <= 1 => {
                phase = 1;
                let _ = part.vowel.push(ch);
            }
            'n' | 't' | 'k' | '#' => {
                phase = 2;
                if ch != '#' {
                    let _ = part.particle.push(ch);
                }
            }
            _ => {}
        }
    }
    part
}

fn consonant_roma(stroke: &str) -> Option<&'static str> {
    CONSONANTS
        .iter()
        .find(|(key, _)| *key == stroke)
        .map(|(_, roma)| *roma)
}

fn vowel_index(stroke: &str) -> Option<usize> {
    VOWELS
        .iter()
        .find(|(key, _, _)| *key == stroke)
        .map(|(_, _, index)| *index)
}

fn vowel_roma_index(roma: &str) -> usize {
    VOWELS
        .iter()
        .find(|(_, value, _)| *value == roma)
        .map(|(_, _, index)| *index)
        .unwrap_or(0)
}

fn triplet(
    table: &[(&'static str, &'static str, &'static str)],
    key: &str,
) -> Option<(&'static str, &'static str)> {
    table
        .iter()
        .find(|(stroke, _, _)| *stroke == key)
        .map(|(_, first, suffix)| (*first, *suffix))
}

fn diphthong_suffix(stroke: &str) -> &'static str {
    DIPHTHONGS
        .iter()
        .find(|(key, _, _)| *key == stroke)
        .map(|(_, _, suffix)| *suffix)
        .unwrap_or("")
}

fn has_english_or_minor_diphthong(vowel: &str, particle: &str) -> bool {
    let mut key = String::<32>::new();
    let _ = key.push_str(vowel);
    let _ = key.push_str(particle);
    triplet(&ENGLISH_DIPHTHONGS, key.as_str()).is_some()
        || triplet(&MINOR_DIPHTHONGS, key.as_str()).is_some()
}

fn convert_to_kana(conso: &str, vowel: &str, particle: &str, include_extra: bool) -> KanaText {
    let mut result = KanaText::new();
    if conso.is_empty() && vowel.is_empty() {
        if include_extra {
            let _ = result.push_str(second_sound(particle));
        }
        return result;
    }

    if conso == "STN" && vowel.is_empty() {
        if include_extra {
            let _ = result.push_str(second_sound(particle));
        }
        return result;
    }

    let mut conso_vowel = String::<32>::new();
    let _ = conso_vowel.push_str(conso);
    let _ = conso_vowel.push_str(vowel);

    let exception = EXCEPTION_KANA
        .iter()
        .find(|(stroke, _)| *stroke == conso_vowel.as_str())
        .map(|(_, kana)| *kana);
    let prefer_exception = matches!(particle, "n" | "tk" | "ntk");

    if prefer_exception {
        if let Some(exception) = exception {
            let _ = result.push_str(adjusted_exception_kana(
                conso_vowel.as_str(),
                particle,
                exception,
            ));
            if include_extra {
                let _ = result.push_str(second_sound(particle));
            }
            return result;
        }
    }

    let mut vowel_particle = String::<32>::new();
    let _ = vowel_particle.push_str(vowel);
    let _ = vowel_particle.push_str(particle);

    if let Some((first, suffix)) = triplet(&ENGLISH_DIPHTHONGS, vowel_particle.as_str()) {
        let c_index = consonant_roma(conso)
            .and_then(|roma| ROMA_ORDER.iter().position(|value| *value == roma))
            .unwrap_or(0);
        let mut base = KANA[c_index][vowel_roma_index(first)];
        base = match base {
            "ち" => "てぃ",
            "ぢ" => "でぃ",
            "づ" => "どぅ",
            "ぁ" => "すた",
            "ぃ" => "すち",
            "ぅ" => "すてぃ",
            "ぇ" => "すて",
            "ぉ" => "すと",
            value => value,
        };
        let _ = result.push_str(base);
        let _ = result.push_str(suffix);
        if result.as_str() == "るしょん" {
            result.clear();
            let _ = result.push_str("りゅーしょん");
        } else if result.as_str() == "ふしょん" {
            result.clear();
            let _ = result.push_str("ふゅーじょん");
        }
        return result;
    }

    if let Some((first, suffix)) = triplet(&MINOR_DIPHTHONGS, vowel_particle.as_str()) {
        let c_index = consonant_roma(conso)
            .and_then(|roma| ROMA_ORDER.iter().position(|value| *value == roma))
            .unwrap_or(0);
        let _ = result.push_str(KANA[c_index][vowel_roma_index(first)]);
        let _ = result.push_str(suffix);
        return result;
    }

    if let Some(exception) = exception {
        let _ = result.push_str(adjusted_exception_kana(
            conso_vowel.as_str(),
            particle,
            exception,
        ));
        if include_extra {
            let _ = result.push_str(second_sound(particle));
        }
        return result;
    }

    let c_index = consonant_roma(conso)
        .and_then(|roma| ROMA_ORDER.iter().position(|value| *value == roma))
        .unwrap_or(0);
    let v_index = vowel_index(vowel)
        .or_else(|| {
            DIPHTHONGS
                .iter()
                .find(|(key, _, _)| *key == vowel)
                .map(|(_, first, _)| vowel_roma_index(first))
        })
        .unwrap_or(0);
    let _ = result.push_str(KANA[c_index][v_index]);
    let _ = result.push_str(diphthong_suffix(vowel));

    if include_extra {
        let _ = result.push_str(second_sound(particle));
    }
    result
}

fn adjusted_exception_kana(
    conso_vowel: &str,
    particle: &str,
    exception: &'static str,
) -> &'static str {
    if particle == "tk" {
        match conso_vowel {
            "SKIAU" => "ちぇ",
            "STKNIAU" => "じぇ",
            "STNIAU" => "しぇ",
            "TNYIAU" => "いぇ",
            _ => exception,
        }
    } else {
        exception
    }
}

fn second_sound(particle: &str) -> &'static str {
    match particle {
        "n" => "ん",
        "t" => "つ",
        "k" => "く",
        "tk" => "っ",
        "nt" => "ち",
        "nk" => "き",
        "ntk" => "ー",
        _ => "",
    }
}

fn transform_particles(left: &str, right: &str, output: &mut KanaText, previous: &mut String<16>) {
    let right_tk = right
        .find('n')
        .map(|index| &right[index + 1..])
        .unwrap_or(right);
    let has_comma = right.contains('n');

    let Some(left_joshi) = particle_left(left) else {
        return;
    };
    let Some(right_joshi) = particle_right(right_tk) else {
        return;
    };
    let Some(right_raw_joshi) = particle_right(right) else {
        return;
    };

    if (left == "n" || left.is_empty()) && right == "ntk" {
        let _ = output.push_str(right_raw_joshi);
        if left == "n" {
            let _ = output.push('、');
        }
    } else if left == "n" && !right.is_empty() {
        let _ = output.push_str(right_joshi);
        let _ = output.push('、');
    } else if !left.is_empty() && (right == "k" || right == "nk") {
        if left == "nt" || left == "ntk" {
            let _ = output.push_str(left_joshi);
            let _ = output.push('の');
        } else {
            let _ = output.push('の');
            let _ = output.push_str(left_joshi);
        }
        if output.as_str() == "のの" {
            output.clear();
            if previous.as_str() == "な" {
                let _ = output.push_str("のが");
            } else {
                let _ = output.push('な');
            }
        }
        if has_comma {
            let _ = output.push('、');
        }
    } else {
        let _ = output.push_str(left_joshi);
        let _ = output.push_str(right_joshi);
        if has_comma {
            let _ = output.push('、');
        }
    }

    previous.clear();
    let _ = previous.push_str(output.as_str());
}

fn particle_left(particle: &str) -> Option<&'static str> {
    match particle {
        "" => Some(""),
        "n" => Some("、"),
        "t" => Some("に"),
        "k" => Some("の"),
        "tk" => Some("で"),
        "nt" => Some("と"),
        "nk" => Some("を"),
        "ntk" => Some("へ"),
        _ => None,
    }
}

fn replace_he_with_ka(output: &mut KanaText) {
    if !output.as_str().contains('へ') {
        return;
    }
    let mut replaced = KanaText::new();
    let mut rest = output.as_str();
    while let Some(index) = rest.find('へ') {
        let _ = replaced.push_str(&rest[..index]);
        let _ = replaced.push('か');
        rest = &rest['へ'.len_utf8() + index..];
    }
    let _ = replaced.push_str(rest);
    *output = replaced;
}

fn particle_right(particle: &str) -> Option<&'static str> {
    match particle {
        "" => Some(""),
        "n" => Some("、"),
        "t" => Some("は"),
        "k" => Some("が"),
        "tk" => Some("も"),
        "nt" => Some("は、"),
        "nk" => Some("が、"),
        "ntk" => Some("や"),
        _ => None,
    }
}

fn particle_command(left: &str, right: &str) -> Option<&'static str> {
    match (left, right) {
        ("", "nt") => Some("."),
        ("", "nk") => Some(","),
        ("n", "nt") => Some("?"),
        ("n", "nk") => Some("!"),
        _ => None,
    }
}
