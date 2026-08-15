pub mod central;
pub mod peripheral;
pub(crate) mod advertisement;

use embassy_time::{Duration, Timer};
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};
use trouble_host::prelude::{Connection, Error, PacketPool};

/// Split traffic is keyboard input and must never be accepted over a plaintext
/// BLE link.  Pairing is initiated explicitly here instead of relying on a
/// client to request it before using the split service.
pub(crate) async fn require_encrypted_link<P: PacketPool>(connection: &Connection<'_, P>) -> Result<(), Error> {
    if connection.security_level()?.encrypted() {
        return Ok(());
    }

    // Keep the peer bondable before pairing starts.  This lets the stack retain
    // the peer identity when the platform supports bonding.
    connection.set_bondable(true)?;
    connection.request_security()?;

    for _ in 0..100 {
        if connection.security_level()?.encrypted() {
            return Ok(());
        }
        Timer::after(Duration::from_millis(50)).await;
    }

    Err(Error::Timeout)
}


#[derive(Clone, Debug, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeerAddress {
    pub peer_id: u8,
    pub is_valid: bool,
    pub address: [u8; 6],
}

impl PeerAddress {
    pub(crate) fn new(peer_id: u8, is_valid: bool, address: [u8; 6]) -> Self {
        Self {
            peer_id,
            is_valid,
            address,
        }
    }
}
