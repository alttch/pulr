# Pulr - pull devices and transform data into events

## What is Pulr

Pulr is a small command-line tool, which can pull data from the device and
convert it into events. Meaning the data is output ONLY when it's changed.

The data is always being output to STDOUT, you can just grep it or use any
pipeline processor (I use vector: https://vector.dev/) to push it via HTTP,
into databases etc.

Before the output, any data part can be transformed: converted, divided,
multiplied, rounded and so on.

The data can be output as plain text or as nd-json.

```shell
pulr -F modbus.yml -L
```

```json
{"id":"sensor:axon/din1.value","v":0}
{"id":"sensor:axon/din2.value","v":0}
{"id":"sensor:axon/din3.value","v":0}
{"id":"sensor:axon/din4.value","v":0}
{"id":"unit:axon/dout1.status","v":0}
{"id":"unit:axon/dout2.status","v":0}
{"id":"unit:axon/dout3.status","v":0}
{"id":"unit:axon/dout4.status","v":0}
{"id":"unit:axon/aout.value","v":0.00619}
{"id":"sensor:axon/ain.value","v":5.2045}
{"id":"unit:tests/u1.status","v":0}
{"id":"unit:tests/u2.status","v":0}
```

## Installing

```shell
pip3 insall pulr
# for SNMP support
pip3 install python3-netsnmp
# for Modbus
pip3 install pymodbus
```

## Configuring

Look in ./examples for the example configurations.

## How does it work

One Pulr process pulls one piece of the hardware equipment. The goal is to pull
and process the data as fast as possible but die as soon as any errors occur.
Pulr is built to be started by supervisor, which collects the data from it and
restarts the process on crashes.

But it's possible to run the tool with "-R" flag, which tells Pulr to restart
the main loop in case of failures.

## Is it fast enough?

Pulr is written in Python but it's written to be fast enough (e.g. Modbus
devices can be pulled up to 50 times per second without any problem).

Pulr code is written to be easily transformed to Rust or Golang, I will do it
if the project will be successful enough.

## What protocols are supported?

Currently supported:

* Modbus (TCP/UDP)
* SNMP (v1/v2)

Planned very soon:

* Ethernet/IP (Allen Bradley ControlLogix)

## What data transformers are available

* speed - calculate value growing speed, useful for SNMP interface counters
* multiply, divide, round
* bit2int - convert boolean bits into integers (1/0)

## Bugs, feature requests, patches

You are welcome. For the patches, please avoid Python-specific coding style
(e.g. function kwargs), as Python version will be rewritten very soon.

Just:

* Outputs. No outputs are planned, except STDOUT. Use pipeline converters.

* Pulling more than one device simultaneously. Isn't planned, start another
  Pulr process instead.

