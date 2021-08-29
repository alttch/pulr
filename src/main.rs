// TODO: move "if verbose" to macro
use argparse::{ArgumentParser, Store, StoreTrue};
use serde::{Deserialize, Deserializer};
use std::time::Duration;
use std::{env, fs, io, io::Read};

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
const VERSION: &str = "1.0.13";

fn get_default_event_timeout() -> f32 {
    0.0
}

fn get_default_timeout() -> f32 {
    5.0
}

fn get_default_beacon() -> f32 {
    2.0
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
    #[serde(alias = "event-timeout", default = "get_default_event_timeout")]
    event_timeout: f32,
    #[serde(default = "get_default_beacon")]
    beacon: f32,
    freq: f64,
    resend: Option<f32>,
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
            .add_option(
                &["-F", "--config"],
                Store,
                "Configuration file ('-' for stdin)",
            )
            .metavar("CONFIG")
            .required();
        ap.refer(&mut output_type)
            .add_option(&["-O", "--output"], Store, "Override output type")
            .metavar("TYPE");
        ap.parse_args_or_exit();
    }
    pl::init();
    let cfg = match cfgfile.as_str() {
        "-" => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).unwrap();
            buf
        }
        _ => fs::read_to_string(cfgfile).expect("Config not found!"),
    };
    let config: Config = serde_yaml::from_str(&cfg).unwrap();
    if config.version < CFG_VERSION_MIN || config.version > CFG_VERSION_MAX {
        panic!("configuration version {} is unsupported", config.version);
    }
    let proto_name = config.proto.name.split('/').next().unwrap();
    let mut otp = config.output;
    if !output_type.is_empty() {
        otp = pl::datatypes::get_output_type(&output_type).unwrap();
    }
    let timeout = Duration::from_millis((config.timeout * 1_000f32) as u64);
    let beacon_interval = Duration::from_micros((config.beacon * 1_000_000.0) as u64);
    let interval = Duration::from_micros((1.0 / config.freq as f64 * 1_000_000.0) as u64);
    let resend_interval = config
        .resend
        .map(|v| Duration::from_micros((v * 1_000_000.0) as u64));
    let verbose_warnings = env::var("PULR_VERBOSE_WARNINGS").map_or(false, |v| v == "1");
    {
        let etimeout: Option<Duration>;
        if config.event_timeout > 0.0 {
            etimeout = Some(Duration::from_micros(
                (config.event_timeout as f64 * 1_000_000.0) as u64,
            ));
        } else {
            etimeout = None;
        }
        let core = pl::Core::new(otp.0, otp.1, config.time_format, etimeout);
        let mut beacon = pl::Beacon::new(otp.0, beacon_interval);

        match proto_name {
            "modbus" => {
                ppmodbus::run(
                    in_loop,
                    verbose,
                    verbose_warnings,
                    cfg,
                    timeout,
                    interval,
                    resend_interval,
                    core,
                    &mut beacon,
                );
            }
            "enip" => {
                ppenip::run(
                    in_loop,
                    verbose,
                    verbose_warnings,
                    cfg,
                    timeout,
                    interval,
                    resend_interval,
                    core,
                    &mut beacon,
                );
            }
            "snmp" => {
                ppsnmp::run(
                    in_loop,
                    verbose,
                    verbose_warnings,
                    cfg,
                    timeout,
                    interval,
                    resend_interval,
                    core,
                    &mut beacon,
                );
            }
            _ => unimplemented!("protocol {}", proto_name),
        }
    }
}
