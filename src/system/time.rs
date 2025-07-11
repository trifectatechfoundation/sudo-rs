use std::{
    io::{Read, Write},
    mem::MaybeUninit,
    ops::{Add, Sub},
};

/// A timestamp relative to `CLOCK_BOOTTIME`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemTime {
    secs: i64,
    nsecs: i64,
}

impl SystemTime {
    pub(super) fn new(secs: i64, nsecs: i64) -> SystemTime {
        SystemTime {
            secs: secs + nsecs.div_euclid(1_000_000_000),
            nsecs: nsecs.rem_euclid(1_000_000_000),
        }
    }

    pub fn now() -> std::io::Result<SystemTime> {
        let mut spec = MaybeUninit::<libc::timespec>::uninit();
        // SAFETY: valid pointer is passed to clock_gettime
        crate::cutils::cerr(unsafe {
            libc::clock_gettime(libc::CLOCK_BOOTTIME, spec.as_mut_ptr())
        })?;
        // SAFETY: The `libc::clock_gettime` will correctly initialize `spec`,
        // otherwise it will return early with the `?` operator.
        let spec = unsafe { spec.assume_init() };
        Ok(spec.into())
    }

    pub(super) fn encode(&self, target: &mut impl Write) -> std::io::Result<()> {
        let secs = self.secs.to_ne_bytes();
        let nsecs = self.nsecs.to_ne_bytes();
        target.write_all(&secs)?;
        target.write_all(&nsecs)?;
        Ok(())
    }

    pub(super) fn decode(from: &mut impl Read) -> std::io::Result<SystemTime> {
        let mut sec_bytes = [0; 8];
        let mut nsec_bytes = [0; 8];

        from.read_exact(&mut sec_bytes)?;
        from.read_exact(&mut nsec_bytes)?;

        Ok(SystemTime::new(
            i64::from_ne_bytes(sec_bytes),
            i64::from_ne_bytes(nsec_bytes),
        ))
    }
}

impl Sub<SystemTime> for SystemTime {
    type Output = Duration;

    fn sub(self, rhs: SystemTime) -> Self::Output {
        Duration::new(self.secs - rhs.secs, self.nsecs - rhs.nsecs)
    }
}

impl Add<Duration> for SystemTime {
    type Output = SystemTime;

    fn add(self, rhs: Duration) -> Self::Output {
        SystemTime::new(self.secs + rhs.secs, self.nsecs + rhs.nsecs)
    }
}

impl Sub<Duration> for SystemTime {
    type Output = SystemTime;

    fn sub(self, rhs: Duration) -> Self::Output {
        SystemTime::new(self.secs - rhs.secs, self.nsecs - rhs.nsecs)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Duration {
    secs: i64,
    nsecs: i64,
}

impl Duration {
    pub fn new(secs: i64, nsecs: i64) -> Duration {
        Duration {
            secs: secs + nsecs.div_euclid(1_000_000_000),
            nsecs: nsecs.rem_euclid(1_000_000_000),
        }
    }

    pub fn seconds(secs: i64) -> Duration {
        Duration::new(secs, 0)
    }

    #[cfg(test)]
    pub fn minutes(minutes: i64) -> Duration {
        Duration::seconds(minutes * 60)
    }

    #[cfg(test)]
    pub fn milliseconds(ms: i64) -> Duration {
        let secs = ms / 1000;
        let ms = ms % 1000;
        Duration::new(secs, ms * 1_000_000)
    }
}

impl Add<Duration> for Duration {
    type Output = Duration;

    fn add(self, rhs: Duration) -> Self::Output {
        Duration::new(self.secs + rhs.secs, self.nsecs + rhs.nsecs)
    }
}

impl Sub<Duration> for Duration {
    type Output = Duration;

    fn sub(self, rhs: Duration) -> Self::Output {
        Duration::new(self.secs - rhs.secs, self.nsecs - rhs.nsecs)
    }
}

impl From<Duration> for std::time::Duration {
    fn from(dur: Duration) -> std::time::Duration {
        std::time::Duration::new(
            dur.secs.try_into().unwrap_or(0),
            dur.nsecs.try_into().unwrap_or(0),
        )
    }
}

impl From<libc::timespec> for SystemTime {
    #[allow(clippy::useless_conversion)]
    fn from(value: libc::timespec) -> Self {
        SystemTime::new(value.tv_sec.into(), value.tv_nsec.into())
    }
}

/// A timestamp relative to `CLOCK_BOOTTIME` on Linux and relative to `CLOCK_REALTIME` on FreeBSD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessCreateTime {
    secs: i64,
    nsecs: i64,
}

impl ProcessCreateTime {
    pub fn new(secs: i64, nsecs: i64) -> ProcessCreateTime {
        ProcessCreateTime {
            secs: secs + nsecs.div_euclid(1_000_000_000),
            nsecs: nsecs.rem_euclid(1_000_000_000),
        }
    }

