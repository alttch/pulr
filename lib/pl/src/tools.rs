use std::io::{stderr, stdout, Write};
use std::thread::sleep;
use std::time::Instant;

// parse host / port
pub struct HostPort {
    pub host: String,
    pub port: u16,
}

impl Default for HostPort {
    fn default() -> Self {
        HostPort {
            host: "".to_owned(),
            port: 0,
        }
    }
}

pub trait ParseHostPort {
    fn parse_host_port(&self, default_port: u16) -> HostPort;
}

impl ParseHostPort for String {
    fn parse_host_port(&self, default_port: u16) -> HostPort {
        let mut i = self.split(":");
        let host: String = i.next().unwrap().to_string();
        let port: u16 = match i.next() {
            Some(v) => v.parse().unwrap(),
            None => default_port,
        };
        return HostPort {
            host: host,
            port: port,
        };
    }
}

// bit manipulations
pub trait GetBit {
    fn get_bit(&self, bit: u8) -> u8;
}

impl GetBit for u16 {
    fn get_bit(&self, bit: u8) -> u8 {
        return (*self >> bit & 1) as u8;
    }
}

// flushed output
const LF: [u8; 1] = [10];

pub fn oprint(content: String) {
    let mut out = stdout();
    out.write(content.as_bytes()).unwrap();
    out.write(&LF).unwrap();
    out.flush().unwrap();
}

pub fn eprint(content: String) {
    let mut out = stderr();
    out.write(content.as_bytes()).unwrap();
    out.write(&LF).unwrap();
    out.flush().unwrap();
}

// sleep funcs
pub trait SleepTricks {
    fn sleep_until(&self) -> ();
}

impl SleepTricks for std::time::Instant {
    fn sleep_until(&self) {
        let t = Instant::now();
        if t >= *self {
            eprint("WARNING: loop timeout".to_owned());
        } else {
            sleep(*self - t);
        }
    }
}
