from pulr import set_data, get_last_pull_time

import struct

from functools import partial


def parse_int(i):
    if isinstance(i, int):
        return i
    elif 'x' in i:
        return int(i, 16)
    else:
        return int(i)


def parse_value(val):
    try:
        value = val.decode()
        try:
            value = int(value)
            if int(value) == float(value):
                value = int(value)
        except:
            pass
    except:
        value = '0x' + ''.join(x[2:].upper() for x in map(hex, val))
    return value


# common data postprocessors

CALC_SPEED = 1
CALC_MULTIPLY = 2
CALC_DIVIDE = 3
CALC_ROUND = 4

MAX_INT16 = 32767
MAX_INT32 = 2147483647

MAX_UINT16 = 65535
MAX_UINT32 = 4294967295
MAX_UINT64 = 18446744073709551615

DATA_TYPE_BIT = 0

DATA_TYPE_INT16 = 1
DATA_TYPE_INT32 = 2

DATA_TYPE_UINT16 = 10
DATA_TYPE_UINT32 = 11
DATA_TYPE_UINT64 = 12

DATA_TYPE_REAL32 = 20

MAX_VAL = {
    DATA_TYPE_UINT16: MAX_UINT16,
    DATA_TYPE_INT32: MAX_UINT32,
    DATA_TYPE_UINT64: MAX_UINT64
}

_speed_cache = {}


def convert_speed(o, tp, interval, value):
    maxval = MAX_VAL[tp]
    if o in _speed_cache:
        v_prev, ptime = _speed_cache[o]
        t_delta = get_last_pull_time() - ptime
        if t_delta < interval:
            return None
        if value >= v_prev:
            v_delta = value - v_prev
        else:
            v_delta = maxval - v_prev + value
        speed = v_delta / t_delta
    else:
        speed = 0
    _speed_cache[o] = (value, get_last_pull_time())
    return speed


def convert_multiply(m, value):
    return value * m


def convert_divide(d, value):
    return value / d


def convert_round(d, value):
    return round(value, d)

def convert_bin_to_int(value):
    return 1 if value is True else 0

def prepare_convert(o, convert, tp):
    if convert is not None:
        converts = []
        for c in convert:
            if c['type'] == 'speed':
                converts.append(
                    partial(convert_speed, o, tp, c.get('interval', 1)))
            elif c['type'] == 'multiply':
                converts.append(partial(convert_multiply, c['multiplier']))
            elif c['type'] == 'divide':
                converts.append(partial(convert_divide, c['divisor']))
            elif c['type'] == 'round':
                converts.append(partial(convert_round, c['digits']))
            elif c['type'] == 'bin2int':
                converts.append(partial(convert_bin_to_int))
            else:
                raise ValueError(f'Unsupported convert {c["type"]}')
        return converts
    else:
        return None


def run_convert(convert, value):
    for c in convert:
        value = c(value)
        if value is None:
            return None
    return value


def value_to_data(o, offset, convert, data_in):
    value = data_in[offset]
    if convert is not None:
        value = run_convert(convert, value)
    set_data(o, value)


def int16_to_data(o, offset, signed, convert, data_in):
    value = data_in[offset]
    if signed and value > MAX_INT16:
        value -= 65536
    if convert is not None:
        value = run_convert(convert, value)
    set_data(o, value)


def int32_to_data(o, offset, signed, convert, data_in):
    value = data_in[offset] * 65536 + data_in[offset + 1]
    if signed and value > MAX_INT32:
        value -= 4294967296
    if convert is not None:
        value = run_convert(convert, value)
    set_data(o, value)


def real32_to_data(o, offset, convert, data_in):
    value = struct.unpack(
        'f',
        struct.pack('H', data_in[offset]) +
        struct.pack('H', data_in[offset + 1]))[0]
    if convert is not None:
        value = run_convert(convert, value)
    set_data(o, value)


def bit_to_data(o, offset, bit, convert, data_in):
    value = (data_in[offset] >> bit) & 1
    if convert is not None:
        value = run_convert(convert, value)
    set_data(o, value)
