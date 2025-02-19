mod uart;

use std::{
    collections::VecDeque,
    fs::File,
    io::{Read, Write},
    time::{Duration, Instant},
};

use bootloader_shared::{
    is_pi_get_prog_info_byte, CRC_ALGORITHM, INSTALLER_PROG_INFO, INSTALLER_SUCCESS, PI_GET_CODE,
    PI_GET_PROG_INFO, PI_SUCCESS,
};
use eyre::{ensure, eyre, Context};
use uart::Uart;

fn main() {
    let mut args = std::env::args();
    let target = args.nth(1).expect("requires a file to install");
    let mut program = Vec::new();
    File::open(target)
        .expect("file does not exist")
        .read_to_end(&mut program)
        .expect("could not read file");

    transmit(&program).unwrap();
}

fn transmit(program: &[u8]) -> Result<(), eyre::Report> {
    let mut checksum = CRC_ALGORITHM.digest_with_initial(0);
    checksum.update(program);
    let checksum = checksum.finalize();

    let mut uart = Uart::open().context("opening uart")?;
    let mut v: u32 = 0;
    let mut count = 0;
    let start = Instant::now();
    println!("listening for prog info req");
    loop {
        let new_value = uart
            .getu8(Duration::from_secs(10))
            .context("waiting for prog info req")?;
        v = (v >> 8) + ((new_value as u32) << 24);
        count += 1;
        if count > 4 {
            println!("got {:#010x}", v);
        }
        if v == PI_GET_PROG_INFO {
            break;
        }

        if Instant::now().duration_since(start) > Duration::from_secs(3) {
            return Err(eyre!(
                "timeout of 3 seconds elapsed while waiting for prog info req match"
            ));
        }
    }

    println!("got prog info request");
    uart.put32(INSTALLER_PROG_INFO)?;

    uart.put32(program.len() as u32)?;
    uart.put32(checksum)?;

    // Ignore trailing GET_PROG_INFO bytes.
    let mut byte = uart.getu8(Duration::from_secs(300))?;
    let mut trailing_bytes = vec![];
    while is_pi_get_prog_info_byte(byte) {
        trailing_bytes.push(byte);
        byte = uart.getu8(Duration::from_secs(10)).with_context(|| {
            format!(
                "clearing prog info bytes: {}",
                trailing_bytes
                    .iter()
                    .map(|b| format!("{:#04x}", b))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })?;
    }
    // Get remaining bytes.
    let mut next = (byte as u32) << 24;
    for _ in 1..4 {
        let byte = uart.getu8(Duration::from_secs(10))?;
        next = (next >> 8) + ((byte as u32) << 24);
    }

    ensure!(
        next == PI_GET_CODE,
        "expected get code, got {:#010x} but expected {:#010x}",
        next,
        PI_GET_CODE
    );
    let pi_checksum = uart.get32()?;

    if pi_checksum != checksum {
        return Err(eyre!(
            "mismatched checksum: got {:#010x} but expected {:#010x}",
            pi_checksum,
            checksum
        ));
    }

    println!(
        "matched checksum, sending program: {} KB",
        program.len() / 1_000
    );
    uart.put_bytes(program)?;

    let next = uart.get32()?;
    ensure!(
        next == PI_SUCCESS,
        "expected success message: got {:#010x} but expected {:#010x}",
        next,
        PI_SUCCESS
    );

    uart.put32(INSTALLER_SUCCESS)?;
    println!("successfully loaded, waiting for DONE!!!\n");

    let mut last_chars = VecDeque::new();
    loop {
        let c = uart.getu8(Duration::from_secs(300))?;
        print!("{}", c as char);
        std::io::stdout().flush()?;
        last_chars.push_back(c as char);
        if last_chars.len() > "DONE!!!".len() {
            last_chars.pop_front();
        }
        if last_chars.iter().cloned().eq("DONE!!!".chars()) {
            break;
        }
    }
    println!("");
    return Ok(());
}
