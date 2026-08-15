//! Pure-Rust Mejiro chord handling.
//!
//! The QMK implementation receives a set of Gemini/Mejiro keys, serialises
//! the set in a stable left-to-right order, and then turns the stroke into
//! host-side key presses.  This module keeps that boundary free of RMK and
//! Embassy types so it can be tested on the host and reused by the firmware
//! controller.

use heapless::{String, Vec};

pub use crate::mejiro_output::{
    text_operations, OutputError, Text, TextOperation, LEFT_TOKEN, MAX_OUTPUT,
};
use crate::mejiro_transform::{
    emitted_text_ends_with_space, emitted_text_len, parse_part, split_id, transform_with_state,
    TransformState,
};
pub use crate::mejiro_transform::{kana_to_romaji, transform};

pub const MAX_CHORD_ID: usize = 64;

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

/// The small, RMK-independent HID contract used by the firmware adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HidKey {
    Backspace,
    Delete,
    Escape,
    Left,
    Down,
    Up,
    Right,
    Home,
    End,
    Enter,
    Space,
    Tab,
    Language1,
    Language2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HidModifier {
    None,
    Shift,
    Ctrl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidAction {
    pub key: HidKey,
    pub modifier: HidModifier,
}

/// Map every non-text Mejiro action to the adapter's HID contract.
pub const fn key_action_to_hid(action: KeyAction) -> HidAction {
    let (key, modifier) = match action {
        KeyAction::Backspace => (HidKey::Backspace, HidModifier::None),
        KeyAction::Delete => (HidKey::Delete, HidModifier::None),
        KeyAction::Escape => (HidKey::Escape, HidModifier::None),
        KeyAction::Left => (HidKey::Left, HidModifier::None),
        KeyAction::Down => (HidKey::Down, HidModifier::None),
        KeyAction::Up => (HidKey::Up, HidModifier::None),
        KeyAction::Right => (HidKey::Right, HidModifier::None),
        KeyAction::Home => (HidKey::Home, HidModifier::None),
        KeyAction::End => (HidKey::End, HidModifier::None),
        KeyAction::ShiftLeft => (HidKey::Left, HidModifier::Shift),
        KeyAction::ShiftDown => (HidKey::Down, HidModifier::Shift),
        KeyAction::ShiftUp => (HidKey::Up, HidModifier::Shift),
        KeyAction::ShiftRight => (HidKey::Right, HidModifier::Shift),
        KeyAction::ShiftHome => (HidKey::Home, HidModifier::Shift),
        KeyAction::ShiftEnd => (HidKey::End, HidModifier::Shift),
        KeyAction::ShiftEnter => (HidKey::Enter, HidModifier::Shift),
        KeyAction::CtrlEnter => (HidKey::Enter, HidModifier::Ctrl),
        KeyAction::Enter => (HidKey::Enter, HidModifier::None),
        KeyAction::Space => (HidKey::Space, HidModifier::None),
        KeyAction::Tab => (HidKey::Tab, HidModifier::None),
        KeyAction::Language1 => (HidKey::Language1, HidModifier::None),
        KeyAction::Language2 => (HidKey::Language2, HidModifier::None),
    };
    HidAction { key, modifier }
}

/// Result of committing one Mejiro chord.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrokeResult {
    Text { text: Text, kana_length: u8 },
    Key(KeyAction),
    RepeatKey(KeyAction),
    Repeat,
    Undo,
    Noop,
    Unsupported,
    Truncated,
}

impl StrokeResult {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }
    }

    pub const fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported | Self::Truncated)
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
    history: Vec<HistoryEntry, 20>,
    last_output_was_space: bool,
    macros: MacroState,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    text: Text,
    length: usize,
}

const MACRO_KEYS: [&str; 7] = ["n", "t", "k", "nt", "nk", "tk", "ntk"];

#[derive(Clone, Debug)]
struct MacroState {
    values: [Text; 7],
    active: [bool; 7],
    truncated: [bool; 7],
    order: Vec<u8, 7>,
}

impl Default for MacroState {
    fn default() -> Self {
        Self {
            values: core::array::from_fn(|_| Text::new()),
            active: [false; 7],
            truncated: [false; 7],
            order: Vec::new(),
        }
    }
}

impl MacroState {
    fn stop_recording(&mut self) {
        self.order.clear();
        self.active = [false; 7];
    }
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
            last_output_was_space: false,
            macros: MacroState::default(),
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
            let result = self
                .macro_result(id.as_str())
                .unwrap_or_else(|| transform_with_state(id.as_str(), &mut self.transform));
            self.update_history_for_result(&result);
            self.record_in_active_macros(&result);
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
        self.last_output_was_space = false;
        self.macros.stop_recording();
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn last_text(&self) -> Option<&str> {
        self.history.last().map(|entry| entry.text.as_str())
    }

