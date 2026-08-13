//! RMK 0.8 adapter for the pure Mejiro session.

use rmk::channel::{KEYBOARD_REPORT_CHANNEL, MEJIRO_EVENT_CHANNEL};
use rmk::controller::Controller;
use rmk::descriptor::KeyboardReport;
use rmk::event::MejiroKeyEvent;
use rmk::heapless::String;
use rmk::hid::Report;
use rmk::types::keycode::{from_ascii, KeyCode};
use rmk::types::modifier::ModifierCombination;

use rmk_mejiro::mejiro::{KeyAction, MejiroKey, MejiroSession, StrokeResult, MAX_OUTPUT};

pub struct MejiroController {
    session: MejiroSession,
}

impl MejiroController {
    pub fn new() -> Self {
        Self {
            session: MejiroSession::new(true),
        }
    }

    async fn handle(&mut self, event: MejiroKeyEvent) {
        let Some(index) = (event.keycode as u16)
            .checked_sub(KeyCode::Kb0 as u16)
            .and_then(|index| u8::try_from(index).ok())
        else {
            return;
        };
        let Some(key) = MejiroKey::from_index(index) else {
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
                if let Some((keycode, modifiers)) = key_action(action) {
                    self.send_tap(keycode, modifiers).await;
                }
            }
            StrokeResult::Repeat => {
                let text = self.session.last_text().map(|value| {
                    let mut copy = String::<MAX_OUTPUT>::new();
                    let _ = copy.push_str(value);
                    copy
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
            StrokeResult::Unsupported => {}
        }
    }

    async fn send_text(&self, text: &str) {
        let mut remaining = text;
        while let Some(index) = remaining.find("{#Left}") {
            self.send_ascii(&remaining[..index]).await;
            self.send_tap(KeyCode::Left, ModifierCombination::new())
                .await;
            remaining = &remaining[index + "{#Left}".len()..];
        }
        self.send_ascii(remaining).await;
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
        let mut pressed = KeyboardReport::default();
        pressed.modifier = modifiers.into_bits();
        pressed.keycodes[0] = keycode as u16 as u8;
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

fn key_action(action: KeyAction) -> Option<(KeyCode, ModifierCombination)> {
    let no_modifiers = ModifierCombination::new();
    Some(match action {
        KeyAction::Backspace => (KeyCode::Backspace, no_modifiers),
        KeyAction::Delete => (KeyCode::Delete, no_modifiers),
        KeyAction::Escape => (KeyCode::Escape, no_modifiers),
        KeyAction::Left => (KeyCode::Left, no_modifiers),
        KeyAction::Down => (KeyCode::Down, no_modifiers),
        KeyAction::Up => (KeyCode::Up, no_modifiers),
        KeyAction::Right => (KeyCode::Right, no_modifiers),
        KeyAction::Home => (KeyCode::Home, no_modifiers),
        KeyAction::End => (KeyCode::End, no_modifiers),
        KeyAction::ShiftLeft => (KeyCode::Left, ModifierCombination::LSHIFT),
        KeyAction::ShiftDown => (KeyCode::Down, ModifierCombination::LSHIFT),
        KeyAction::ShiftUp => (KeyCode::Up, ModifierCombination::LSHIFT),
        KeyAction::ShiftRight => (KeyCode::Right, ModifierCombination::LSHIFT),
        KeyAction::ShiftHome => (KeyCode::Home, ModifierCombination::LSHIFT),
        KeyAction::ShiftEnd => (KeyCode::End, ModifierCombination::LSHIFT),
        KeyAction::ShiftEnter => (KeyCode::Enter, ModifierCombination::LSHIFT),
        KeyAction::CtrlEnter => (KeyCode::Enter, ModifierCombination::LCTRL),
        KeyAction::Enter => (KeyCode::Enter, no_modifiers),
        KeyAction::Space => (KeyCode::Space, no_modifiers),
        KeyAction::Tab => (KeyCode::Tab, no_modifiers),
        KeyAction::Language1 => (KeyCode::Language1, no_modifiers),
        KeyAction::Language2 => (KeyCode::Language2, no_modifiers),
    })
}
