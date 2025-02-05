#![no_std]

pub const PI_ERROR: u32 = 0x00001111;
pub const PI_GET_PROG_INFO: u32 = 0xEEEEFFFF;
pub const fn is_pi_get_prog_info_byte(b: u8) -> bool {
    b == 0xEE || b == 0xFF
}
pub const PI_GET_CODE: u32 = 0x11112222;
pub const PI_SUCCESS: u32 = 0x22223333;

pub const INSTALLER_PROG_INFO: u32 = 0xBEEFDEAD;
pub const INSTALLER_CODE: u32 = 0x33334444;
pub const INSTALLER_SUCCESS: u32 = 0x44445555;

pub const BASE: u32 = 0x8000;

pub const CRC_ALGORITHM: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_BZIP2);
