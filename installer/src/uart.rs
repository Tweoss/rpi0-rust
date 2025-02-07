use std::{
    fs::File,
    io::{ErrorKind, Read, Write},
    os::fd::AsRawFd,
    time::{Duration, Instant},
};

use eyre::Context;
use termios::{
    cfsetspeed, os::macos::CRTSCTS, tcsetattr, Termios, CLOCAL, CREAD, CS8, CSIZE, CSTOPB, ECHO,
    ECHOE, ICANON, IGNBRK, ISIG, IXANY, IXOFF, IXON, OPOST, PARENB, TCSANOW, VMIN, VTIME,
};

const SPEED: u64 = 115_200 * 8;
const TIMEOUT: u8 = 10;

pub struct Uart {
    file: File,
}

impl Uart {
    pub fn open() -> Result<Self, eyre::Report> {
        let file = File::options()
            .read(true)
            .write(true)
            .open("/dev/cu.SLAB_USBtoUART")
            .context("missing usb file")?;
        let fd = file.as_raw_fd();
        let mut termios = Termios::from_fd(fd).unwrap();
        cfsetspeed(&mut termios, SPEED).unwrap();
        termios.c_iflag &= !IGNBRK; // disable break processing
        termios.c_lflag = 0; // no signaling chars, no echo, no canonical processing
        termios.c_oflag = 0; // no remapping, no delays
        termios.c_cc[VMIN] = 0; // read doesn't block
                                // VTIME is in .1 seconds, so have to multiply by 10.
        termios.c_cc[VTIME] = TIMEOUT * 10; // this seems to cause issues?

        // Setup 8n1 mode.
        // Disables the Parity Enable bit(PARENB),So No Parity
        termios.c_cflag &= !PARENB;
        // CSTOPB = 2 Stop bits,here it is cleared so 1 Stop bit
        termios.c_cflag &= !CSTOPB;
        // Clears the mask for setting the data size
        termios.c_cflag &= !CSIZE;
        // Set the data bits = 8
        termios.c_cflag |= CS8;
        // No Hardware flow Control
        termios.c_cflag &= !CRTSCTS;
        // Enable receiver,Ignore Modem Control lines
        termios.c_cflag |= CREAD | CLOCAL;

        // Disable XON/XOFF flow control both i/p and o/p
        termios.c_iflag &= !(IXON | IXOFF | IXANY);
        // Non Cannonical mode
        termios.c_iflag &= !(ICANON | ECHO | ECHOE | ISIG);
        // No Output Processing
        termios.c_oflag &= !OPOST;

        tcsetattr(fd, TCSANOW, &termios).unwrap();

        Ok(Self { file })
    }

    // TODO: get rid of timeout
    pub fn getu8(&mut self, timeout: Duration) -> Result<u8, eyre::Report> {
        let mut buf = [0u8; 1];
        let start = Instant::now();
        while self.file.read(&mut buf)? == 0 {
            if Instant::now().duration_since(start) > timeout {
                return Err(std::io::Error::new(
                    ErrorKind::TimedOut,
                    format!(
                        "Timed out getting byte after {} seconds.",
                        timeout.as_secs_f32()
                    ),
                )
                .into());
            }
        }
        Ok(buf[0])
    }

    pub fn get32(&mut self) -> Result<u32, eyre::Report> {
        let mut buf = [0u8; 4];
        self.file.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    pub fn put_bytes(&mut self, v: &[u8]) -> Result<(), eyre::Report> {
        Ok(self.file.write_all(v)?)
    }

    pub fn put32(&mut self, v: u32) -> Result<(), eyre::Report> {
        Ok(self.file.write_all(&u32::to_le_bytes(v))?)
    }
}
