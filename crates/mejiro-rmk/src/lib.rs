#![no_std]

//! RMK 0.8 adapter for the pure Mejiro session.

use rmk::channel::{KEYBOARD_REPORT_CHANNEL, MEJIRO_EVENT_CHANNEL};
use rmk::controller::Controller;
use rmk::descriptor::KeyboardReport;
use rmk::event::MejiroKeyEvent;
use rmk::heapless::String;
use rmk::hid::Report;
use rmk::types::keycode::{from_ascii, KeyCode};
use rmk::types::modifier::ModifierCombination;

use mejiro_core::mejiro::{
    key_action_to_hid, text_operations, HidKey, HidModifier, KeyAction, MejiroKey, MejiroSession,
    StrokeResult, TextOperation, MAX_OUTPUT,
};

pub const MEJIRO_KEY_COUNT: usize = 24;

/// Maps each of the 24 logical Mejiro keys to an RMK keycode.
///
/// The default map uses `Kb0` through `Kb23`, but another keyboard can use
/// any 24 codes from RMK's `Kb0` through `Kb31` range as long as the array
/// order matches `MejiroKey::from_index`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MejiroKeyMap {
    codes: [u16; MEJIRO_KEY_COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MejiroKeyMapError {
    NonVirtualKey(KeyCode),
    DuplicateKey(KeyCode),
}

impl MejiroKeyMap {
    /// Construct a map for a static keyboard definition.
    ///
    /// Prefer [`Self::try_new`] when the map comes from runtime or external
    /// configuration; this constructor remains `const` for RMK keymaps.
    pub const fn new(codes: [KeyCode; MEJIRO_KEY_COUNT]) -> Self {
        let mut raw_codes = [0; MEJIRO_KEY_COUNT];
        let mut index = 0;
        while index < MEJIRO_KEY_COUNT {
            raw_codes[index] = codes[index] as u16;
            index += 1;
        }
        Self { codes: raw_codes }
    }

    pub fn try_new(codes: [KeyCode; MEJIRO_KEY_COUNT]) -> Result<Self, MejiroKeyMapError> {
        let mut index = 0;
        while index < MEJIRO_KEY_COUNT {
            if !codes[index].is_kb() {
                return Err(MejiroKeyMapError::NonVirtualKey(codes[index]));
            }
            let mut previous = 0;
            while previous < index {
                if codes[previous] == codes[index] {
                    return Err(MejiroKeyMapError::DuplicateKey(codes[index]));
                }
                previous += 1;
            }
            index += 1;
        }
        Ok(Self::new(codes))
    }

    pub const fn default_kb() -> Self {
        Self::new([
            KeyCode::Kb0,
            KeyCode::Kb1,
            KeyCode::Kb2,
            KeyCode::Kb3,
            KeyCode::Kb4,
            KeyCode::Kb5,
            KeyCode::Kb6,
            KeyCode::Kb7,
            KeyCode::Kb8,
            KeyCode::Kb9,
            KeyCode::Kb10,
            KeyCode::Kb11,
            KeyCode::Kb12,
            KeyCode::Kb13,
            KeyCode::Kb14,
            KeyCode::Kb15,
            KeyCode::Kb16,
            KeyCode::Kb17,
            KeyCode::Kb18,
            KeyCode::Kb19,
            KeyCode::Kb20,
            KeyCode::Kb21,
            KeyCode::Kb22,
            KeyCode::Kb23,
        ])
    }

    pub fn resolve(&self, keycode: KeyCode) -> Option<MejiroKey> {
        if !keycode.is_kb() {
            return None;
        }

        let raw_keycode = keycode as u16;
        self.codes
            .iter()
            .position(|code| *code == raw_keycode)
            .and_then(|index| MejiroKey::from_index(index as u8))
    }
}

impl Default for MejiroKeyMap {
    fn default() -> Self {
        Self::default_kb()
    }
}

pub struct MejiroController {
    session: MejiroSession,
    keymap: MejiroKeyMap,
}

impl Default for MejiroController {
    fn default() -> Self {
        Self::new()
    }
}

impl MejiroController {
    pub fn new() -> Self {
        Self::with_keymap(MejiroKeyMap::default_kb())
    }

    pub fn with_keymap(keymap: MejiroKeyMap) -> Self {
        Self {
            session: MejiroSession::new(true),
            keymap,
        }
    }

    async fn handle(&mut self, event: MejiroKeyEvent) {
        let Some(key) = self.keymap.resolve(event.keycode) else {
            return;
        };

        let result = if event.pressed {
            self.session.press(key)
        } else {
            self.session.release(key)
        };

        if let Some(result) = result {
            self.emit(result).await;
        }
    }

    async fn emit(&mut self, result: StrokeResult) {
        match result {
            StrokeResult::Text { text, .. } => self.send_text(text.as_str()).await,
            StrokeResult::Key(action) => {
                let (keycode, modifiers) = key_action(action);
                self.send_tap(keycode, modifiers).await;
            }
            StrokeResult::RepeatKey(action) => {
                let (keycode, modifiers) = key_action(action);
                self.send_tap(keycode, modifiers).await;
                self.send_tap(keycode, modifiers).await;
            }
            StrokeResult::Repeat => {
                let text = self.session.last_text().and_then(|value| {
                    let mut copy = String::<MAX_OUTPUT>::new();
                    copy.push_str(value).ok()?;
                    Some(copy)
                });
                if let Some(text) = text {
                    self.send_text(text.as_str()).await;
                }
            }
            StrokeResult::Undo => {
                if let Some(length) = self.session.undo_last() {
                    self.send_backspaces(length).await;
                }
            }
            StrokeResult::Noop => {}
            StrokeResult::Unsupported => {}
            StrokeResult::Truncated => {}
        }
    }

    async fn send_text(&self, text: &str) {
        let Ok(operations) = text_operations(text) else {
            return;
        };
        for operation in operations {
            match operation {
                TextOperation::Text(value) => self.send_ascii(value.as_str()).await,
                TextOperation::Left => {
                    self.send_tap(KeyCode::Left, ModifierCombination::new())
                        .await
                }
            }
        }
    }

    async fn send_ascii(&self, text: &str) {
        for byte in text.bytes() {
            let (keycode, shifted) = from_ascii(byte);
            if keycode == KeyCode::No {
                continue;
            }
            let modifiers = if shifted {
                ModifierCombination::LSHIFT
            } else {
                ModifierCombination::new()
            };
            self.send_tap(keycode, modifiers).await;
        }
    }

    async fn send_backspaces(&self, count: usize) {
        for _ in 0..count {
            self.send_tap(KeyCode::Backspace, ModifierCombination::new())
                .await;
        }
    }

    async fn send_tap(&self, keycode: KeyCode, modifiers: ModifierCombination) {
        let mut keycodes = [0; 6];
        keycodes[0] = keycode as u16 as u8;
        let pressed = KeyboardReport {
            modifier: modifiers.into_bits(),
            keycodes,
            ..Default::default()
        };
        KEYBOARD_REPORT_CHANNEL
            .send(Report::KeyboardReport(pressed))
            .await;

        KEYBOARD_REPORT_CHANNEL
            .send(Report::KeyboardReport(KeyboardReport::default()))
            .await;
    }
}

impl Controller for MejiroController {
    type Event = MejiroKeyEvent;

    async fn process_event(&mut self, event: Self::Event) {
        self.handle(event).await;
    }

    async fn next_message(&mut self) -> Self::Event {
        MEJIRO_EVENT_CHANNEL.receive().await
    }
}

fn key_action(action: KeyAction) -> (KeyCode, ModifierCombination) {
    let hid = key_action_to_hid(action);
    let keycode = match hid.key {
        HidKey::Backspace => KeyCode::Backspace,
        HidKey::Delete => KeyCode::Delete,
        HidKey::Escape => KeyCode::Escape,
        HidKey::Left => KeyCode::Left,
        HidKey::Down => KeyCode::Down,
        HidKey::Up => KeyCode::Up,
        HidKey::Right => KeyCode::Right,
        HidKey::Home => KeyCode::Home,
        HidKey::End => KeyCode::End,
        HidKey::Enter => KeyCode::Enter,
        HidKey::Space => KeyCode::Space,
        HidKey::Tab => KeyCode::Tab,
        HidKey::Language1 => KeyCode::Language1,
        HidKey::Language2 => KeyCode::Language2,
    };
    let modifiers = match hid.modifier {
        HidModifier::None => ModifierCombination::new(),
        HidModifier::Shift => ModifierCombination::LSHIFT,
        HidModifier::Ctrl => ModifierCombination::LCTRL,
    };
    (keycode, modifiers)
}

#[cfg(test)]
mod tests {
    use rmk::controller::Controller;
    use rmk::event::MejiroKeyEvent;
    use rmk::hid::Report;

    use super::{
        KeyCode, MejiroController, MejiroKey, MejiroKeyMap, MejiroKeyMapError, StrokeResult,
        KEYBOARD_REPORT_CHANNEL,
    };

    #[test]
    fn default_map_preserves_kb_contract() {
        let map = MejiroKeyMap::default();

        assert_eq!(map.resolve(KeyCode::Kb0), Some(MejiroKey::LeftHash));
        assert_eq!(map.resolve(KeyCode::Kb23), Some(MejiroKey::RightStar));
    }

    #[test]
    fn custom_map_accepts_other_rmk_virtual_keycodes() {
        let map = MejiroKeyMap::new([
            KeyCode::Kb31,
            KeyCode::Kb30,
            KeyCode::Kb29,
            KeyCode::Kb28,
            KeyCode::Kb27,
            KeyCode::Kb26,
            KeyCode::Kb25,
            KeyCode::Kb24,
            KeyCode::Kb23,
            KeyCode::Kb22,
            KeyCode::Kb21,
            KeyCode::Kb20,
            KeyCode::Kb19,
            KeyCode::Kb18,
            KeyCode::Kb17,
            KeyCode::Kb16,
            KeyCode::Kb15,
            KeyCode::Kb14,
            KeyCode::Kb13,
            KeyCode::Kb12,
            KeyCode::Kb11,
            KeyCode::Kb10,
            KeyCode::Kb9,
            KeyCode::Kb8,
        ]);

        assert_eq!(map.resolve(KeyCode::Kb31), Some(MejiroKey::LeftHash));
        assert_eq!(map.resolve(KeyCode::Kb9), Some(MejiroKey::RightKStroke));
        assert_eq!(map.resolve(KeyCode::Kb8), Some(MejiroKey::RightStar));
        assert_eq!(map.resolve(KeyCode::Kb0), None);
        assert_eq!(map.resolve(KeyCode::A), None);
    }

    #[test]
    fn validated_map_rejects_non_virtual_and_duplicate_keycodes() {
        let mut codes = [
            KeyCode::Kb0,
            KeyCode::Kb1,
            KeyCode::Kb2,
            KeyCode::Kb3,
            KeyCode::Kb4,
            KeyCode::Kb5,
            KeyCode::Kb6,
            KeyCode::Kb7,
            KeyCode::Kb8,
            KeyCode::Kb9,
            KeyCode::Kb10,
            KeyCode::Kb11,
            KeyCode::Kb12,
            KeyCode::Kb13,
            KeyCode::Kb14,
            KeyCode::Kb15,
            KeyCode::Kb16,
            KeyCode::Kb17,
            KeyCode::Kb18,
            KeyCode::Kb19,
            KeyCode::Kb20,
            KeyCode::Kb21,
            KeyCode::Kb22,
            KeyCode::Kb23,
        ];
        codes[23] = KeyCode::Kb22;
        assert_eq!(
            MejiroKeyMap::try_new(codes),
            Err(MejiroKeyMapError::DuplicateKey(KeyCode::Kb22))
        );

        codes[23] = KeyCode::A;
        assert_eq!(
            MejiroKeyMap::try_new(codes),
            Err(MejiroKeyMapError::NonVirtualKey(KeyCode::A))
        );
    }

    fn test_keymap() -> MejiroKeyMap {
        MejiroKeyMap::new([
            KeyCode::Kb0,
            KeyCode::Kb1,
            KeyCode::Kb2,
            KeyCode::Kb3,
            KeyCode::Kb4,
            KeyCode::Kb5,
            KeyCode::Kb6,
            KeyCode::Kb31,
            KeyCode::Kb8,
            KeyCode::Kb9,
            KeyCode::Kb10,
            KeyCode::Kb11,
            KeyCode::Kb12,
            KeyCode::Kb13,
            KeyCode::Kb14,
            KeyCode::Kb15,
            KeyCode::Kb16,
            KeyCode::Kb17,
            KeyCode::Kb18,
            KeyCode::Kb19,
            KeyCode::Kb20,
            KeyCode::Kb21,
            KeyCode::Kb22,
            KeyCode::Kb23,
        ])
    }

    fn event(keycode: KeyCode, pressed: bool) -> MejiroKeyEvent {
        MejiroKeyEvent {
            row: 0,
            col: 0,
            pressed,
            keycode,
        }
    }

    async fn assert_report(modifier: u8, keycode: KeyCode) {
        match KEYBOARD_REPORT_CHANNEL.receive().await {
            Report::KeyboardReport(report) => {
                assert_eq!(
                    (report.modifier, report.keycodes[0]),
                    (modifier, keycode as u16 as u8)
                );
                assert_eq!(report.keycodes[1..], [0; 5]);
            }
            report => panic!("expected keyboard report, got {report:?}"),
        }
    }

    async fn assert_no_reports() {
        assert!(KEYBOARD_REPORT_CHANNEL.try_receive().is_err());
    }

    #[test]
    fn controller_event_paths_emit_expected_hid_reports() {
        rmk::embassy_futures::block_on(async {
            KEYBOARD_REPORT_CHANNEL.clear();
            let mut controller = MejiroController::with_keymap(test_keymap());

            // Kb31 is remapped to LeftA and must emit the same HID text as the
            // default Kb7 mapping would emit.
            controller.process_event(event(KeyCode::Kb31, true)).await;
            controller.process_event(event(KeyCode::Kb31, false)).await;
            assert_report(0, KeyCode::A).await;
            assert_report(0, KeyCode::No).await;

            // A text operation containing {#Left} must preserve the cursor
            // movement after the preceding ASCII text.
            for keycode in [KeyCode::Kb12, KeyCode::Kb16, KeyCode::Kb18] {
                controller.process_event(event(keycode, true)).await;
            }
            for keycode in [KeyCode::Kb12, KeyCode::Kb16, KeyCode::Kb18] {
                controller.process_event(event(keycode, false)).await;
            }
            assert_report(2, KeyCode::Quote).await;
            assert_report(0, KeyCode::No).await;
            assert_report(2, KeyCode::Quote).await;
            assert_report(0, KeyCode::No).await;
            assert_report(0, KeyCode::Left).await;
            assert_report(0, KeyCode::No).await;

            // Repeat re-emits the last text, while Undo emits the recorded
            // ASCII length as backspaces.
            controller.process_event(event(KeyCode::Kb0, true)).await;
            controller.process_event(event(KeyCode::Kb0, false)).await;
            assert_report(2, KeyCode::Quote).await;
            assert_report(0, KeyCode::No).await;
            assert_report(2, KeyCode::Quote).await;
            assert_report(0, KeyCode::No).await;
            assert_report(0, KeyCode::Left).await;
            assert_report(0, KeyCode::No).await;
            controller.process_event(event(KeyCode::Kb19, true)).await;
            controller.process_event(event(KeyCode::Kb19, false)).await;
            assert_report(0, KeyCode::Backspace).await;
            assert_report(0, KeyCode::No).await;
            assert_report(0, KeyCode::Backspace).await;
            assert_report(0, KeyCode::No).await;

            controller.emit(StrokeResult::Unsupported).await;
            controller.emit(StrokeResult::Truncated).await;
            assert_no_reports().await;
        });
    }
}
