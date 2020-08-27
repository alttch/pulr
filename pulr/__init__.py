__author__ = 'Altertech, https://www.altertech.com/'
__copyright__ = 'Copyright (C) 2020 Altertech'
__license__ = 'Apache License 2.0'

__version__ = '0.0.1'

import sys
import argparse
import importlib
import threading
import jsonschema

import yaml

from time import perf_counter, sleep
from .outputs import OUTPUT_METHODS, output_params, oprint

data = {}

pullers = []
processor_maps = []

DEFAULT_TIMEOUT = 5
DEFAULT_FREQUENCY = 1
DEFAULT_BEACON_FREQUENCY = 2

config = {
    'timeout': DEFAULT_TIMEOUT,
    'freq': DEFAULT_FREQUENCY,
    'beacon': DEFAULT_BEACON_FREQUENCY,
    'output': {
        'type': 'stdout'
    }
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
            'type': 'object'
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


def get_object_id(i):
    if config['output']['type'] == 'stdout/eva-datapuller':
        o = i.rsplit('.', 1)
        if o[1] == 'status':
            o[1] = 's'
        elif o[1] == 'value':
            o[1] = 'v'
        o = tuple(o)
    else:
        o = i
    return o


def set_data(o, value):
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


def do(loop=False):

    interval = config['interval']

    next_iter = perf_counter() + interval

    def pull_and_process():
        nonlocal next_iter
        for plr, prc_map in pullers:
            data = plr()
            for fn in prc_map:
                fn(data)
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
    try:
        global output
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
        a = ap.parse_args()

        with open(a.config) as fh:
            config.update(yaml.safe_load(fh))

        jsonschema.validate(config, CONFIG_SCHEMA)

        config['interval'] = 1 / config['freq']

        try:
            om = OUTPUT_METHODS[config['output']['type']]
        except KeyError:
            raise Exception(
                'Unsupported output type or output type not specified')
        jsonschema.validate(config['output'], om['config_schema'])
        output = om['output']
        output_params.update(config['output'])
        send_beacon = om.get('beacon')

        if 'timeout' not in output_params:
            output_params['timeout'] = config['timeout']

        proto = config['proto']['name']

        if '/' in proto:
            proto = proto.split('/', 1)[0]

        lib = importlib.import_module(f'pulr.proto.{proto}')

        lib.init(config['proto'],
                 config.get('pull', []),
                 timeout=config['timeout'])

        if a.loop and send_beacon:
            threading.Thread(target=_t_beacon,
                             name='beacon',
                             args=(send_beacon, config['beacon']),
                             daemon=True).start()

        try:
            do(loop=a.loop)
        finally:
            lib.shutdown()
    except KeyboardInterrupt:
        pass
