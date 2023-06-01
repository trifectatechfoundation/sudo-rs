use libc::*;
use std::{io, mem::MaybeUninit, os::fd::AsRawFd};
use sudo_cutils::cerr;

const INPUT_FLAGS: tcflag_t = IGNPAR
    | PARMRK
    | INPCK
    | ISTRIP
    | INLCR
    | IGNCR
    | ICRNL
    // | IUCLC /* not in libc */
    | IXON
    | IXANY
    | IXOFF
    | IMAXBEL
    | IUTF8;
const OUTPUT_FLAGS: tcflag_t = OPOST | OLCUC | ONLCR | OCRNL | ONOCR | ONLRET;
const CONTROL_FLAGS: tcflag_t = CS7 | CS8 | PARENB | PARODD;
const LOCAL_FLAGS: tcflag_t = ISIG
    | ICANON
    // | XCASE /* not in libc */
    | ECHO
    | ECHOE
    | ECHOK
    | ECHONL
    | NOFLSH
    | TOSTOP
    | IEXTEN
    | ECHOCTL
    | ECHOKE
    | PENDIN;

pub struct TermContext {
    got_sigttou: bool,
    changed: bool,
    oterm: MaybeUninit<termios>,
}

impl TermContext {
    pub const fn new() -> Self {
        Self {
            got_sigttou: false,
            changed: false,
            oterm: MaybeUninit::uninit(),
        }
    }

    /// Based on `sudo_term_copy`
    pub fn copy<F: AsRawFd, G: AsRawFd>(&mut self, src: &F, dst: &G) -> bool {
        let src = src.as_raw_fd();
        let dst = dst.as_raw_fd();

        let mut tt_src = MaybeUninit::<termios>::uninit();
        let mut tt_dst = MaybeUninit::<termios>::uninit();
        let mut wsize = MaybeUninit::<winsize>::uninit();

        if unsafe { tcgetattr(src, tt_src.as_mut_ptr()) } != 0
            || unsafe { tcgetattr(dst, tt_dst.as_mut_ptr()) } != 0
        {
            return false;
        }

        let tt_src = unsafe { tt_src.assume_init() };
        let mut tt_dst = unsafe { tt_dst.assume_init() };

        // Clear select input, output, control and local flags.
        tt_dst.c_iflag &= !INPUT_FLAGS;
        tt_dst.c_oflag &= !OUTPUT_FLAGS;
        tt_dst.c_cflag &= !CONTROL_FLAGS;
        tt_dst.c_lflag &= !LOCAL_FLAGS;

        // Copy select input, output, control and local flags.
        tt_dst.c_iflag |= tt_src.c_iflag & INPUT_FLAGS;
        tt_dst.c_oflag |= tt_src.c_oflag & OUTPUT_FLAGS;
        tt_dst.c_cflag |= tt_src.c_cflag & CONTROL_FLAGS;
        tt_dst.c_lflag |= tt_src.c_lflag & LOCAL_FLAGS;

        // Copy special chars from src verbatim.
        tt_dst.c_cc.copy_from_slice(&tt_src.c_cc);

        // Copy speed from src (zero output speed closes the connection).
        {
            let mut speed = unsafe { cfgetospeed(&tt_src) };
            if speed == B0 {
                speed = B38400;
                unsafe { cfsetospeed(&mut tt_dst, speed) };
                speed = unsafe { cfgetispeed(&tt_src) };
                unsafe { cfsetispeed(&mut tt_dst, speed) };
            }
        }

        if unsafe { self.tcsetattr_nobg(dst, TCSAFLUSH, Some(&mut tt_dst)) } == -1 {
            return false;
        }

        if unsafe { ioctl(src, TIOCGWINSZ, &mut wsize) } == 0b0 {
            unsafe { ioctl(dst, TIOCSWINSZ, &mut wsize) };
        }

        true
    }
    /// Based on `sudo_term_raw`
    pub fn raw<F: AsRawFd>(&mut self, fd: &F, isig: c_int) -> bool {
        let fd = fd.as_raw_fd();

        if !self.changed && (unsafe { tcgetattr(fd, self.oterm.as_mut_ptr()) } != 0) {
            return false;
        }

        let mut term = unsafe { self.oterm.assume_init() };
        //
        // Set terminal to raw mode but optionally enable terminal signals.
        unsafe { cfmakeraw(&mut term) };

        if isig != 0 {
            term.c_cflag |= ISIG;
        }

        if unsafe { self.tcsetattr_nobg(fd, TCSADRAIN, Some(&mut term)) } == 0 {
            self.changed = true;
            return true;
        }

        false
    }

    /// Based on `sudo_term_restore`.
    pub fn restore<F: AsRawFd>(&mut self, fd: &F, flush: bool) -> bool {
        if self.changed {
            let fd = fd.as_raw_fd();
            let flags = if flush { TCSAFLUSH } else { TCSADRAIN };
            if unsafe { self.tcsetattr_nobg(fd, flags, None) } != 0 {
                return false;
            }
            self.changed = false;
        }

        true
    }

    // Based on `tcsetattr_nobg`
    unsafe fn tcsetattr_nobg(
        &mut self,
        fd: c_int,
        flags: c_int,
        tp: Option<*mut termios>,
    ) -> c_int {
        let tp = tp.unwrap_or(self.oterm.as_mut_ptr());
        let mut sa = unsafe { MaybeUninit::<sigaction>::zeroed().assume_init() };
        let mut osa = MaybeUninit::<sigaction>::uninit();
        let mut rc: c_int = 0;

        // If we receive SIGTTOU from tcsetattr() it means we are
        // not in the foreground process group.
        // This should be less racy than using tcgetpgrp().
        unsafe { sigemptyset(&mut sa.sa_mask) };
        self.got_sigttou = false;
        unsafe { sigaction(SIGTTOU, &sa, osa.as_mut_ptr()) };
        let osa = unsafe { osa.assume_init() };
        while {
            match cerr(unsafe { tcsetattr(fd, flags, tp) }) {
                Ok(new_rc) => {
                    rc = new_rc;
                    rc != 0 && !self.got_sigttou
                }
                Err(err) => err.kind() == io::ErrorKind::Interrupted,
            }
        } {}

        unsafe { sigaction(SIGTTOU, &osa, std::ptr::null_mut()) };

        rc
    }
}

pub struct WinSize {
    wsize: winsize,
}

impl WinSize {
    pub fn get<F: AsRawFd>(fd: &F) -> io::Result<Self> {
        let fd = fd.as_raw_fd();
        let mut wsize = MaybeUninit::<winsize>::uninit();

        cerr(unsafe { ioctl(fd, TIOCGWINSZ, wsize.as_mut_ptr()) })?;

        Ok(Self {
            wsize: unsafe { wsize.assume_init() },
        })
    }

    pub fn set<F: AsRawFd>(&self, fd: &F) -> io::Result<()> {
        let fd = fd.as_raw_fd();
        cerr(unsafe { ioctl(fd, TIOCSWINSZ, &self.wsize) })?;
        Ok(())
    }

    pub fn rows(&self) -> c_ushort {
        self.wsize.ws_row
    }

    pub fn cols(&self) -> c_ushort {
        self.wsize.ws_col
    }
}
