#![no_std]
#![allow(incomplete_features)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(unsized_const_params)]

mod pin_array;
pub use pin_array::get_pins;

use core::{
    cell::RefCell,
    marker::{ConstParamTy_, UnsizedConstParamTy},
};

use frunk::{
    hlist::{self, HList},
    HCons, HList, HNil,
};

const PIN_COUNT: usize = 54;

struct MockPinArray {
    pins: [PinState; PIN_COUNT],
}

impl Default for MockPinArray {
    fn default() -> Self {
        Self {
            pins: [PinState::Unset; PIN_COUNT],
        }
    }
}

#[derive(Default, Clone, Copy)]
enum PinState {
    #[default]
    Unset,
    Output(bool),
    Input,
}

// #[thread_local]
// static STATE_MOCKUP: RefCell<MockPinArray> = RefCell::new(MockPinArray::default());

pub struct If<const COND: bool>;
pub const fn valid_pin(n: usize) -> bool {
    n < PIN_COUNT
}

pub trait True {}
impl True for If<true> {}

#[derive(PartialEq, Eq, Debug)]
pub enum PinFsel {
    Unset,
    Input,
    Output,
    Alt0,
    Alt5,
}

impl UnsizedConstParamTy for PinFsel {}
impl ConstParamTy_ for PinFsel {}

pub struct Pin<const INDEX: usize, const FSEL: PinFsel>
where
    If<{ valid_pin(INDEX) }>: True;

impl<const INDEX: usize, const FSEL: PinFsel> core::fmt::Debug for Pin<INDEX, FSEL>
where
    If<{ valid_pin(INDEX) }>: True,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Pin<{INDEX}, {:?}>", FSEL))
    }
}

impl<const I: usize, const F: PinFsel> Pin<I, F>
where
    If<{ valid_pin(I) }>: True,
{
    fn set_fsel(i: usize, and_mask: u32, or_mask: u32, w: impl FnOnce(usize, usize)) {
        let (a, o) = (and_mask, or_mask);
        let peek = |v| {
            w(0x20200000_usize + 4 * (i / 10), v as usize);
            v
        };
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            match i {
                0..10 => gpio.gpfsel0().modify(|r, w| w.bits(peek(r.bits() & a | o))),
                10..20 => gpio.gpfsel1().modify(|r, w| w.bits(peek(r.bits() & a | o))),
                20..30 => gpio.gpfsel2().modify(|r, w| w.bits(peek(r.bits() & a | o))),
                30..40 => gpio.gpfsel3().modify(|r, w| w.bits(peek(r.bits() & a | o))),
                40..50 => gpio.gpfsel4().modify(|r, w| w.bits(peek(r.bits() & a | o))),
                50..PIN_COUNT => gpio.gpfsel5().modify(|r, w| w.bits(peek(r.bits() & a | o))),
                _ => unreachable!(),
            };
        }
    }

    pub fn into_output(self, w: impl FnOnce(usize, usize)) -> Pin<I, { PinFsel::Output }> {
        log::debug!("setting pin {I} to output");
        // #[cfg(test)]
        // {
        //     STATE_MOCKUP.with_borrow_mut(|v| v.pins[I] = PinState::Output(false));
        //     return PinOut::<I>;
        // }
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b001 << ((I % 10) * 3);
        Self::set_fsel(I, and, or, w);
        Pin::<I, { PinFsel::Output }>
    }

    pub fn into_input(self, w: impl FnOnce(usize, usize)) -> Pin<I, { PinFsel::Input }> {
        let and = !(0b111 << ((I % 10) * 3));
        log::debug!("setting pin {I} to input");
        Self::set_fsel(I, and, 0, w);
        Pin::<I, { PinFsel::Input }>
    }

    pub fn into_alt0(self, w: impl FnOnce(usize, usize)) -> Pin<I, { PinFsel::Alt0 }> {
        log::debug!("setting pin {I} to alt0");
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b100 << ((I % 10) * 3);
        Self::set_fsel(I, and, or, w);
        Pin::<I, { PinFsel::Alt0 }>
    }

    pub fn into_alt5(self, w: impl FnOnce(usize, usize)) -> Pin<I, { PinFsel::Alt5 }> {
        log::debug!("setting pin {I} to alt5");
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b010 << ((I % 10) * 3);
        Self::set_fsel(I, and, or, w);
        Pin::<I, { PinFsel::Alt5 }>
    }
}

impl<const I: usize> Pin<I, { PinFsel::Output }>
where
    If<{ valid_pin(I) }>: True,
{
    pub fn write(&mut self, bit: bool, w: impl FnOnce(usize, usize)) {
        log::debug!("writing {bit} to pin {I}");
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            if bit {
                let peek = |v| {
                    w(0x2020001C + (I / 32) * 4, v as usize);
                    v
                };
                match I {
                    0..32 => gpio.gpset0().write_with_zero(|w| w.bits(peek(1 << I))),
                    32..PIN_COUNT => gpio
                        .gpset1()
                        .write_with_zero(|w| w.bits(peek(1 << (I % 32)))),
                    _ => unreachable!(),
                }
            } else {
                let peek = |v| {
                    w(0x20200028 + (I / 32) * 4, v as usize);
                    v
                };
                match I {
                    0..32 => gpio.gpclr0().write_with_zero(|w| w.bits(peek(1 << I))),
                    32..PIN_COUNT => gpio
                        .gpclr1()
                        .write_with_zero(|w| w.bits(peek(1 << (I % 32)))),
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl<const I: usize> Pin<I, { PinFsel::Input }>
where
    If<{ valid_pin(I) }>: True,
{
    pub fn read(&self, w: impl FnOnce(usize, bool)) -> bool {
        let peek = |v| {
            w(0x20200034 + (I / 32) * 4, v);
            v
        };
        let out = unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            peek(
                (match I {
                    0..32 => (gpio.gplev0().read().bits() >> I) & 1,
                    32..PIN_COUNT => (gpio.gplev1().read().bits() >> (I % 32)) & 1,
                    _ => unreachable!(),
                }) == 1,
            )
        };

        log::debug!("read {out} from pin {I}");
        out
    }
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
