//! # Handle GPIO pins
use core::marker::{ConstParamTy_, PhantomData};

const PIN_COUNT: usize = 54;

#[derive(Default, Clone, Copy)]
#[allow(unused)]
enum PinState {
    #[default]
    Unset,
    Output(bool),
    Input,
}

pub struct If<const COND: bool>;
pub struct If2<const COND: bool>;
pub const fn valid_pin(n: usize) -> bool {
    n < PIN_COUNT
}
pub const fn is_output(fsel: PinFsel) -> bool {
    matches!(fsel, PinFsel::Output)
}
pub const fn is_input(fsel: PinFsel) -> bool {
    matches!(fsel, PinFsel::Input)
}

pub trait True {}
impl True for If<true> {}
impl True for If2<true> {}

#[derive(PartialEq, Eq, Debug)]
pub enum PinFsel {
    Unset,
    Input,
    Output,
    Alt0,
    Alt5,
}

#[derive(Default)]
pub struct Unset {}
#[derive(Default)]
pub struct Input {}
#[derive(Default)]
pub struct Output {}
#[derive(Default)]
pub struct Alt0 {}
#[derive(Default)]
pub struct Alt5 {}

pub trait PinFselS {}
impl PinFselS for Unset {}
impl PinFselS for Input {}
impl PinFselS for Output {}
impl PinFselS for Alt0 {}
impl PinFselS for Alt5 {}

impl ConstParamTy_ for PinFsel {}

/// A representation of a singular pin.
/// The associated `FSEL` of type [`PinFsel`] indicates the compile-time state
/// of the pin.
pub struct Pin<const INDEX: usize, FSEL>
where
    If<{ valid_pin(INDEX) }>: True,
    FSEL: PinFselS,
{
    _hidden: PhantomData<FSEL>,
}

impl<const INDEX: usize, FSEL: PinFselS + Default + core::fmt::Debug> core::fmt::Debug
    for Pin<INDEX, FSEL>
where
    If<{ valid_pin(INDEX) }>: True,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Pin<{INDEX}, {:?}>", FSEL::default()))
    }
}
impl<const I: usize, F: PinFselS> Pin<I, F> where If<{ valid_pin(I) }>: True {}

impl<const I: usize, F: PinFselS> Pin<I, F>
where
    If<{ valid_pin(I) }>: True,
{
    pub unsafe fn forge() -> Pin<I, F> {
        Self {
            _hidden: PhantomData::default(),
        }
    }

    fn set_fsel(i: usize, and_mask: u32, or_mask: u32) {
        let (a, o) = (and_mask, or_mask);
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            match i {
                0..10 => gpio.gpfsel0().modify(|r, w| w.bits(r.bits() & a | o)),
                10..20 => gpio.gpfsel1().modify(|r, w| w.bits(r.bits() & a | o)),
                20..30 => gpio.gpfsel2().modify(|r, w| w.bits(r.bits() & a | o)),
                30..40 => gpio.gpfsel3().modify(|r, w| w.bits(r.bits() & a | o)),
                40..50 => gpio.gpfsel4().modify(|r, w| w.bits(r.bits() & a | o)),
                50..PIN_COUNT => gpio.gpfsel5().modify(|r, w| w.bits(r.bits() & a | o)),
                _ => unreachable!(),
            };
        }
    }

    pub fn erase(self) -> Pin<I, Unset> {
        Pin::<I, Unset> {
            _hidden: PhantomData::default(),
        }
    }

    pub fn into_output(self) -> Pin<I, Output> {
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b001 << ((I % 10) * 3);
        Self::set_fsel(I, and, or);
        Pin::<I, Output> {
            _hidden: PhantomData::default(),
        }
    }
    pub fn into_input(self) -> Pin<I, Input> {
        let and = !(0b111 << ((I % 10) * 3));
        Self::set_fsel(I, and, 0);
        Pin::<I, Input> {
            _hidden: PhantomData::default(),
        }
    }

    pub fn into_alt0(self) -> Pin<I, Alt0> {
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b100 << ((I % 10) * 3);
        Self::set_fsel(I, and, or);
        Pin::<I, Alt0> {
            _hidden: PhantomData::default(),
        }
    }

    pub fn into_alt5(self) -> Pin<I, Alt5> {
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b010 << ((I % 10) * 3);
        Self::set_fsel(I, and, or);
        Pin::<I, Alt5> {
            _hidden: PhantomData::default(),
        }
    }
}

impl<const I: usize> Pin<I, Output>
where
    If<{ valid_pin(I) }>: True,
{
    pub fn write(&mut self, bit: bool) {
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            if bit {
                match I {
                    0..32 => gpio.gpset0().write_with_zero(|w| w.bits(1 << I)),
                    32..PIN_COUNT => gpio.gpset1().write_with_zero(|w| w.bits(1 << (I % 32))),
                    _ => unreachable!(),
                }
            } else {
                match I {
                    0..32 => gpio.gpclr0().write_with_zero(|w| w.bits(1 << I)),
                    32..PIN_COUNT => gpio.gpclr1().write_with_zero(|w| w.bits(1 << (I % 32))),
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl<const I: usize> Pin<I, Input>
where
    If<{ valid_pin(I) }>: True,
{
    pub fn read(&self) -> bool {
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            (match I {
                0..32 => (gpio.gplev0().read().bits() >> I) & 1,
                32..PIN_COUNT => (gpio.gplev1().read().bits() >> (I % 32)) & 1,
                _ => unreachable!(),
            }) == 1
        }
    }

    pub fn set_falling_detection(&self, enabled: bool) {
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            if enabled {
                match I {
                    0..32 => gpio.gpfen0().write_with_zero(|w| w.bits(1 << I)),
                    32..PIN_COUNT => gpio.gpfen1().write_with_zero(|w| w.bits(1 << (I % 32))),
                    _ => unreachable!(),
                }
            } else {
                match I {
                    0..32 => gpio.gpfen0().modify(|r, w| w.bits(r.bits() & !(1 << I))),
                    32..PIN_COUNT => gpio
                        .gpfen1()
                        .modify(|r, w| w.bits(r.bits() & !(1 << (I % 32)))),
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn set_rising_detection(&self, enabled: bool) {
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            if enabled {
                match I {
                    0..32 => gpio.gpren0().write_with_zero(|w| w.bits(1 << I)),
                    32..PIN_COUNT => gpio.gpren1().write_with_zero(|w| w.bits(1 << (I % 32))),
                    _ => unreachable!(),
                }
            } else {
                match I {
                    0..32 => gpio.gpren0().modify(|r, w| w.bits(r.bits() & !(1 << I))),
                    32..PIN_COUNT => gpio
                        .gpren1()
                        .modify(|r, w| w.bits(r.bits() & !(1 << (I % 32)))),
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn event_detected(&self) -> bool {
        (unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            match I {
                0..32 => (gpio.gpeds0().read().bits() >> I) & 1,
                32..PIN_COUNT => (gpio.gpeds1().read().bits() >> (I % 32)) & 1,
                _ => unreachable!(),
            }
        }) == 1
    }

    pub fn clear_event(&self) {
        unsafe {
            let gpio = bcm2835_lpa::GPIO::steal();
            match I {
                0..32 => gpio.gpeds0().write_with_zero(|w| w.bits(1 << I)),
                32..PIN_COUNT => gpio.gpeds1().write_with_zero(|w| w.bits(1 << I)),
                _ => unreachable!(),
            }
        }
    }
}
