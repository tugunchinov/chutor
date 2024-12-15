use crate::syscall;
use std::os::fd::RawFd;

pub struct Epoll {
    fd: RawFd,
}

impl Epoll {
    const MAX_EVENTS: libc::c_int = 1024;
    const TIMEOUT: libc::c_int = 1000;

    pub(crate) fn new() -> std::io::Result<Self> {
        let fd = syscall!(epoll_create1(libc::EPOLL_CLOEXEC))?;
        Ok(Self { fd })
    }

    fn get_read_event_one_shot(data: u64) -> libc::epoll_event {
        libc::epoll_event {
            events: (libc::EPOLLONESHOT | libc::EPOLLIN) as u32,
            u64: data,
        }
    }

    fn get_write_event_one_shot(data: u64) -> libc::epoll_event {
        libc::epoll_event {
            events: (libc::EPOLLONESHOT | libc::EPOLLOUT) as u32,
            u64: data,
        }
    }

    /// One-shot
    pub(crate) fn add_read_interest_one_shot(&self, fd: RawFd, data: u64) -> std::io::Result<()> {
        let mut event = Self::get_read_event_one_shot(data);
        syscall!(epoll_ctl(self.fd, libc::EPOLL_CTL_ADD, fd, &mut event))?;
        Ok(())
    }

    pub(crate) fn enable_read_interest_one_shot(
        &self,
        fd: RawFd,
        data: u64,
    ) -> std::io::Result<()> {
        let mut event = Self::get_read_event_one_shot(data);
        syscall!(epoll_ctl(self.fd, libc::EPOLL_CTL_MOD, fd, &mut event))?;
        Ok(())
    }

    pub(crate) fn enable_write_interest_one_shot(
        &self,
        fd: RawFd,
        data: u64,
    ) -> std::io::Result<()> {
        let mut event = Self::get_write_event_one_shot(data);
        syscall!(epoll_ctl(self.fd, libc::EPOLL_CTL_MOD, fd, &mut event))?;
        Ok(())
    }

    pub(crate) fn remove_interest(&self, fd: RawFd) -> std::io::Result<()> {
        syscall!(epoll_ctl(
            self.fd,
            libc::EPOLL_CTL_DEL,
            fd,
            std::ptr::null_mut()
        ))?;
        Ok(())
    }

    pub(crate) fn wait(&self) -> std::io::Result<Vec<libc::epoll_event>> {
        let mut events_buf = Vec::with_capacity(Self::MAX_EVENTS as usize);
        let events_buf_ptr = events_buf.as_mut_ptr();

        let ready_events_cnt = syscall!(epoll_wait(
            self.fd,
            events_buf_ptr,
            Self::MAX_EVENTS,
            Self::TIMEOUT
        ))?;

        unsafe { events_buf.set_len(ready_events_cnt as usize) };

        Ok(events_buf)
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        syscall!(close(self.fd)).expect("failed to close syscall handle");
    }
}
