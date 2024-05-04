#![no_std]
#![no_main]

use core::fmt::Write;
use cortex_m_semihosting::debug;

use embedded_hal::can::{ExtendedId, Frame, Id, StandardId};

use heapless::Vec;

use slcan_parser::CanserialFrame;

use device::bdma::vals::{Dir, Pl, Size};
use device::gpio::vals::{CnfIn, CnfOut, Mode};
use stm32_metapac::{self as device, interrupt};

use defmt_rtt as _; // global logger

use panic_probe as _;

pub mod can;

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

/// Terminates the application and makes a semihosting-capable debug tool exit
/// with status code 0.
pub fn exit() -> ! {
    loop {
        debug::exit(debug::EXIT_SUCCESS);
    }
}

/// Hardfault handler.
///
/// Terminates the application and makes a semihosting-capable debug tool exit
/// with an error. This seems better than the default, which is to spin in a
/// loop.
#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    loop {
        debug::exit(debug::EXIT_FAILURE);
    }
}

/// Convert `CanFrame` to `Vec` containing ascii string generated by a `CanserialFrame`
pub fn bxcan_to_vec(bxcan_frame: &bxcan::Frame) -> Option<Vec<u8, 32>> {
    let frame = bxcan_to_canserial(bxcan_frame)?;
    // longest canserial ascii string for an extended id frame is 26 bytes
    let mut buffer: Vec<u8, 32> = Vec::new();
    core::write!(&mut buffer, "{}\r", frame).ok();
    Some(buffer)
}

/// Convert `bxcan::Id` to `Id`
///
/// bxcan doesn't use embedded_hal
pub fn bxcan_to_canserial_id(id: &bxcan::Id) -> Option<Id> {
    match id {
        bxcan::Id::Standard(stdid) => match StandardId::new(stdid.as_raw()) {
            Some(id) => Some(Id::Standard(id)),
            None => None,
        },
        bxcan::Id::Extended(extid) => match ExtendedId::new(extid.as_raw()) {
            Some(id) => Some(Id::Extended(id)),
            None => None,
        },
    }
}

/// Convert `bxcan::Frame` to `CanserialFrame` for use with serial port
pub fn bxcan_to_canserial(bcanframe: &bxcan::Frame) -> Option<CanserialFrame> {
    match bcanframe.is_remote_frame() {
        true => CanserialFrame::new_remote(
            bxcan_to_canserial_id(&bcanframe.id())?,
            bcanframe.dlc() as usize,
        ),
        false => match bcanframe.data() {
            Some(d) => CanserialFrame::new_frame(
                bxcan_to_canserial_id(&bcanframe.id())?,
                d.get(0..d.len())?,
            ),
            // possible to have an empty data frame
            None => CanserialFrame::new_frame(bxcan_to_canserial_id(&bcanframe.id())?, &[]),
        },
    }
}

/// Convert `Id` to `bxcan::Id`
///
/// bxcan doesn't use embedded_hal
pub fn canserial_to_bxcan_id(id: &Id) -> Option<bxcan::Id> {
    match id {
        Id::Standard(stdid) => match bxcan::StandardId::new(stdid.as_raw()) {
            Some(id) => Some(bxcan::Id::Standard(id)),
            None => None,
        },
        Id::Extended(extid) => match bxcan::ExtendedId::new(extid.as_raw()) {
            Some(id) => Some(bxcan::Id::Extended(id)),
            None => None,
        },
    }
}

/// Convert `CanserialFrame` to `bxcan::Frame` for use with bcan driver
pub fn canserial_to_bxcan(slcan: &CanserialFrame) -> Option<bxcan::Frame> {
    match slcan.is_remote_frame() {
        true => Some(bxcan::Frame::new_remote(
            canserial_to_bxcan_id(&slcan.id())?,
            slcan.dlc() as u8,
        )),
        false => Some(bxcan::Frame::new_data(
            canserial_to_bxcan_id(&slcan.id())?,
            bxcan::Data::new(slcan.data())?,
        )),
    }
}

// defmt-test 0.3.0 has the limitation that this `#[tests]` attribute can only be used
// once within a crate. the module can be in any file but there can only be at most
// one `#[tests]` module in this library crate
#[cfg(test)]
#[defmt_test::tests]
mod unit_tests {
    use super::*;

    #[test]
    fn test_canserial_to_bxcan() {
        let id = Id::Standard(StandardId::new(0x1).unwrap());
        let cframe =
            CanserialFrame::new_frame(id, &[0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8]).unwrap();
        let bframe = canserial_to_bxcan(&cframe).unwrap();
        //assert_eq!(cframe.id(), bframe.id());
        assert_eq!(cframe.dlc(), bframe.dlc() as usize);
        for (i, b) in bframe.data().unwrap().iter().enumerate() {
            assert_eq!(cframe.data()[i], *b);
        }
    }
}
