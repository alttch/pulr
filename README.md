# Pulr - pull fieldbus devices and generate events

<img src="https://img.shields.io/badge/license-Apache%202.0-green" /> <img
src="https://img.shields.io/badge/rust-2018-pink.svg" />

## What is Pulr

Pulr is a small command-line tool, which can pull data from the device and
convert it into events. Meaning the data is outputted ONLY when it's changed.

The data is always being outputted to STDOUT, you can grep it or use any
pipeline processor (I use [vector](https://vector.dev/)) to push it via HTTP,
into databases etc.

Before the output, any data part can be transformed: converted, divided,
multiplied, rounded and so on.

The data can be outputted as plain text or as nd-json.

```shell
pulr -F modbus.yml -L
```

```json
{"id":"sensor:axon/din1.value","value":0}
{"id":"sensor:axon/din2.value","value":1}
{"id":"sensor:axon/din3.value","value":1}
{"id":"sensor:axon/din4.value","value":0}
{"id":"unit:axon/dout1.status","value":1}
{"id":"unit:axon/dout2.status","value":1}
{"id":"unit:axon/dout3.status","value":0}
{"id":"unit:axon/dout4.status","value":0}
{"id":"unit:axon/aout.value","value":0.00619}
{"id":"sensor:axon/ain.value","value":5.2045}
{"id":"unit:tests/u1.status","value":0}
{"id":"unit:tests/u2.status","value":0}
```

## Installing

[Download a static binary from releases
page](https://github.com/alttch/pulr/releases) and enjoy.

## Building from source

Ethernet/IP support requires
[libplctag](https://github.com/libplctag/libplctag), download and install it:

```shell
git clone https://github.com/libplctag/libplctag
cd libplctag
cmake CMakeLists.txt
make
sudo make install
sudo ldconfig
```

After, either run "make release\_x86\_64" / "make release\_armhf" or use a
proper build\*.rs file and build it with *cargo*:

```shell
cp -vf build-x86_64.rs build.rs
cargo build --release # or custom options
```

## Configuring

Look in ./examples for the example configurations.

## How does it work

One Pulr instance pulls one piece of the hardware equipment. The goal is to
pull and process the data as fast as possible, but die as soon as any errors
occur. Pulr is built to be started by supervisor, which collects the data from
it and restarts the process on crashes.

## Is it fast enough?

Pulr is written in Rust. So it's rocket-fast and super memory efficient. You
can start a hundred of Pulr processes on a single machine and barely notice any
load.

The first (draft) Pulr version was written in Python, it isn't supported any
longer, but kept in "legacy0" branch.

## Protocols

Currently supported:

* Modbus (TCP only)
* SNMP (v2)
* Ethernet/IP (Allen Bradley-compatible)

## Data transformers

* **calc\_speed** - calculate value growing speed, useful for SNMP interface
  counters

* **multiply**, **divide**, **round**

## Output type

* text (aliases: stdout, plain, "-") - output the data as plain text, default
* ndjson (alias: json) - output the data as newline delimited JSON
* csv - comma-separated values
* eva/datapuller - specific type for [EVA ICS](https://www.eva-ics.com/)

Optional field "time-format" adds time to data output. Valid values are:
"rfc3339", "timestamp" (alias: raw).

### JSON output customization

By default, data in JSON is outputted as

```json
{ "time": "time rfc 3339/timestamp", "id": "metric id", "value": "event value" }
{ "time": "time rfc 3339/timestamp", "id": "metric id", "value": "event value" }
{ "time": "time rfc 3339/timestamp", "id": "metric id", "value": "event value" }
```

Specifying output format as **ndjson/short** (aliases: *ndjson/s*,
*json/short*, *json/s*), this can be switched to "short" format, with only 2
fields: "time" and "metric id":

```json
{ "time": "time rfc 3339/timestamp", "metric id": "event value" }
{ "time": "time rfc 3339/timestamp", "metric id": "event value" }
{ "time": "time rfc 3339/timestamp", "metric id": "event value" }
```

## Real life example

### Running manually

Consider you have a device, want to collect metrics from it and store them into
[InfluxDB](https://www.influxdata.com/). Pulr comes with a tiny tool called
**ndj2influx**, which allows parsing NDJSON data and storing it directly into
InfluxDB.

It's highly recommended to set "time-format" (rfc3339 or raw, doesn't matter)
to have metrics stored with the same timestamp the pull request has been
performed.

And full command to get metrics and store will be:

```shell
pulr -F /path/to/pulr-config.yml -L -O ndjson | \
     ndj2influx http://<INFLUXDB-IP-OR-HOST>:8086 \
         <DATABASE_NAME> @device1 -U <DB_USERNAME>:<PASSWORD> -M id -v
```

Let's explain all options:

* pulr option *-L* tells Pulr to work in loop, continuously pulling the device.

* pulr option *-O ndjson* makes sure Pulr will output data in NDJSON format.

* first two ndj2influx options specify InfluxDB API URL and database name

* the next option should specify base metric column (device name). As pulr
  doesn't input it, set it for all metrics to *@device1*

* option *-U* is used to pass InfluxDB authentication. If auth isn't turned on,
  the option is not requited.

* option *-M id* is to use "id" column as metric id.

* option *-v* is for verbose output and can be omitted in production.

The same result is produced with commands:

```shell
pulr -F /path/to/pulr-config.yml -L -O ndjson/short | \
     ndj2influx http://<INFLUXDB-IP-OR-HOST>:8086 \
         <DATABASE_NAME> @device1 -U <DB_USERNAME>:<PASSWORD> -v
```

everything is almost the same, except Pulr is told to produce "short"
(id=value) JSON output and option *-M id* for ndj2influx can be omitted.

### Running with supervisor

Both tools will crash as soon as any problem occurs. They're made this way,
because in production "it's better crash and be restarted than freeze".

To automatically restart the tools, let's use any supervisor, e.g.
[Supervisord](http://supervisord.org/) (available in almost all Linux distros).

Create a simple config and put it to */etc/supervisor/conf.d/pulr-device1.conf*

```ini
[program:pulr-device1]
command=sh -c "sleep 1 && pulr -F /path/to/pulr-config.yml -L -O ndjson | ndj2influx http://<dbhost>:8086 pulr @router1 -U <DB_USERNAME>:<PASSWORD> -M id -v"
autorestart=true
autostart=true
priority=100
events=PROCESS_STATE
```

That's all. Supervisord will monitor the processes and restart them if
necessary.

## Rust version difference

As it was mentioned above, Rust version is fast. It's very fast and efficient.
However it differs from the draft Python version:

* "version" field in configuration file should be set to "2"

* "transform" syntax was changed a little bit, see config examples. "speed"
  function renamed to "calc\_speed".

* more command line args.

* No more "bit2int" and "int2bit" transformers, it's hardly to imagine where
  they could be useful.

* Modbus via UDP is no longer supported, use TCP instead. Only single Modbus
  unit can be pulled per process (look *examples/modbus.yml*).

## Troubleshooting

Before reporting a problem, try running Pulr with verbose output ("-v" command
line flag).

## Bugs, feature requests, patches

You welcome.

Just:

* Outputs. No outputs are planned, except STDOUT. Use pipeline converters.

* Pulling more than one device simultaneously. Isn't planned, start another
  Pulr process instead.

