use nix::fcntl::{open, OFlag};
use nix::sys::termios::{
    cfsetispeed, cfsetospeed, tcgetattr, tcsetattr, BaudRate, ControlFlags, InputFlags,
    OutputFlags, LocalFlags, SpecialCharacterIndices, Termios, SetArg,
};
use nix::unistd::{close, write, read};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct TerminalDevice {
    fd: i32,
    termios: Termios,
}

impl TerminalDevice {
    pub fn new<P: Into<PathBuf>>(filepath: P) -> anyhow::Result<TerminalDevice> {
        let oflag = OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_SYNC;
        let fd = unsafe { open(&filepath.into(), oflag, nix::sys::stat::Mode::empty())? };
        let termios = tcgetattr(fd)?;
        Ok(TerminalDevice { fd, termios })
    }

    pub fn configure_for_arduino(&mut self, baud_rate: BaudRate) -> anyhow::Result<()> {
        cfsetispeed(&mut self.termios, baud_rate)?;
        cfsetospeed(&mut self.termios, baud_rate)?;
        self.termios.control_flags |= ControlFlags::CS8;
        self.termios.output_flags &= !(OutputFlags::ONLCR
            | OutputFlags::ONOCR
            | OutputFlags::OCRNL);
        self.termios.output_flags |= OutputFlags::ONLRET;
        self.termios.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON);
        self.termios.input_flags &= !(InputFlags::INPCK | InputFlags::ISTRIP | InputFlags::IGNCR);

        self.termios.control_chars[SpecialCharacterIndices::VMIN as usize] = 1;
        self.termios.control_chars[SpecialCharacterIndices::VMIN as usize] = 2;
        tcsetattr(self.fd, SetArg::TCSAFLUSH, &self.termios)?;
        Ok(())
    }

    pub fn interface(&self) -> (Sender<Vec<u8>>, Receiver<Vec<u8>>) {
        let (todevice, tocopy) = channel::<Vec<u8>>();
        let (fromcopy, fromdevice) = channel::<Vec<u8>>();
        let fd = self.fd;

        // Reads data from tocopy and sends it to the terminal device
        std::thread::spawn(move || loop {
            match tocopy.recv() {
                Ok(data) => unsafe {
                    libc::write(fd, data.as_ptr() as *const libc::c_void, data.len());
                },
                Err(_) => {
                    break;
                }
            }
        });

        // Reads data from the terminal device and sends it to fromdevice
        std::thread::spawn(move || loop {
            #[allow(unused_mut)]
            let mut buf = [0 as u8; 1024];
            let nb_bytes_read =
                unsafe { libc::read(fd, buf.as_ptr() as *mut libc::c_void, buf.len()) };
            if fromcopy
                .send(buf[0..nb_bytes_read as usize].to_vec())
                .is_err()
            {
                break;
            }
        });
        (todevice, fromdevice)
    }
}

impl std::ops::Drop for TerminalDevice {
    fn drop(&mut self) {
        let _ = close(self.fd);
    }
}
