#![no_main]
#![no_std]

use mejiro_rmk::MejiroController;
use panic_probe as _;
use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard_central {
    #[controller(event)]
    fn mejiro_controller() -> MejiroController {
        MejiroController::new()
    }
}