    pub fn undo_last(&mut self) -> Option<usize> {
        self.transform.pending_tsu = false;
        self.last_output_was_space = false;
        Some(self.history.pop().map_or(2, |entry| entry.length))
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

    fn update_history_for_result(&mut self, result: &StrokeResult) {
        match result {
            StrokeResult::Text { text, .. } if !text.is_empty() => {
                self.push_history(text.clone());
                self.last_output_was_space = emitted_text_ends_with_space(text.as_str());
            }
            StrokeResult::Repeat => {
                if let Some(entry) = self.history.last().cloned() {
                    self.push_history_entry(entry);
                    self.last_output_was_space = self
                        .history
                        .last()
                        .is_some_and(|entry| emitted_text_ends_with_space(entry.text.as_str()));
                }
            }
            StrokeResult::Key(KeyAction::Backspace) => {
                self.transform.pending_tsu = false;
                if self.last_output_was_space {
                    self.last_output_was_space = false;
                    return;
                }
                if let Some(entry) = self.history.last_mut() {
                    entry.length = entry.length.saturating_sub(1);
                    if entry.length == 0 {
                        let _ = self.history.pop();
                    }
                }
                self.last_output_was_space = false;
            }
            StrokeResult::Key(KeyAction::Delete) => {
                self.transform.pending_tsu = false;
                self.last_output_was_space = false;
            }
            StrokeResult::Key(KeyAction::Space) => {
                self.last_output_was_space = true;
            }
            StrokeResult::Key(_) => {
                self.last_output_was_space = false;
            }
            StrokeResult::RepeatKey(action) => {
                self.last_output_was_space = matches!(action, KeyAction::Space);
            }
            StrokeResult::Text { .. } => {}
            StrokeResult::Undo
            | StrokeResult::Noop
            | StrokeResult::Unsupported
            | StrokeResult::Truncated => {}
        }
    }

    fn push_history(&mut self, text: Text) {
        self.push_history_entry(HistoryEntry {
            length: emitted_text_len(text.as_str()),
            text,
        });
    }

    fn push_history_entry(&mut self, entry: HistoryEntry) {
        if self.history.len() == 20 {
            for index in 1..self.history.len() {
                self.history[index - 1] = self.history[index].clone();
            }
            let _ = self.history.pop();
        }
        let _ = self.history.push(entry);
    }

    fn macro_result(&mut self, id: &str) -> Option<StrokeResult> {
        let has_hash = id.contains('#');
        let has_asterisk = id.contains('*');
        let (left_raw, right_raw) = split_id(id);
        let left = parse_part(left_raw);
        let right = parse_part(right_raw);
        let left_has_sound = !left.conso.is_empty() || !left.vowel.is_empty();
        let right_has_sound = !right.conso.is_empty() || !right.vowel.is_empty();
        let pure_left_particle = !left_has_sound
            && !left.particle.is_empty()
            && !right_has_sound
            && right.particle.is_empty();

        if has_hash && has_asterisk && pure_left_particle {
            if let Some(index) = MACRO_KEYS
                .iter()
                .position(|key| *key == left.particle.as_str())
            {
                if self.macros.active[index] {
                    self.macros.active[index] = false;
                    self.remove_macro_order(index as u8);
                } else {
                    self.macros.values[index].clear();
                    self.macros.truncated[index] = false;
                    self.macros.active[index] = true;
                    self.push_macro_order(index as u8);
                }
                return Some(StrokeResult::Noop);
            }
        }

        let generic_stop = has_hash
            && has_asterisk
            && !left_has_sound
            && left.particle.is_empty()
            && !right_has_sound
            && right.particle.is_empty();
        if generic_stop {
            if let Some(index) = self.macros.order.pop() {
                self.macros.active[index as usize] = false;
            }
            return Some(StrokeResult::Noop);
        }

        if has_hash && !has_asterisk && pure_left_particle {
            if let Some(index) = MACRO_KEYS
                .iter()
                .position(|key| *key == left.particle.as_str())
            {
                if self.macros.truncated[index] {
                    return Some(StrokeResult::Truncated);
                }
                if self.macros.values[index].is_empty() {
                    return Some(StrokeResult::Noop);
                }
                return Some(StrokeResult::Text {
                    text: self.macros.values[index].clone(),
                    kana_length: 0,
                });
            }
        }

        None
    }

    fn record_in_active_macros(&mut self, result: &StrokeResult) {
        let mut text = Text::new();
        match result {
            StrokeResult::Text { text: value, .. } => {
                if text.push_str(value.as_str()).is_err() {
                    return;
                }
            }
            StrokeResult::Repeat => {
                if let Some(value) = self.history.last() {
                    if text.push_str(value.text.as_str()).is_err() {
                        return;
                    }
                }
            }
            _ => return,
        }
        if text.is_empty() {
            return;
        }
        for index in 0..MACRO_KEYS.len() {
            if self.macros.active[index] {
                let remaining = self.macros.values[index]
                    .capacity()
                    .saturating_sub(self.macros.values[index].len());
                if remaining < text.len()
                    || self.macros.values[index].push_str(text.as_str()).is_err()
                {
                    self.macros.truncated[index] = true;
                }
            }
        }
    }

    fn push_macro_order(&mut self, index: u8) {
        self.remove_macro_order(index);
        let _ = self.macros.order.push(index);
    }

    fn remove_macro_order(&mut self, index: u8) {
        if let Some(position) = self.macros.order.iter().position(|value| *value == index) {
            for cursor in position + 1..self.macros.order.len() {
                self.macros.order[cursor - 1] = self.macros.order[cursor];
            }
            let _ = self.macros.order.pop();
        }
    }
}
