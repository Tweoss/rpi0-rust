use core::marker::{ConstParamTy_, UnsizedConstParamTy};

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
    If<{ valid_pin(INDEX) }>: True,
{
    _hidden: (),
}

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
    pub unsafe fn forge() -> Pin<I, { F }> {
        Self { _hidden: () }
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

    pub fn erase(self) -> Pin<I, { PinFsel::Unset }> {
        Pin::<I, { PinFsel::Unset }> { _hidden: () }
    }

    pub fn into_output(self) -> Pin<I, { PinFsel::Output }> {
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b001 << ((I % 10) * 3);
        Self::set_fsel(I, and, or);
        Pin::<I, { PinFsel::Output }> { _hidden: () }
    }

    pub fn into_input(self) -> Pin<I, { PinFsel::Input }> {
        let and = !(0b111 << ((I % 10) * 3));
        Self::set_fsel(I, and, 0);
        Pin::<I, { PinFsel::Input }> { _hidden: () }
    }

    pub fn into_alt0(self) -> Pin<I, { PinFsel::Alt0 }> {
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b100 << ((I % 10) * 3);
        Self::set_fsel(I, and, or);
        Pin::<I, { PinFsel::Alt0 }> { _hidden: () }
    }

    pub fn into_alt5(self) -> Pin<I, { PinFsel::Alt5 }> {
        let and = !(0b111 << ((I % 10) * 3));
        let or = 0b010 << ((I % 10) * 3);
        Self::set_fsel(I, and, or);
        Pin::<I, { PinFsel::Alt5 }> { _hidden: () }
    }
}

impl<const I: usize> Pin<I, { PinFsel::Output }>
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

impl<const I: usize> Pin<I, { PinFsel::Input }>
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
}
