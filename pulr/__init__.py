# TODO tests

__author__ = 'Altertech, https://www.altertech.com/'
__copyright__ = 'Copyright (C) 2020 Altertech'
__license__ = 'Apache License 2.0'

__version__ = '0.0.5'

import sys
import argparse
import importlib
import threading
import jsonschema

import yaml

from time import perf_counter, sleep
from .outputs import OUTPUT_METHODS, oprint
from queue import Queue

q = Queue()

processor = None

data = {}

pullers = []
processor_maps = []

DEFAULT_TIMEOUT = 5
DEFAULT_FREQUENCY = 1
DEFAULT_BEACON_FREQUENCY = 0

last_pull_time = 0

config = {
    'timeout': DEFAULT_TIMEOUT,
    'freq': DEFAULT_FREQUENCY,
    'beacon': DEFAULT_BEACON_FREQUENCY,
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


def _t_beacon(fn, interval):
    try:
        next_beacon = perf_counter() + interval
        while True:
            ts = next_beacon - perf_counter()
            if ts > 0:
                sleep(ts)
            fn()
            next_beacon += interval
    except:
        import traceback
        oprint('beacon error', file=sys.stderr)
        oprint(traceback.format_exc())


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
        import traceback
        oprint(traceback.format_exc(), file=sys.stderr)


def do(loop=False):

    interval = config['interval']

    next_iter = perf_counter() + interval

    def pull_and_process():
        nonlocal next_iter
        global last_pull_time

        last_pull_time = perf_counter()

        for plr, prc_map in pullers:
            q.put((plr(), prc_map))
        if not processor.is_alive():
            raise RuntimeError('processor thread is gone')
        ts = next_iter - perf_counter()
        if loop:
            if ts > 0:
                sleep(ts)
            else:
                oprint('WARNING: main loop timeout', file=sys.stderr)
        next_iter += interval

    if loop:
        while True:
            pull_and_process()
    else:
        pull_and_process()


def main():
    global output
    global processor
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
                    '--restart',
                    help='Auto restart loop on errors',
                    action='store_true')
    a = ap.parse_args()

    with open(a.config) as fh:
        config.update(yaml.safe_load(fh))

    jsonschema.validate(config, CONFIG_SCHEMA)

    config['interval'] = 1 / config['freq']

    try:
        om = OUTPUT_METHODS[config['output']]
    except KeyError:
        raise Exception('Unsupported output type or output type not specified')
    output = om['output']
    send_beacon = om.get('beacon')

    proto = config['proto']['name']

    if '/' in proto:
        proto = proto.split('/', 1)[0]

    lib = importlib.import_module(f'pulr.proto.{proto}')

    if a.loop and send_beacon and config['beacon']:
        threading.Thread(target=_t_beacon,
                         name='beacon',
                         args=(send_beacon, config['beacon']),
                         daemon=True).start()

    while True:
        clear()
        if processor is None or not processor.is_alive():
            processor = threading.Thread(target=_t_processor, name='processor')
        try:
            processor.start()
            try:
                lib.init(config['proto'],
                         config.get('pull', []),
                         timeout=config['timeout'])

                try:
                    do(loop=a.loop)
                finally:
                    lib.shutdown()
                if not a.loop:
                    break
            except KeyboardInterrupt:
                break
            except:
                if a.restart and a.loop:
                    import traceback
                    oprint(traceback.format_exc(), file=sys.stderr)
                    sleep(config['interval'])
                else:
                    raise
        finally:
            q.put(None)
