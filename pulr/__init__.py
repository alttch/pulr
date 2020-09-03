# TODO tests

__author__ = 'Altertech, https://www.altertech.com/'
__copyright__ = 'Copyright (C) 2020 Altertech'
__license__ = 'Apache License 2.0'

__version__ = '0.1.8'

import sys
import argparse
import importlib
import threading
import jsonschema

import yaml

from time import perf_counter, sleep
from .outputs import OUTPUT_METHODS, eprint, print_trace, set_time_format
from queue import Queue

q = Queue()

processor = None

fn_beacon = None

data = {}

pullers = []
processor_maps = []

DEFAULT_TIMEOUT = 5
DEFAULT_FREQUENCY = 1
DEFAULT_BEACON_INTERVAL = 0

last_pull_time = 0

config = {
    'timeout': DEFAULT_TIMEOUT,
    'freq': DEFAULT_FREQUENCY,
    'beacon': DEFAULT_BEACON_INTERVAL,
    'output': None
}

CONFIG_SCHEMA = {
    'type': 'object',
    'properties': {
        'version': {
            'type': 'integer',
            'minimum': 1
        },
        'timeout': {
            'type': 'number',
            'minimum': 0
        },
        'beacon': {
            'type': 'number',
            'minimum': 0
        },
        'freq': {
            'type': 'number',
            'minimum': 0
        },
        'proto': {
            'type': 'object'
        },
        'output': {
            'type': ['string', 'null']
        },
        'time-format': {
            'type': 'string',
            'enum': ['iso', 'timestamp']
        },
        'pull': {
            'type': 'array'
        }
    },
    'additionalProperties': False,
    'required': ['version', 'proto', 'pull']
}

output = None


def register_puller(fn, pmap=[]):
    pullers.append((fn, pmap))


def get_last_pull_time():
    return last_pull_time


def clear():
    data.clear()
    pullers.clear()
    processor_maps.clear()
    from .dp import clear
    dp.clear()


def set_data(o, value):
    if value is None:
        return
    current = data.get(o)
    if current != value:
        data[o] = value
        output(o, value)


def _t_processor():
    try:
        while True:
            try:
                data, prc_map = q.get()
            except TypeError:
                break
            for fn in prc_map:
                fn(data)
    except:
        print_trace()


def do(loop=False):

    interval = config['interval']

    beacon_interval = config['beacon']

    t = perf_counter()

    next_iter = t + interval
    next_beacon = t + beacon_interval

    def pull_and_process():
        nonlocal next_iter, next_beacon
        global last_pull_time

        last_pull_time = perf_counter()

        for plr, prc_map in pullers:
            q.put((plr(), prc_map))
        if not processor.is_alive():
            raise RuntimeError('processor thread is gone')
        t = perf_counter()
        if fn_beacon is not None and loop and next_beacon < t:
            fn_beacon()
            while next_beacon < t:
                next_beacon += beacon_interval
            t = perf_counter()
        ts = next_iter - t
        if loop:
            if ts > 0:
                sleep(ts)
            else:
                eprint('WARNING: main loop timeout')
        next_iter += interval

    if loop:
        while True:
            pull_and_process()
    else:
        pull_and_process()


def main():
    global output
    global processor
    global fn_beacon
    ap = argparse.ArgumentParser()
    ap.add_argument('-F',
                    '--config',
                    help='Configuration file',
                    metavar='CONFIG',
                    required=True)
    ap.add_argument('-L',
                    '--loop',
                    help='Loop (production)',
                    action='store_true')
    ap.add_argument('-R',
                    '--auto-restart',
                    help='Auto restart loop on errors',
                    action='store_true')
    a = ap.parse_args()

    with open(a.config) as fh:
        config.update(yaml.safe_load(fh))

    jsonschema.validate(config, CONFIG_SCHEMA)

    config['interval'] = 1 / config['freq']

    set_time_format(config.get('time-format'))

    try:
        om = OUTPUT_METHODS[config['output']]
    except KeyError:
        raise Exception('Unsupported output type')
    output = om['output']

    if config.get('beacon'):
        fn_beacon = om.get('beacon')

    proto = config['proto']['name']

    if '/' in proto:
        proto = proto.split('/', 1)[0]

    lib = importlib.import_module(f'pulr.proto.{proto}')

    while True:
        clear()
        try:
            lib.init(config['proto'],
                     config.get('pull', []),
                     timeout=config['timeout'])

            processor = threading.Thread(target=_t_processor,
                                         name='processor',
                                         daemon=True)
            processor.start()
            try:
                do(loop=a.loop)
            finally:
                # finish data processing before shutting down proto lib
                q.put(None)
                processor.join()
                lib.shutdown()
            if not a.loop:
                break
        except KeyboardInterrupt:
            break
        except:
            if a.auto_restart and a.loop:
                print_trace()
                sleep(config['interval'])
            else:
                raise
