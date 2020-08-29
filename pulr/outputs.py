import sys
import time

from neotermcolor import colored, set_style

set_style('time', color='white', attrs='dark')
set_style('id', color='blue', attrs='bold')
set_style('val', color='yellow')

from functools import partial

import pytz

from .beacons import beacon_empty_line

oprint = partial(print, flush=True)
eprint = partial(print, flush=True, file=sys.stderr)

time_format = None

try:
    LOCAL_TZ = pytz.timezone(time.tzname[0])
except:
    eprint('unable to determine local time zone')
    LOCAL_TZ = None

TIME_FORMAT_ISO = 1
TIME_FORMAT_TIMESTAMP = 2

TIME_FORMATS = {'iso': TIME_FORMAT_ISO, 'timestamp': TIME_FORMAT_TIMESTAMP}


def set_time_format(tf):
    global time_format
    if tf is not None:
        time_format = TIME_FORMATS[tf]


try:
    import rapidjson as json
except:
    import json


def get_time():
    if time_format is None:
        return None
    elif time_format == TIME_FORMAT_ISO:
        from datetime import datetime
        return datetime.now().replace(tzinfo=LOCAL_TZ).isoformat()
    elif time_format == TIME_FORMAT_TIMESTAMP:
        return time.time()


def print_trace():
    import traceback
    eprint(traceback.format_exc())


def output_devnull(*args, **kwargs):
    pass


def output_stdout(o, value):
    oprint(f'{colored(str(get_time()), "@time") + " " if time_format else ""}'
           f'{colored(o, "@id")} {colored(value,"@val")}')


def output_stdout_csv(o, value):
    oprint(f'{str(get_time()) + ";" if time_format else ""}{o};{value}')


def output_stdout_ndjson(o, value):
    d = {'id': o, 'value': value}
    if time_format:
        d['time'] = get_time()
    oprint(json.dumps(d))


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
    'text': {
        'output': output_stdout,
        'beacon': beacon_empty_line
    },
    'ndjson': {
        'output': output_stdout_ndjson,
        'beacon': beacon_empty_line
    },
    'csv': {
        'output': output_stdout_csv,
        'beacon': beacon_empty_line
    },
    'eva/datapuller': {
        'output': output_eva_datapuller,
        'beacon': beacon_empty_line
    }
}

OUTPUT_METHODS[None] = OUTPUT_METHODS['text']
