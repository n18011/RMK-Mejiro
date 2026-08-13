//! Pure-Rust Mejiro chord handling.
//!
//! The QMK implementation receives a set of Gemini/Mejiro keys, serialises
//! the set in a stable left-to-right order, and then turns the stroke into
//! host-side key presses.  This module keeps that boundary free of RMK and
//! Embassy types so it can be tested on the host and reused by the firmware
//! controller.

use heapless::{String, Vec};

use crate::mejiro_verbs::{VerbType, VERB_DICTIONARY};

pub const MAX_CHORD_ID: usize = 64;
pub const MAX_OUTPUT: usize = 128;

pub type Text = String<MAX_OUTPUT>;

const LEFT_LABELS: [&str; 12] = ["#", "S", "T", "K", "N", "Y", "I", "A", "U", "n", "t", "k"];
const RIGHT_LABELS: [&str; 12] = ["S", "T", "K", "N", "Y", "I", "A", "U", "n", "t", "k", "*"];

/// The 24 physical keys used by the Mejiro/Gemini layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MejiroKey {
    LeftHash = 0,
    LeftS = 1,
    LeftT = 2,
    LeftK = 3,
    LeftN = 4,
    LeftY = 5,
    LeftI = 6,
    LeftA = 7,
    LeftU = 8,
    LeftNStroke = 9,
    LeftTStroke = 10,
    LeftKStroke = 11,
    RightS = 12,
    RightT = 13,
    RightK = 14,
    RightN = 15,
    RightY = 16,
    RightI = 17,
    RightA = 18,
    RightU = 19,
    RightNStroke = 20,
    RightTStroke = 21,
    RightKStroke = 22,
    RightStar = 23,
}

impl MejiroKey {
    pub const fn bit(self) -> u32 {
        1u32 << self as u8
    }

    pub const fn from_index(index: u8) -> Option<Self> {
        Some(match index {
            0 => Self::LeftHash,
            1 => Self::LeftS,
            2 => Self::LeftT,
            3 => Self::LeftK,
            4 => Self::LeftN,
            5 => Self::LeftY,
            6 => Self::LeftI,
            7 => Self::LeftA,
            8 => Self::LeftU,
            9 => Self::LeftNStroke,
            10 => Self::LeftTStroke,
            11 => Self::LeftKStroke,
            12 => Self::RightS,
            13 => Self::RightT,
            14 => Self::RightK,
            15 => Self::RightN,
            16 => Self::RightY,
            17 => Self::RightI,
            18 => Self::RightA,
            19 => Self::RightU,
            20 => Self::RightNStroke,
            21 => Self::RightTStroke,
            22 => Self::RightKStroke,
            23 => Self::RightStar,
            _ => return None,
        })
    }
}

/// A de-duplicated set of currently participating Mejiro keys.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Chord {
    bits: u32,
}

impl Chord {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub fn press(&mut self, key: MejiroKey) {
        self.bits |= key.bit();
    }

    pub fn release(&mut self, key: MejiroKey) {
        self.bits &= !key.bit();
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub const fn contains(self, key: MejiroKey) -> bool {
        self.bits & key.bit() != 0
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    /// Return the canonical Mejiro ID: left hand, separator, right hand.
    pub fn id(self) -> String<MAX_CHORD_ID> {
        let mut id = String::new();
        let left = self.bits & 0x0fff != 0;
        let right = self.bits & 0xfff000 != 0;

        for (index, label) in LEFT_LABELS.iter().enumerate() {
            if self.bits & (1u32 << index) != 0 {
                let _ = id.push_str(label);
            }
        }

        if left || right {
            let _ = id.push('-');
        }

        for (index, label) in RIGHT_LABELS.iter().enumerate() {
            if self.bits & (1u32 << (index + 12)) != 0 {
                let _ = id.push_str(label);
            }
        }

        id
    }
}

/// Actions that are sent as a single RMK key instead of text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAction {
    Backspace,
    Delete,
    Escape,
    Left,
    Down,
    Up,
    Right,
    Home,
    End,
    ShiftLeft,
    ShiftDown,
    ShiftUp,
    ShiftRight,
    ShiftHome,
    ShiftEnd,
    ShiftEnter,
    CtrlEnter,
    Enter,
    Space,
    Tab,
    Language1,
    Language2,
}

/// Result of committing one Mejiro chord.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrokeResult {
    Text { text: Text, kana_length: u8 },
    Key(KeyAction),
    Repeat,
    Undo,
    Unsupported,
}

