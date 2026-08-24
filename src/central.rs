#![no_main]
#![no_std]

use mejiro_rmk::{MejiroProcessor, MejiroReportSink};
use panic_probe as _;
use rmk::event::ConnectionType;
use rmk::hid::Report;
use rmk::macros::rmk_central;

struct RmkReportSink;

impl MejiroReportSink for RmkReportSink {
    async fn send(&mut self, transport: ConnectionType, report: Report) {
        match transport {
            ConnectionType::Usb => rmk::channel::USB_REPORT_CHANNEL.send(report).await,
            ConnectionType::Ble => {
                #[cfg(target_arch = "arm")]
                rmk::channel::BLE_REPORT_CHANNEL.send(report).await;
            }
        }
    }
}

#[rmk_central]
mod keyboard_central {
    #[register_processor(event)]
    fn mejiro_processor() -> MejiroProcessor<RmkReportSink> {
        MejiroProcessor::new(RmkReportSink)
    }
}