    #[cfg(test)]
    pub(super) fn now() -> std::io::Result<ProcessCreateTime> {
        let mut spec = MaybeUninit::<libc::timespec>::uninit();
        // SAFETY: valid pointer is passed to clock_gettime
        crate::cutils::cerr(unsafe {
            libc::clock_gettime(
                if cfg!(target_os = "freebsd") {
                    libc::CLOCK_REALTIME
                } else {
                    libc::CLOCK_BOOTTIME
                },
                spec.as_mut_ptr(),
            )
        })?;
        // SAFETY: The `libc::clock_gettime` will correctly initialize `spec`,
        // otherwise it will return early with the `?` operator.
        let spec = unsafe { spec.assume_init() };

        // the below conversion is not as useless as clippy thinks, on 32bit systems
        #[allow(clippy::useless_conversion)]
        Ok(ProcessCreateTime::new(
            spec.tv_sec.into(),
            spec.tv_nsec.into(),
        ))
    }

    #[cfg(test)]
    pub(super) fn secs(&self) -> i64 {
        self.secs
    }

    pub(super) fn encode(&self, target: &mut impl Write) -> std::io::Result<()> {
        let secs = self.secs.to_ne_bytes();
        let nsecs = self.nsecs.to_ne_bytes();
        target.write_all(&secs)?;
        target.write_all(&nsecs)?;
        Ok(())
    }

    pub(super) fn decode(from: &mut impl Read) -> std::io::Result<ProcessCreateTime> {
        let mut sec_bytes = [0; 8];
        let mut nsec_bytes = [0; 8];

        from.read_exact(&mut sec_bytes)?;
        from.read_exact(&mut nsec_bytes)?;

        Ok(ProcessCreateTime::new(
            i64::from_ne_bytes(sec_bytes),
            i64::from_ne_bytes(nsec_bytes),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_durations_and_times() {
        assert_eq!(Duration::new(1, 1_000_000_000), Duration::seconds(2));
        assert_eq!(
            Duration::new(-2, 500_000_000),
            Duration::seconds(-1) + Duration::milliseconds(-500)
        );

        assert_eq!(SystemTime::new(-1, 2_000_000_000), SystemTime::new(1, 0));
        assert_eq!(
            SystemTime::new(2, -500_000_000),
            SystemTime::new(1, 500_000_000)
        );
    }

    #[test]
    fn test_time_ops() {
        assert_eq!(
            Duration::seconds(2) + Duration::seconds(3),
            Duration::seconds(5)
        );
        assert_eq!(
            Duration::seconds(3) - Duration::seconds(1),
            Duration::seconds(2)
        );
        assert_eq!(
            Duration::seconds(-10) + Duration::seconds(-5),
            Duration::seconds(-15)
        );
        assert_eq!(
            Duration::milliseconds(5555) + Duration::milliseconds(5555),
            Duration::seconds(11) + Duration::milliseconds(110)
        );
        assert_eq!(
            Duration::milliseconds(-5555) + Duration::milliseconds(-1111),
            Duration::milliseconds(-6666)
        );
        assert_eq!(
            Duration::seconds(10) - Duration::seconds(-5),
            Duration::seconds(15)
        );

        assert_eq!(
            SystemTime::new(0, 0) + Duration::seconds(3),
            SystemTime::new(3, 0)
        );
        assert_eq!(
            SystemTime::new(10, 0) - Duration::seconds(4),
            SystemTime::new(6, 0)
        );
    }
}
