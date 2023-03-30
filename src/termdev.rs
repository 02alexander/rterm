use nix::fcntl::{open, OFlag};
use nix::sys::termios::{
    cfsetispeed, cfsetospeed, tcflush, tcgetattr, tcsetattr, BaudRate, ControlFlags, FlushArg,
    InputFlags, LocalFlags, OutputFlags, SetArg, SpecialCharacterIndices, Termios,
};
use nix::unistd::{close, read, write};
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;

pub struct TerminalDevice {
    fd: i32,
    termios: Termios,
    drop_handler: Arc<TerminalCloser>,
}

/// Used to handle closing of file when the terminal is split into read and write part.   
struct TerminalCloser {
    fd: i32,
}

pub struct TerminalReader {
    fd: i32,
    drop_handler: Arc<TerminalCloser>,
}

pub struct TerminalWriter {
    fd: i32,
    drop_handler: Arc<TerminalCloser>,
}

impl TerminalDevice {
    pub fn new<P: Into<PathBuf>>(filepath: P) -> anyhow::Result<TerminalDevice> {
        let oflag = OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_SYNC | OFlag::O_NONBLOCK;
        let fd = open(&filepath.into(), oflag, nix::sys::stat::Mode::empty())?;
        let termios = tcgetattr(fd)?;
        let drop_handler = Arc::new(TerminalCloser {fd});
        Ok(TerminalDevice { fd, termios, drop_handler })
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

    /// Splits the device into a read and a write part.
    pub fn split(self) -> (TerminalReader, TerminalWriter) {
        (TerminalReader {
            fd: self.fd,
            drop_handler: self.drop_handler.clone(),
        },TerminalWriter {
            fd: self.fd,
            drop_handler: self.drop_handler.clone(),
        })
    }
}

impl std::io::Read for TerminalDevice {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        read(self.fd, buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, Box::new(e)))

    }
}

impl std::io::Write for TerminalDevice {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        write(self.fd, buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, Box::new(e)))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        tcflush(self.fd, FlushArg::TCIOFLUSH).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, Box::new(e)))
    }
}

impl std::io::Write for TerminalWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        write(self.fd, buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, Box::new(e)))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        tcflush(self.fd, FlushArg::TCIOFLUSH).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, Box::new(e)))
    }
}


impl std::io::Read for TerminalReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        read(self.fd, buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, Box::new(e)))
    }
}

impl std::ops::Drop for TerminalCloser {
    fn drop(&mut self) {
        let _ = close(self.fd);
    }
}
