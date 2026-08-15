//! Output boundaries shared by the pure session and the RMK adapter.
//!
//! Keeping the token parser and capacity policy here prevents the large
//! language-transform module from owning transport details.

use heapless::{String, Vec};

pub const MAX_OUTPUT: usize = 128;
pub type Text = String<MAX_OUTPUT>;

/// The only non-ASCII operation embedded in a text result.
pub const LEFT_TOKEN: &str = "{#Left}";

/// Failure returned when a conversion cannot fit in the HID output buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    CapacityExceeded,
}

/// A text stream operation consumed by the RMK adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextOperation {
    Text(Text),
    Left,
}

/// Split a result into ASCII text and the typed left-arrow operation.
pub fn text_operations(input: &str) -> Result<Vec<TextOperation, MAX_OUTPUT>, OutputError> {
    if input.len() > MAX_OUTPUT {
        return Err(OutputError::CapacityExceeded);
    }

    let mut operations = Vec::new();
    let mut rest = input;
    while let Some(index) = rest.find(LEFT_TOKEN) {
        if index != 0 {
            let mut text = Text::new();
            text.push_str(&rest[..index])
                .map_err(|_| OutputError::CapacityExceeded)?;
            operations
                .push(TextOperation::Text(text))
                .map_err(|_| OutputError::CapacityExceeded)?;
        }
        operations
            .push(TextOperation::Left)
            .map_err(|_| OutputError::CapacityExceeded)?;
        rest = &rest[index + LEFT_TOKEN.len()..];
    }
    if !rest.is_empty() {
        let mut text = Text::new();
        text.push_str(rest)
            .map_err(|_| OutputError::CapacityExceeded)?;
        operations
            .push(TextOperation::Text(text))
            .map_err(|_| OutputError::CapacityExceeded)?;
    }
    Ok(operations)
}
