#![no_std]
#![allow(async_fn_in_trait)]

//! RMK 0.9 adapter for the pure Mejiro session.
//!
//! RMK's `ActionEvent` is the integration boundary: the keyboard core has
//! already resolved the physical key through the active keymap, and this
//! adapter consumes only `Action::User(n)` events assigned to Mejiro.

#[cfg(test)]
extern crate std;

use rmk::event::{ActionEvent, ConnectionStatusChangeEvent, ConnectionType, KeyboardEventPos};
use rmk::heapless::String;
use rmk::hid::{KeyboardReport, Report};
use rmk::macros::processor;
use rmk::types::action::Action;
use rmk::types::keycode::{from_ascii, HidKeyCode};
use rmk::types::modifier::ModifierCombination;

use mejiro_core::mejiro::{
    key_action_to_hid, text_operations, HidKey, HidModifier, KeyAction, MejiroKey, MejiroSession,
    StrokeResult, TextOperation, MAX_OUTPUT,
};

pub const MEJIRO_KEY_COUNT: usize = 24;
pub const RMK_USER_KEY_COUNT: u8 = 32;

/// Output boundary for an RMK transport.
///
/// RMK 0.9 has separate USB and BLE report channels. The adapter deliberately
/// does not choose one: the firmware supplies a sink that routes a report to
/// the currently active transport.
pub trait MejiroReportSink {
    async fn send(&mut self, transport: ConnectionType, report: Report);
}

/// Write a formatted line to the USB CDC debug logger.
#[cfg(feature = "usb-debug")]
#[macro_export]
macro_rules! usb_serial_print {
    ($($arg:tt)*) => {
        ::log::info!($($arg)*);
    };
}

#[cfg(not(feature = "usb-debug"))]
#[macro_export]
macro_rules! usb_serial_print {
    ($($arg:tt)*) => {};
}

/// Alias with an explicit line-oriented name for USB serial diagnostics.
#[macro_export]
macro_rules! usb_serial_println {
    ($($arg:tt)*) => {
        $crate::usb_serial_print!($($arg)*);
    };
}

