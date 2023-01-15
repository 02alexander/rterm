use nix::fcntl::{open, OFlag};
use nix::sys::termios::{
    cfsetispeed, cfsetospeed, tcflush, tcgetattr, tcsetattr, BaudRate, ControlFlags, FlushArg,
    InputFlags, LocalFlags, OutputFlags, SetArg, SpecialCharacterIndices, Termios,
};
use nix::unistd::{close, read, write};
use std::path::PathBuf;

pub struct TerminalDevice {
    fd: i32,
    termios: Termios,
}

impl TerminalDevice {
    pub fn new<P: Into<PathBuf>>(filepath: P) -> anyhow::Result<TerminalDevice> {
        let oflag = OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_SYNC | OFlag::O_NONBLOCK;
        let fd = open(&filepath.into(), oflag, nix::sys::stat::Mode::empty())?;
        let termios = tcgetattr(fd)?;
        Ok(TerminalDevice { fd, termios })
    }

    pub fn configure_for_arduino(&mut self, baud_rate: BaudRate) -> anyhow::Result<()> {
        cfsetispeed(&mut self.termios, baud_rate)?;
        cfsetospeed(&mut self.termios, baud_rate)?;
        self.termios.control_flags |= ControlFlags::CS8;
        self.termios.output_flags &=
            !(OutputFlags::ONLCR | OutputFlags::ONOCR | OutputFlags::OCRNL);
        self.termios.output_flags |= OutputFlags::ONLRET;
        self.termios.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON);
        self.termios.input_flags |= InputFlags::IGNCR;
        self.termios.input_flags &= !(InputFlags::INPCK | InputFlags::ISTRIP);

        self.termios.control_chars[SpecialCharacterIndices::VMIN as usize] = 1;
        self.termios.control_chars[SpecialCharacterIndices::VTIME as usize] = 0;
        tcsetattr(self.fd, SetArg::TCSAFLUSH, &self.termios)?;
        Ok(())
    }
}

impl std::io::Read for TerminalDevice {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match read(self.fd, buf) {
            Ok(n) => Ok(n),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, Box::new(e))),
        }
    }
}

impl std::io::Write for TerminalDevice {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match write(self.fd, buf) {
            Ok(n) => Ok(n),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, Box::new(e))),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match tcflush(self.fd, FlushArg::TCIOFLUSH) {
            Ok(_) => Ok(()),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, Box::new(e))),
        }
    }
}

impl std::ops::Drop for TerminalDevice {
    fn drop(&mut self) {
        let _ = close(self.fd);
    }
}
