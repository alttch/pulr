// TODO: move "if verbose" to macro
use argparse::{ArgumentParser, Store, StoreTrue};
use serde::{Deserialize, Deserializer};
use std::fs;
use std::time::Duration;

use pl;

#[path = "proto/common.rs"]
#[macro_use]
mod common;

#[path = "proto/ppmodbus.rs"]
mod ppmodbus;

#[path = "proto/ppenip.rs"]
mod ppenip;

#[path = "proto/ppsnmp.rs"]
mod ppsnmp;

const HOMEPAGE: &str = "https://github.com/alttch/pulr";
const VERSION: &str = "1.0.2";

fn get_default_timeout() -> f32 {
    return 5.0;
}

fn get_default_beacon() -> f32 {
    return 2.0;
}

const CFG_VERSION_MIN: u16 = 2;
const CFG_VERSION_MAX: u16 = 2;

#[derive(Deserialize)]
struct GenProto {
    name: String,
}

fn de_output<'de, D>(
    deserializer: D,
) -> serde::export::Result<(pl::datatypes::OutputType, pl::datatypes::OutputFlags), D::Error>
where
    D: Deserializer<'de>,
{
    return Ok(
        pl::datatypes::get_output_type(&String::deserialize(deserializer).unwrap()).unwrap(),
    );
}

fn de_time_format<'de, D>(
    deserializer: D,
) -> serde::export::Result<pl::datatypes::TimeFormat, D::Error>
where
    D: Deserializer<'de>,
{
    return Ok(
        pl::datatypes::get_time_format(&String::deserialize(deserializer).unwrap()).unwrap(),
    );
}

#[derive(Deserialize)]
struct Config {
    version: u16,
    #[serde(default = "get_default_timeout")]
    timeout: f32,
    #[serde(default = "get_default_beacon")]
    beacon: f32,
    freq: u16,
    #[serde(
        default = "pl::datatypes::get_default_output",
        deserialize_with = "de_output"
    )]
    output: (pl::datatypes::OutputType, pl::datatypes::OutputFlags),
    proto: GenProto,
    #[serde(
        alias = "time-format",
        default = "pl::datatypes::get_default_time_format",
        deserialize_with = "de_time_format"
    )]
    time_format: pl::datatypes::TimeFormat,
}

fn main() {
    #[cfg(windows)]
    colored::control::set_override(false);

    let mut in_loop = false;
    let mut verbose = false;
    let mut cfgfile = String::new();
    let mut output_type = String::new();
    let greeting = format!("Pulr v{} ({})", VERSION, HOMEPAGE);
    {
        let mut ap = ArgumentParser::new();
        ap.set_description(greeting.as_str());
        ap.refer(&mut in_loop)
            .add_option(&["-L", "--loop"], StoreTrue, "Loop (production)");
        ap.refer(&mut verbose).add_option(
            &["-v", "--verbose"],
            StoreTrue,
            "Verbose output (debug)",
        );
        ap.refer(&mut cfgfile)
            .add_option(&["-F", "--config"], Store, "Configuration file")
            .metavar("CONFIG")
            .required();
        ap.refer(&mut output_type)
            .add_option(&["-O", "--output"], Store, "Override output type")
            .metavar("TYPE");
        ap.parse_args_or_exit();
    }
    let cfg = fs::read_to_string(cfgfile).expect("Config not found!");
    let config: Config = serde_yaml::from_str(&cfg).unwrap();
    if config.version < CFG_VERSION_MIN || config.version > CFG_VERSION_MAX {
        panic!("configuration version {} is unsupported", config.version);
    }
    let proto_name = config.proto.name.split("/").next().unwrap();
    let mut otp = config.output;
    if output_type != "" {
        otp = pl::datatypes::get_output_type(&output_type).unwrap();
    }
    let timeout = Duration::from_millis((config.timeout * 1000 as f32) as u64);
    let beacon_interval = Duration::from_micros((config.beacon * 1_000_000.0) as u64);
    let interval = Duration::from_micros((1.0 / config.freq as f64 * 1_000_000.0) as u64);
    {
        let core = pl::Core::new(otp.0, otp.1, config.time_format);
        let mut beacon = pl::Beacon::new(otp.0, beacon_interval);

        match proto_name {
            "modbus" => {
                ppmodbus::run(in_loop, verbose, cfg, timeout, interval, core, &mut beacon);
                ()
            }
            "enip" => {
                ppenip::run(in_loop, verbose, cfg, timeout, interval, core, &mut beacon);
                ()
            }
            "snmp" => {
                ppsnmp::run(in_loop, verbose, cfg, timeout, interval, core, &mut beacon);
                ()
            }
            _ => unimplemented!("protocol {}", proto_name),
        }
    }
    return;
}
