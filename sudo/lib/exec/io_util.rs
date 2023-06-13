use std::io;

/// Return `true` if the IO error is an interruption.
pub(super) fn was_interrupted(err: &io::Error) -> bool {
    // ogsudo checks against `EINTR` and `EAGAIN`.
    matches!(
        err.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
    )
}

/// Call `f` repeatedly until it succeds or it encounters a non-interruption error.
pub(super) fn retry_while_interrupted<T>(mut f: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match f() {
            Err(err) if was_interrupted(&err) => {}
            result => return result,
        }
    }
}