/// Maps RMK `User0` through `User31` actions to the 24 logical Mejiro keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MejiroKeyMap {
    user_keys: [u8; MEJIRO_KEY_COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MejiroKeyMapError {
    InvalidUserKey(u8),
    DuplicateUserKey(u8),
}

impl MejiroKeyMap {
    /// Construct a map for a static keyboard definition.
    ///
    /// The array index is the Mejiro key index; each value is the RMK user-key
    /// number used in `keyboard.toml` (`User0` through `User31`). Prefer
    /// [`Self::try_new`] when the map comes from external configuration.
    pub const fn new(user_keys: [u8; MEJIRO_KEY_COUNT]) -> Self {
        Self { user_keys }
    }

    pub fn try_new(user_keys: [u8; MEJIRO_KEY_COUNT]) -> Result<Self, MejiroKeyMapError> {
        let mut index = 0;
        while index < MEJIRO_KEY_COUNT {
            if user_keys[index] >= RMK_USER_KEY_COUNT {
                return Err(MejiroKeyMapError::InvalidUserKey(user_keys[index]));
            }
            let mut previous = 0;
            while previous < index {
                if user_keys[previous] == user_keys[index] {
                    return Err(MejiroKeyMapError::DuplicateUserKey(user_keys[index]));
                }
                previous += 1;
            }
            index += 1;
        }
        Ok(Self::new(user_keys))
    }

    pub const fn default_user() -> Self {
        Self::new([
            8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
            30, 31,
        ])
    }

    pub fn resolve(&self, user_key: u8) -> Option<MejiroKey> {
        self.user_keys
            .iter()
            .position(|key| *key == user_key)
            .and_then(|index| MejiroKey::from_index(index as u8))
    }
}

impl Default for MejiroKeyMap {
    fn default() -> Self {
        Self::default_user()
    }
}

/// RMK 0.9 processor that feeds resolved Mejiro user-key actions into
/// [`mejiro_core`].
#[processor(subscribe = [ActionEvent, ConnectionStatusChangeEvent])]
pub struct MejiroProcessor<S: MejiroReportSink> {
    session: MejiroSession,
    keymap: MejiroKeyMap,
    output: S,
    transport: Option<ConnectionType>,
}

impl<S: MejiroReportSink> MejiroProcessor<S> {
    pub fn new(output: S) -> Self {
        Self::with_keymap(output, MejiroKeyMap::default_user())
    }

    pub fn with_keymap(output: S, keymap: MejiroKeyMap) -> Self {
        Self {
            session: MejiroSession::new(true),
            keymap,
            output,
            transport: None,
        }
    }

    async fn on_connection_status_change_event(&mut self, event: ConnectionStatusChangeEvent) {
        self.transport = event.0.decide_active();
    }

    async fn on_action_event(&mut self, event: ActionEvent) {
        let Action::User(user_key) = event.action else {
            return;
        };

        // `ActionEvent` is also published for encoder actions. Mejiro is a
        // matrix-key engine, so only physical key positions participate.
        if !matches!(event.keyboard_event.pos, KeyboardEventPos::Key(_)) {
            return;
        }

        #[cfg(feature = "usb-debug")]
        usb_serial_println!(
            "mejiro action: user={}, pressed={}",
            user_key,
            event.keyboard_event.pressed
        );

        let Some(key) = self.keymap.resolve(user_key) else {
            return;
        };

        let result = if event.keyboard_event.pressed {
            self.session.press(key)
        } else {
            self.session.release(key)
        };

        if let Some(result) = result {
            #[cfg(feature = "usb-debug")]
            usb_serial_println!("mejiro result: {:?}", result);
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
            StrokeResult::Noop | StrokeResult::Unsupported | StrokeResult::Truncated => {}
        }
    }

    async fn send_text(&mut self, text: &str) {
        let Ok(operations) = text_operations(text) else {
            return;
        };
        for operation in operations {
            match operation {
                TextOperation::Text(value) => self.send_ascii(value.as_str()).await,
                TextOperation::Left => {
                    self.send_tap(HidKeyCode::Left, ModifierCombination::new())
                        .await
                }
            }
        }
    }

    async fn send_ascii(&mut self, text: &str) {
        for byte in text.bytes() {
            let (keycode, shifted) = from_ascii(byte);
            if keycode == HidKeyCode::No {
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

    async fn send_backspaces(&mut self, count: usize) {
        for _ in 0..count {
            self.send_tap(HidKeyCode::Backspace, ModifierCombination::new())
                .await;
        }
    }

    async fn send_tap(&mut self, keycode: HidKeyCode, modifiers: ModifierCombination) {
        let mut keycodes = [0; 6];
        keycodes[0] = keycode as u8;
        let pressed = KeyboardReport {
            modifier: modifiers.into_bits(),
            keycodes,
            ..Default::default()
        };
        self.send_report(Report::KeyboardReport(pressed)).await;
        self.send_report(Report::KeyboardReport(KeyboardReport::default()))
            .await;
    }

    async fn send_report(&mut self, report: Report) {
        if let Some(transport) = self.transport {
            self.output.send(transport, report).await;
        }
    }
}

fn key_action(action: KeyAction) -> (HidKeyCode, ModifierCombination) {
    let hid = key_action_to_hid(action);
    let keycode = match hid.key {
        HidKey::Backspace => HidKeyCode::Backspace,
        HidKey::Delete => HidKeyCode::Delete,
        HidKey::Escape => HidKeyCode::Escape,
        HidKey::Left => HidKeyCode::Left,
        HidKey::Down => HidKeyCode::Down,
        HidKey::Up => HidKeyCode::Up,
        HidKey::Right => HidKeyCode::Right,
        HidKey::Home => HidKeyCode::Home,
        HidKey::End => HidKeyCode::End,
        HidKey::Enter => HidKeyCode::Enter,
        HidKey::Space => HidKeyCode::Space,
        HidKey::Tab => HidKeyCode::Tab,
        HidKey::Language1 => HidKeyCode::Language1,
        HidKey::Language2 => HidKeyCode::Language2,
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
    use std::vec::Vec;

    use rmk::event::{ActionEvent, ConnectionStatus, KeyboardEvent};
    use rmk::hid::Report;
    use rmk::types::action::Action;
    use rmk::types::connection::UsbState;
    use rmk::types::keycode::{HidKeyCode, KeyCode};

    use super::{
        ConnectionStatusChangeEvent, ConnectionType, MejiroKey, MejiroKeyMap, MejiroKeyMapError,
        MejiroProcessor, MejiroReportSink,
    };

    #[derive(Default)]
    struct TestSink {
        transports: Vec<ConnectionType>,
        reports: Vec<Report>,
    }

    impl MejiroReportSink for TestSink {
        async fn send(&mut self, transport: ConnectionType, report: Report) {
            self.transports.push(transport);
            self.reports.push(report);
        }
    }

    #[test]
    fn default_map_preserves_user_key_contract() {
        let map = MejiroKeyMap::default();

        assert_eq!(map.resolve(8), Some(MejiroKey::LeftHash));
        assert_eq!(map.resolve(31), Some(MejiroKey::RightStar));
        assert_eq!(map.resolve(7), None);
    }

    #[test]
    fn custom_map_accepts_other_rmk_user_keys() {
        let map = MejiroKeyMap::new([
            31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10,
            9, 8,
        ]);

        assert_eq!(map.resolve(31), Some(MejiroKey::LeftHash));
        assert_eq!(map.resolve(9), Some(MejiroKey::RightKStroke));
        assert_eq!(map.resolve(8), Some(MejiroKey::RightStar));
        assert_eq!(map.resolve(0), None);
    }

    #[test]
    fn validated_map_rejects_invalid_and_duplicate_user_keys() {
        let mut keys = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
        ];
        keys[23] = 22;
        assert_eq!(
            MejiroKeyMap::try_new(keys),
            Err(MejiroKeyMapError::DuplicateUserKey(22))
        );

        keys[23] = 32;
        assert_eq!(
            MejiroKeyMap::try_new(keys),
            Err(MejiroKeyMapError::InvalidUserKey(32))
        );
    }

    fn event(user_key: u8, pressed: bool) -> ActionEvent {
        ActionEvent {
            action: Action::User(user_key),
            keyboard_event: KeyboardEvent::key(0, 0, pressed),
        }
    }

    fn configured_usb_status() -> ConnectionStatusChangeEvent {
        let mut status = ConnectionStatus::new();
        status.usb = UsbState::Configured;
        ConnectionStatusChangeEvent(status)
    }

    fn keyboard_report(report: &Report) -> &rmk::hid::KeyboardReport {
        match report {
            Report::KeyboardReport(report) => report,
            _ => panic!("expected keyboard report"),
        }
    }

    #[test]
    fn processor_consumes_user_actions_and_emits_reports() {
        rmk::embassy_futures::block_on(async {
            let map = MejiroKeyMap::new([
                0, 1, 2, 3, 4, 5, 6, 31, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23,
            ]);
            let mut processor = MejiroProcessor::with_keymap(TestSink::default(), map);

            processor
                .on_connection_status_change_event(configured_usb_status())
                .await;
            processor.on_action_event(event(31, true)).await;
            processor.on_action_event(event(31, false)).await;

            assert_eq!(
                processor.output.transports,
                [ConnectionType::Usb, ConnectionType::Usb]
            );
            assert_eq!(processor.output.reports.len(), 2);
            assert_eq!(keyboard_report(&processor.output.reports[0]).modifier, 0);
            assert_eq!(
                keyboard_report(&processor.output.reports[0]).keycodes[0],
                HidKeyCode::A as u8
            );
            assert_eq!(
                keyboard_report(&processor.output.reports[1]).keycodes,
                [0; 6]
            );
        });
    }

    #[test]
    fn processor_ignores_non_user_actions() {
        rmk::embassy_futures::block_on(async {
            let mut processor = MejiroProcessor::new(TestSink::default());
            processor
                .on_connection_status_change_event(configured_usb_status())
                .await;
            processor
                .on_action_event(ActionEvent {
                    action: Action::Key(KeyCode::Hid(HidKeyCode::A)),
                    keyboard_event: KeyboardEvent::key(0, 0, true),
                })
                .await;

            assert!(processor.output.reports.is_empty());
        });
    }

    #[test]
    fn processor_leaves_rmk_reserved_user_actions_alone() {
        rmk::embassy_futures::block_on(async {
            let mut processor = MejiroProcessor::new(TestSink::default());
            processor
                .on_connection_status_change_event(configured_usb_status())
                .await;
            processor.on_action_event(event(7, true)).await;
            processor.on_action_event(event(7, false)).await;

            assert!(processor.output.reports.is_empty());
        });
    }

    #[test]
    fn processor_ignores_user_actions_from_rotary_encoders() {
        rmk::embassy_futures::block_on(async {
            let mut processor = MejiroProcessor::new(TestSink::default());
            processor
                .on_connection_status_change_event(configured_usb_status())
                .await;
            processor
                .on_action_event(ActionEvent {
                    action: Action::User(8),
                    keyboard_event: KeyboardEvent::rotary_encoder(
                        0,
                        rmk::input_device::rotary_encoder::Direction::Clockwise,
                        true,
                    ),
                })
                .await;

            assert!(processor.output.reports.is_empty());
        });
    }
}