impl StrokeResult {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }
    }

    pub const fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

#[derive(Clone, Debug)]
struct TransformState {
    last_vowel: String<16>,
    pending_tsu: bool,
    previous_particle: String<16>,
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

/// Stateful first-up chord accumulator used by the firmware controller.
#[derive(Clone, Debug)]
pub struct MejiroSession {
    chord: Chord,
    held: Chord,
    down_count: u8,
    previous_down_count: u8,
    chord_active: bool,
    chord_has_new_press: bool,
    first_up: bool,
    transform: TransformState,
    history: Vec<Text, 20>,
}

impl MejiroSession {
    pub fn new(first_up: bool) -> Self {
        Self {
            chord: Chord::new(),
            held: Chord::new(),
            down_count: 0,
            previous_down_count: 0,
            chord_active: false,
            chord_has_new_press: false,
            first_up,
            transform: TransformState::default(),
            history: Vec::new(),
        }
    }

    pub fn press(&mut self, key: MejiroKey) -> Option<StrokeResult> {
        self.held.press(key);
        self.down_count = self.down_count.saturating_add(1);

        if self.down_count > self.previous_down_count {
            self.chord_active = true;
            self.chord.press(key);
            self.chord_has_new_press = true;
        }
        self.previous_down_count = self.down_count;
        None
    }

    pub fn release(&mut self, key: MejiroKey) -> Option<StrokeResult> {
        self.held.release(key);
        self.down_count = self.down_count.saturating_sub(1);

        if self.first_up && !self.chord_has_new_press {
            self.reset_chord();
            self.seed_from_held();
        }

        let should_commit = if self.first_up {
            self.chord_has_new_press && self.down_count < self.previous_down_count
        } else {
            self.down_count == 0 && self.previous_down_count > 0
        };

        let result = if self.chord_active && should_commit && !self.chord.is_empty() {
            let id = self.chord.id();
            let result = transform_with_state(id.as_str(), &mut self.transform);
            if let StrokeResult::Text { text, .. } = &result {
                if !text.is_empty() {
                    let _ = self.history.push(text.clone());
                }
            }
            self.reset_chord();
            if self.first_up && self.down_count > 0 {
                self.seed_from_held();
            }
            Some(result)
        } else {
            None
        };

        self.previous_down_count = self.down_count;
        result
    }

