#[path = "datatypes.rs"]
pub mod datatypes;
#[path = "tools.rs"]
pub mod tools;

#[macro_use]
extern crate lazy_static;

use tools::SleepTricks;

use colored::*;
use std::time::{Duration, Instant};

use std::collections::HashMap;

use datatypes::{Event, OutputType};
use tools::oprint;
use transform;

use std::sync::RwLock;

pub fn init() {
    #[cfg(windows)]
    colored::control::set_override(false);
}

pub fn print_debug(s: &String) {
    oprint(s.cyan().dimmed().to_string());
}

pub struct Beacon {
    tp: OutputType,
    beacon_interval: Duration,
    next_beacon: Instant,
    enabled: bool,
}

impl Beacon {
    pub fn new(output_type: OutputType, beacon_interval: Duration) -> Self {
        return Beacon {
            tp: output_type,
            beacon_interval,
            next_beacon: Instant::now() + beacon_interval,
            enabled: beacon_interval.as_micros() > 0,
        };
    }
    pub fn ping(&mut self) {
        if self.enabled {
            let t = Instant::now();
            if self.next_beacon < t {
                match self.tp {
                    OutputType::Stdout
                    | OutputType::StdoutCsv
                    | OutputType::StdoutNdJson
                    | OutputType::StdoutEvaDatapuller => beacon_stdout(),
                }
                while self.next_beacon < t {
                    self.next_beacon = self.next_beacon + self.beacon_interval;
                }
            }
        }
    }
}

pub struct EventTimer {
    last_event_time: Instant,
}

impl EventTimer {
    pub fn new() -> Self {
        Self {
            last_event_time: Instant::now(),
        }
    }

    pub fn trigger(&mut self) {
        self.last_event_time = Instant::now();
    }

    pub fn since_event(&self) -> Duration {
        Instant::now() - self.last_event_time
    }
}

use std::cell::RefCell;
thread_local!(static EVENT_CACHE: RefCell<HashMap<u64, String>> = RefCell::new(HashMap::new()));

lazy_static! {
    static ref EVENT_TIMER: RwLock<EventTimer> = RwLock::new(EventTimer::new());
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Core {
    tp: OutputType,
    event_output_flags: datatypes::OutputFlags,
    time_format: datatypes::TimeFormat,
    event_timeout: Option<Duration>,
}

impl Core {
    pub fn new(
        output_type: OutputType,
        event_output_flags: datatypes::OutputFlags,
        time_format: datatypes::TimeFormat,
        event_timeout: Option<Duration>,
    ) -> Self {
        if event_timeout.is_some() {
            EVENT_TIMER.write().unwrap().trigger();
        }
        Self {
            tp: output_type,
            event_output_flags,
            time_format,
            event_timeout,
        }
    }

    pub fn create_event_time(self) -> datatypes::EventTime {
        return datatypes::EventTime::new(self.time_format);
    }

    pub fn create_event<'a, T: ToString + transform::Transform>(
        self,
        id: &'a String,
        value: T,
        transform: &'a datatypes::EventTransformList,
        t: &'a datatypes::EventTime,
    ) -> Event<'a, T> {
        return Event::new(id, value, transform, t, self.event_output_flags);
    }

    pub fn output<T: serde::Serialize + std::fmt::Display + transform::Transform>(
        self,
        event: &Event<T>,
    ) {
        self._output(event, 0);
    }

    pub fn since_event(self) -> Option<Duration> {
        match self.event_timeout {
            Some(_) => Some(EVENT_TIMER.read().unwrap().since_event()),
            None => None,
        }
    }

    pub fn is_event_timeout(self) -> bool {
        match self.event_timeout {
            Some(v) => EVENT_TIMER.read().unwrap().since_event() > v,
            None => false,
        }
    }

    fn _output<T: serde::Serialize + std::fmt::Display + transform::Transform>(
        self,
        event: &Event<T>,
        ti: usize,
    ) {
        // transform if required
        if event.transform_list.len() > ti {
            match event.transform_at(ti) {
                Some(ev) => self._output(&ev, ti + 1),
                None => {}
            }
            return;
        }
        //
        let val = event.value.to_string();
        EVENT_CACHE.with(|event_cache_cell| {
            let mut cache = event_cache_cell.borrow_mut();
            let prev = cache.get(&event.id_hash);
            if !prev.is_some() || *prev.unwrap() != val {
                cache.insert(event.id_hash, val);
                if self.event_timeout.is_some() {
                    EVENT_TIMER.write().unwrap().trigger();
                }
                match self.tp {
                    OutputType::Stdout => output_stdout(event),
                    OutputType::StdoutCsv => output_stdout_csv(event),
                    OutputType::StdoutNdJson => output_stdout_ndjson(event),
                    OutputType::StdoutEvaDatapuller => output_eva_datapuller(event),
                };
            }
        });
    }

    pub fn clear_event_cache(self) {
        EVENT_CACHE.with(|event_cache_cell| {
            let mut cache = event_cache_cell.borrow_mut();
            cache.clear();
        });
    }
}

pub fn output_stdout<T: ToString>(event: &Event<T>) {
    let mut t = event.t.to_string();
    if t != "" {
        t = t + " "
    }
    let s = format!(
        "{}{} {}",
        t.white().dimmed(),
        event.id.blue().bold(),
        event.value.to_string().yellow()
    );
    oprint(s);
}

pub fn output_stdout_csv<T: std::fmt::Display>(event: &Event<T>) {
    let mut t = event.t.to_string();
    if t != "" {
        t = t + ";"
    }
    let s = format!("{}{};{}", t, event.id, event.value);
    oprint(s);
}

pub fn output_stdout_ndjson<T: serde::Serialize + std::fmt::Display>(event: &Event<T>) {
    oprint(serde_json::to_string(event).unwrap());
}

pub fn output_eva_datapuller<T: std::fmt::Display>(event: &Event<T>) {
    if event.id.ends_with(".value") {
        oprint(format!(
            "{} u None {}",
            &event.id[..event.id.len() - 6],
            event.value
        ));
    } else if event.id.ends_with(".status") {
        oprint(format!(
            "{} u {}",
            &event.id[..event.id.len() - 7],
            event.value
        ));
    } else {
        oprint(format!("{} u None {}", event.id, event.value));
    }
}

pub fn beacon_stdout() {
    oprint("".to_owned());
}

// workers
pub struct IntervalLoop {
    next_iter: Instant,
    interval: Duration,
}

impl IntervalLoop {
    pub fn new(interval: Duration) -> Self {
        return IntervalLoop {
            next_iter: Instant::now() + interval,
            interval,
        };
    }

    pub fn sleep(&mut self) -> bool {
        let result = self.next_iter.sleep_until();
        if result {
            self.next_iter = self.next_iter + self.interval;
        } else {
            tools::eprint(format!(
                "WARNING: loop timeout ({} ms + {} ms)",
                self.interval.as_micros() as f64 / 1000.0,
                (Instant::now() - self.next_iter).as_micros() as f64 / 1000.0
            ));
            self.next_iter = Instant::now() + self.interval
        }
        result
    }
}
