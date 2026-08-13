#![no_main]
#![no_std]

mod mejiro_controller;

use mejiro_controller::MejiroController;
use panic_probe as _;
use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard_central {
    #[controller(event)]
    fn mejiro_controller() -> MejiroController {
        MejiroController::new()
    }
}