    pub fn reset(&mut self) {
        self.held = Chord::new();
        self.down_count = 0;
        self.previous_down_count = 0;
        self.reset_chord();
        self.transform = TransformState::default();
        self.history.clear();
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn last_text(&self) -> Option<&str> {
        self.history.last().map(|text| text.as_str())
    }

    pub fn undo_last(&mut self) -> Option<usize> {
        self.history.pop().map(|text| text.len())
    }

    fn reset_chord(&mut self) {
        self.chord = Chord::new();
        self.chord_active = false;
        self.chord_has_new_press = false;
    }

    fn seed_from_held(&mut self) {
        if !self.held.is_empty() {
            self.chord = self.held;
            self.chord_active = true;
            self.chord_has_new_press = false;
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
    ("いぇ", "ye"),
    ("しぇ", "she"),
    ("じぇ", "je"),
    ("ちぇ", "che"),
    ("てぃ", "thi"),
    ("てゅ", "tyu"),
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
];

/// Convert hiragana to the same Hepburn-ish ASCII sequence used by QMK.
pub fn kana_to_romaji(input: &str) -> Text {
    let mut output = Text::new();
    let mut rest = input;

    while !rest.is_empty() && output.len() < MAX_OUTPUT.saturating_sub(10) {
        if rest.starts_with('っ') {
            let after = &rest['っ'.len_utf8()..];
            if let Some((_, roma)) = longest_kana_match(after) {
                let first = roma.as_bytes().first().copied().unwrap_or_default();
                if matches!(first, b'a' | b'e' | b'i' | b'o' | b'u') {
                    let _ = output.push_str("xtu");
                } else if first.is_ascii_alphabetic() {
                    let _ = output.push(first as char);
                } else {
                    let _ = output.push_str("xtu");
                }
            } else {
                let _ = output.push_str("xtu");
            }
            rest = after;
            continue;
        }

        if let Some((kana, roma)) = longest_kana_match(rest) {
            let _ = output.push_str(roma);
            rest = &rest[kana.len()..];
            continue;
        }

        if let Some(ch) = rest.chars().next() {
            if ch.is_ascii() {
                let _ = output.push(ch);
            }
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }

    output
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

fn transform_with_state(id: &str, state: &mut TransformState) -> StrokeResult {
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

    if id.contains('#') && !id.ends_with('*') {
        let mut normalized = String::<MAX_CHORD_ID>::new();
        for ch in id.chars().filter(|ch| *ch != '#') {
            let _ = normalized.push(ch);
        }
        let result = transform_with_state(normalized.as_str(), state);
        return repeat_result(result);
    }

    let has_asterisk = id.ends_with('*');
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
        if let (Some(left_output), Some(right_output)) = (
            abbreviation_side(ABSTRACT_LEFT, left_raw),
            abbreviation_side(ABSTRACT_RIGHT, right_raw),
        ) {
            let mut output = Text::new();
            let _ = output.push_str(left_output);
            let _ = output.push_str(right_output);
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
        let mut kana = Text::new();
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

    let left_final_tsu = left_has_sound
        && left_part.particle.as_str() == "tk"
        && !right_has_sound
        && !right_has_particle;
    let right_final_tsu =
        right_has_sound && right_part.particle.as_str() == "tk" && !left_has_particle;
    let left_plus_particle = left_has_sound && !right_has_sound && right_has_particle;
    let ntk_n = left_has_sound
        && left_part.particle.as_str() == "ntk"
        && !right_has_sound
        && right_part.particle.as_str() == "n";

    let mut kana = Text::new();
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
        let mut particles = Text::new();
        transform_particles(
            left_part.particle.as_str(),
            right_part.particle.as_str(),
            &mut particles,
            &mut state.previous_particle,
        );
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
    let _ = text.push_str(value);
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
    let form = conjugation_form(right.particle.as_str());

    let mut output = Text::new();
    if stroke.contains('-') {
        match base.as_str() {
            "I-K" => {
                let _ = output.push_str(IKU_FORMS[form]);
                let _ = output.push_str(conjugation_suffix(right.particle.as_str()));
                return Some(text_from_kana(output));
            }
            "A-" => {
                let _ = output.push_str(ARU_FORMS[form]);
                let _ = output.push_str(conjugation_suffix(right.particle.as_str()));
                return Some(text_from_kana(output));
            }
            "K-" => {
                let _ = output.push_str(KAHEN_FORMS[form]);
                let _ = output.push_str(conjugation_suffix(right.particle.as_str()));
                return Some(text_from_kana(output));
            }
            _ => {}
        }
    }

    if has_asterisk {
        if let Some(entry) = VERB_DICTIONARY
            .iter()
            .find(|entry| entry.stroke == base.as_str())
        {
            let _ = output.push_str(entry.stem);
            let ending = match entry.kind {
                VerbType::Godan => godan_ending(entry.row, form),
                VerbType::Kami => kami_ending(entry.row, form),
                VerbType::Simo => simo_ending(entry.row, form),
                VerbType::Kahen => KAHEN_FORMS[form],
                VerbType::Special => "",
            };
            let _ = output.push_str(ending);
            let _ = output.push_str(conjugation_suffix(right.particle.as_str()));
            return Some(text_from_kana(output));
        }
    }

    let left_has_sound = !left.conso.is_empty() || !left.vowel.is_empty();
    let right_has_sound = !right.conso.is_empty() || !right.vowel.is_empty();
    if !left_has_sound || !right_has_sound {
        return None;
    }

    let left_kana = convert_to_kana(left.conso.as_str(), left.vowel.as_str(), "", false);
    let right_kana = convert_to_kana(right.conso.as_str(), right.vowel.as_str(), "", false);

    if !left_has_sound && left.particle.is_empty() && right.vowel.as_str() == "I" {
        let row = kana_row(right_kana.as_str())?;
        let _ = output.push_str(kami_ending(row, form));
        let _ = output.push_str(conjugation_suffix(right.particle.as_str()));
        return Some(text_from_kana(output));
    }

    if !left_has_sound && left.particle.is_empty() && right.vowel.as_str() == "IA" {
        let row = kana_row(right_kana.as_str())?;
        let _ = output.push_str(simo_ending(row, form));
        let _ = output.push_str(conjugation_suffix(right.particle.as_str()));
        return Some(text_from_kana(output));
    }

    if right.vowel.is_empty() && !right.conso.is_empty() {
        let row = kana_row(right_kana.as_str()).or_else(|| consonant_row(right.conso.as_str()))?;
        let _ = output.push_str(left_kana.as_str());
        let _ = output.push_str(godan_ending(row, form));
        let _ = output.push_str(conjugation_suffix(right.particle.as_str()));
        return Some(text_from_kana(output));
    }

    None
}

const IKU_FORMS: [&str; 10] = [
    "いか",
    "いかさ",
    "いから",
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

fn conjugation_form(particle: &str) -> usize {
    match particle {
        "n" => 0,
        "t" => 5,
        "k" | "nk" | "tk" => 3,
        "nt" => 0,
        "ntk" => 5,
        _ => 4,
    }
}

fn conjugation_suffix(particle: &str) -> &'static str {
    match particle {
        "n" => "ない",
        "t" => "た",
        "k" => "ます",
        "nt" => "なかった",
        "nk" => "ません",
        "tk" => "ました",
        "ntk" => "て",
        _ => "",
    }
}

fn godan_ending(row: char, form: usize) -> &'static str {
    const ENDINGS: &[(char, [&str; 10])] = &[
        (
            'k',
            [
                "か", "かさ", "から", "き", "く", "い", "こう", "け", "け", "け",
            ],
        ),
        (
            'g',
            [
                "が", "がさ", "がら", "ぎ", "ぐ", "い", "ごう", "げ", "げ", "げ",
            ],
        ),
        (
            's',
            [
                "さ", "ささ", "さら", "し", "す", "し", "そう", "せ", "せ", "せ",
            ],
        ),
        (
            't',
            [
                "た", "たさ", "たら", "ち", "つ", "っ", "とう", "て", "て", "て",
            ],
        ),
        (
            'n',
            [
                "な", "なさ", "なら", "に", "ぬ", "ん", "のう", "ね", "ね", "ね",
            ],
        ),
        (
            'b',
            [
                "ば", "ばさ", "ばら", "び", "ぶ", "ん", "ぼう", "べ", "べ", "べ",
            ],
        ),
        (
            'm',
            [
                "ま", "まさ", "まら", "み", "む", "ん", "もう", "め", "め", "め",
            ],
        ),
        (
            'r',
            [
                "ら", "らさ", "ら", "り", "る", "っ", "ろう", "れ", "れ", "れ",
            ],
        ),
        (
            'w',
            [
                "わ", "わさ", "わら", "い", "う", "っ", "おう", "え", "え", "え",
            ],
        ),
    ];
    ENDINGS
        .iter()
        .find(|(candidate, _)| *candidate == row)
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
        .find(|(candidate, _)| *candidate == row)
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
        ("わを", 'w'),
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
            let _ = repeated.push_str(text.as_str());
            let _ = repeated.push_str(text.as_str());
            StrokeResult::Text {
                text: repeated,
                kana_length: kana_length.saturating_mul(2),
            }
        }
        other => other,
    }
}

fn text_from_kana_str(value: &str) -> StrokeResult {
    let mut kana = Text::new();
    let _ = kana.push_str(value);
    text_from_kana(kana)
}

fn text_from_kana(kana: Text) -> StrokeResult {
    let kana_length = kana.chars().count().min(u8::MAX as usize) as u8;
    let text = kana_to_romaji(kana.as_str());
    StrokeResult::Text { text, kana_length }
}

#[derive(Clone, Debug, Default)]
struct Part {
    conso: String<16>,
    vowel: String<16>,
    particle: String<16>,
}

fn split_id(id: &str) -> (&str, &str) {
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

fn parse_part(input: &str) -> Part {
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

fn convert_to_kana(conso: &str, vowel: &str, particle: &str, include_extra: bool) -> Text {
    let mut result = Text::new();
    if conso.is_empty() && vowel.is_empty() {
        if include_extra {
            let _ = result.push_str(second_sound(particle));
        }
        return result;
    }

    let mut conso_vowel = String::<32>::new();
    let _ = conso_vowel.push_str(conso);
    let _ = conso_vowel.push_str(vowel);

    if let Some(exception) = EXCEPTION_KANA
        .iter()
        .find(|(stroke, _)| *stroke == conso_vowel.as_str())
        .map(|(_, kana)| *kana)
    {
        let _ = result.push_str(exception);
        if include_extra {
            let _ = result.push_str(second_sound(particle));
        }
        return result;
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

fn transform_particles(left: &str, right: &str, output: &mut Text, previous: &mut String<16>) {
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
        "ntk" => Some("か"),
        _ => None,
    }
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
