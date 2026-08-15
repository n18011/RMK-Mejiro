const SPLIT_SERVICE_UUID: [u8; 16] = [
    70, 153, 101, 152, 54, 53, 10, 191, 7, 75, 229, 24, 170, 251, 213, 77,
];
const SPLIT_MANUFACTURER_PREFIX: [u8; 4] = [0x04, 0xff, 0x18, 0xe1];

/// Extract a split-peripheral id from an advertisement.
///
/// The id follows the four-byte manufacturer prefix.  Keep this parser
/// independent from the scanner so malformed packets can be tested without a
/// Bluetooth controller.
pub(crate) fn peripheral_id(data: &[u8]) -> Option<u8> {
    // The id is at offset 25, so a 25-byte packet is still truncated.
    if data.len() < 26 {
        return None;
    }

    if data[4] != 0x07
        || !data[5..].starts_with(&SPLIT_SERVICE_UUID)
        || data[21..25] != SPLIT_MANUFACTURER_PREFIX
    {
        return None;
    }

    Some(data[25])
}

#[cfg(test)]
mod tests {
    use super::{peripheral_id, SPLIT_MANUFACTURER_PREFIX, SPLIT_SERVICE_UUID};

    fn valid_advertisement(id: u8) -> [u8; 26] {
        let mut data = [0; 26];
        data[4] = 0x07;
        data[5..21].copy_from_slice(&SPLIT_SERVICE_UUID);
        data[21..25].copy_from_slice(&SPLIT_MANUFACTURER_PREFIX);
        data[25] = id;
        data
    }

    #[test]
    fn rejects_packet_truncated_before_peripheral_id() {
        let data = valid_advertisement(3);
        assert_eq!(peripheral_id(&data[..25]), None);
    }

    #[test]
    fn extracts_peripheral_id_from_valid_advertisement() {
        assert_eq!(peripheral_id(&valid_advertisement(7)), Some(7));
    }

    #[test]
    fn rejects_wrong_service_or_manufacturer_data() {
        let mut data = valid_advertisement(7);
        data[5] ^= 1;
        assert_eq!(peripheral_id(&data), None);

        let mut data = valid_advertisement(7);
        data[21] ^= 1;
        assert_eq!(peripheral_id(&data), None);
    }
}
