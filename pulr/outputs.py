import sys
from functools import partial
from types import SimpleNamespace
from .beacons import beacon_empty_line

oprint = partial(print, flush=True)
eprint = partial(print, flush=True, file=sys.stderr)

_d = SimpleNamespace()

try:
    import rapidjson as json
except:
    import json


def print_trace():
    import traceback
    eprint(traceback.format_exc())


def output_devnull(*args, **kwargs):
    pass


def output_stdout(o, value):
    oprint(f'{o} {value}')


def output_stdout_ndjson(o, value):
    oprint(json.dumps({'id': o, 'v': value}))


def output_eva_datapuller(o, value):
    if o.endswith('.value'):
        s = f'{o[:-6]} u '
        val_mode = True
    elif o.endswith('.status'):
        s = f'{o[:-7]} u '
        val_mode = False
    else:
        s = f'{o} u '
        val_mode = True
    oprint(s + (f'None {value}' if val_mode else str(value)))


OUTPUT_METHODS = {
    None: {
        'output': output_stdout,
        'beacon': beacon_empty_line
    },
    'ndjson': {
        'output': output_stdout_ndjson,
        'beacon': beacon_empty_line
    },
    'eva/datapuller': {
        'output': output_eva_datapuller,
        'beacon': beacon_empty_line
    }
}
