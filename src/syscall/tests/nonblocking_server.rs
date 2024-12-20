use crate::syscall::epoll::Epoll;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::process::Command;
use std::thread;

#[test]
fn hello_server() {
    start_server();

    let mut jobs = Vec::new();

    for _ in 0..=10 {
        let job = thread::spawn(|| {
            let output = unsafe {
                String::from_utf8_unchecked(
                    Command::new("curl")
                        .arg("-X")
                        .arg("POST")
                        .arg("http://localhost:8080")
                        .arg("-d")
                        .arg("@/usr/share/man/man1/echo.1.gz")
                        .output()
                        .unwrap()
                        .stdout,
                )
            };

            assert_eq!(output, "Hello");
        });

        jobs.push(job);
    }

    for j in jobs {
        j.join().unwrap();
    }
}

impl Debug for Epoll {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

#[derive(Debug)]
pub struct RequestContext<'a> {
    pub stream: TcpStream,
    pub content_length: usize,
    pub buf: Vec<u8>,
    pub epoll: &'a Epoll,
}

impl<'a> RequestContext<'a> {
    const HTTP_RESPONSE: &'static [u8] = b"HTTP/1.1 200 OK
content-type: text/html
content-length: 5

Hello";

    fn new(stream: TcpStream, epoll: &'a Epoll) -> Self {
        Self {
            stream,
            buf: Vec::new(),
            content_length: 0,
            epoll,
        }
    }

    fn read_cb(&mut self, key: u64) -> std::io::Result<()> {
        let mut buf = [0u8; 4096];
        match self.stream.read(&mut buf) {
            Ok(bytes_cnt) => {
                if let Ok(data) = std::str::from_utf8(&buf[0..100]) {
                    self.parse_and_set_content_length(data);
                }
                self.buf.extend_from_slice(&buf[..bytes_cnt]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => unreachable!(),
            Err(e) => {
                return Err(e);
            }
        };
        if self.buf.len() >= self.content_length {
            println!("got all data: {} bytes", self.buf.len());
            self.epoll
                .enable_write_interest_one_shot(self.stream.as_raw_fd(), key)?;
        } else {
            self.epoll
                .enable_read_interest_one_shot(self.stream.as_raw_fd(), key)?;
        }
        Ok(())
    }

    fn parse_and_set_content_length(&mut self, data: &str) {
        if data.contains("HTTP") {
            if let Some(content_length) = data
                .lines()
                .find(|l| l.to_lowercase().starts_with("content-length: "))
            {
                if let Some(len) = content_length
                    .to_lowercase()
                    .strip_prefix("content-length: ")
                {
                    self.content_length = len.parse::<usize>().expect("content-length is valid");
                    println!("set content length: {} bytes", self.content_length);
                }
            }
        }
    }

    fn write_cb(&mut self, key: u64) -> std::io::Result<()> {
        match self.stream.write_all(Self::HTTP_RESPONSE) {
            Ok(_) => println!("answered from request {}", key),
            Err(e) => eprintln!("could not answer to request {}, {}", key, e),
        };
        self.stream.shutdown(std::net::Shutdown::Both)?;
        let fd = self.stream.as_raw_fd();
        self.epoll.remove_interest(fd)?;
        Ok(())
    }
}

fn start_server() {
    thread::spawn(|| {
        let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
        listener.set_nonblocking(true).unwrap();
        let listener_fd = listener.as_raw_fd();
        let mut request_contexts: HashMap<u64, RequestContext> = HashMap::new();

        let epoll = Epoll::new().unwrap();
        epoll.add_read_interest_one_shot(listener_fd, 0).unwrap();
        let mut key = 1;

        loop {
            println!("requests in flight: {}", request_contexts.len());
            let events = epoll.wait().unwrap();

            for event in events {
                match event.u64 {
                    0 => {
                        match listener.accept() {
                            Ok((stream, addr)) => {
                                stream.set_nonblocking(true).unwrap();
                                println!("new client: {}", addr);
                                key += 1;
                                epoll
                                    .add_read_interest_one_shot(stream.as_raw_fd(), key)
                                    .unwrap();
                                request_contexts.insert(key, RequestContext::new(stream, &epoll));
                            }
                            Err(e) => eprintln!("couldn't accept: {}", e),
                        };
                        epoll.enable_read_interest_one_shot(listener_fd, 0).unwrap();
                    }
                    key => {
                        let mut to_delete = None;
                        if let Some(context) = request_contexts.get_mut(&key) {
                            match event.events {
                                v if v as i32 & libc::EPOLLIN == libc::EPOLLIN => {
                                    context.read_cb(key).unwrap();
                                }
                                v if v as i32 & libc::EPOLLOUT == libc::EPOLLOUT => {
                                    context.write_cb(key).unwrap();
                                    to_delete = Some(key);
                                }
                                v => println!("unexpected events: {}", v),
                            };
                        }
                        if let Some(key) = to_delete {
                            request_contexts.remove(&key);
                        }
                    }
                }
            }
        }
    });
}
