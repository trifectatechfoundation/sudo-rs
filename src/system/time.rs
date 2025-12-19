use std::{
    io::{Read, Write},
    mem::MaybeUninit,
    ops::{Add, Sub},
    time::Duration,
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

    #[inline]
    fn checked_add(self, rhs: Duration) -> Option<SystemTime> {
        let rhs_secs = rhs.as_secs().try_into().ok()?;
        let rhs_nsecs = rhs.subsec_nanos().into();

        let secs = self.secs.checked_add(rhs_secs)?;
        let nsecs = self.nsecs.checked_add(rhs_nsecs)?;

        Some(SystemTime::new(secs, nsecs))
    }

    #[inline]
    fn checked_sub(self, rhs: Duration) -> Option<SystemTime> {
        let rhs_secs = rhs.as_secs().try_into().ok()?;
        let rhs_nsecs = rhs.subsec_nanos().into();

        let secs = self.secs.checked_sub(rhs_secs)?;
        let nsecs = self.nsecs.checked_sub(rhs_nsecs)?;

        Some(SystemTime::new(secs, nsecs))
    }
}

impl Add<Duration> for SystemTime {
    type Output = SystemTime;

    #[inline]
    fn add(self, rhs: Duration) -> Self::Output {
        self.checked_add(rhs)
            .expect("overflow when adding duration")
    }
}

impl Sub<Duration> for SystemTime {
    type Output = SystemTime;

    #[inline]
    fn sub(self, rhs: Duration) -> Self::Output {
        self.checked_sub(rhs)
            .expect("overflow when subtracting duration")
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
    fn test_new_system_time() {
        assert_eq!(SystemTime::new(-1, 2_000_000_000), SystemTime::new(1, 0));
        assert_eq!(
            SystemTime::new(2, -500_000_000),
            SystemTime::new(1, 500_000_000)
        );
    }

    #[test]
    fn test_time_ops() {
        assert_eq!(
            SystemTime::new(0, 0) + Duration::from_secs(3),
            SystemTime::new(3, 0)
        );
        assert_eq!(
            SystemTime::new(0, 500_000_000) + Duration::from_nanos(2_500_000_000),
            SystemTime::new(3, 0)
        );
        assert_eq!(
            SystemTime::new(10, 0) - Duration::from_secs(4),
            SystemTime::new(6, 0)
        );
        assert_eq!(
            SystemTime::new(10, 0) - Duration::from_nanos(3_500_000_000),
            SystemTime::new(6, 500_000_000)
        );
    }

    #[test]
    fn get_process_start_time() {
        use crate::system::{Process, WithProcess};
        let time = Process::starting_time(WithProcess::Current).unwrap();

        let now = {
            let super::SystemTime { secs, nsecs } = super::SystemTime::now().unwrap();
            super::ProcessCreateTime { secs, nsecs }
        };

        assert!(time.secs > now.secs - 24 * 60 * 60);
        assert!(time < now);
    }
}
