mod epoll;
mod futex;

#[cfg(test)]
mod tests;

// TODO:

#[macro_export]
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* ) ) => {{
        #[allow(clippy::macro_metavars_in_unsafe)]
        let res = unsafe { libc::$fn($($arg,)*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}
