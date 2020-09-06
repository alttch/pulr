// TODO: speed transform
#[path = "datatypes.rs"]
pub mod datatypes;
#[path = "tools.rs"]
pub mod tools;

use tools::SleepTricks;

use colored::*;
use std::time::{Duration, Instant};

use std::collections::HashMap;

use datatypes::{Event, OutputType};
use tools::oprint;
use transform;

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
            beacon_interval: beacon_interval,
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

use std::cell::RefCell;
thread_local!(static EVENT_CACHE: RefCell<HashMap<u64, String>> = RefCell::new(HashMap::new()));

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Output {
    tp: OutputType,
}

impl Output {
    pub fn new(output_type: OutputType) -> Self {
        return Output { tp: output_type };
    }

    pub fn output<T: serde::Serialize + std::fmt::Display + transform::Transform>(
        self,
        event: &Event<T>,
    ) {
        self._output(event, 0);
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
                match self.tp {
                    OutputType::Stdout => output_stdout(event),
                    OutputType::StdoutCsv => output_stdout_csv(event),
                    OutputType::StdoutNdJson => output_stdout_ndjson(event),
                    OutputType::StdoutEvaDatapuller => output_eva_datapuller(event),
                };
            }
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
        oprint(format!("{} u {}", event.id, event.value));
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
            interval: interval,
        };
    }

    pub fn sleep(&mut self) {
        self.next_iter.sleep_until();
        self.next_iter = self.next_iter + self.interval;
    }
}
