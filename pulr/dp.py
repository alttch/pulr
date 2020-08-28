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


# common data types

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
    DATA_TYPE_UINT32: MAX_UINT32,
    DATA_TYPE_UINT64: MAX_UINT64
}

# transformers

_speed_cache = {}


def clear():
    _speed_cache.clear()


def transform_speed(o, interval, tp, value):
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


def transform_multiply(m, tp, value):
    return value * m


def transform_divide(d, tp, value):
    return value / d


def transform_round(d, tp, value):
    return round(value, None if d == 0 else d)


def transform_bit_to_int(tp, value):
    return 1 if value else 0


def prepare_transform(o, transform):
    if transform is not None:
        transforms = []
        for c in transform:
            if c['type'] == 'speed':
                transforms.append(
                    partial(transform_speed, o, c.get('interval', 1)))
            elif c['type'] == 'multiply':
                transforms.append(partial(transform_multiply, c['multiplier']))
            elif c['type'] == 'divide':
                transforms.append(partial(transform_divide, c['divisor']))
            elif c['type'] == 'round':
                transforms.append(partial(transform_round, c['digits']))
            elif c['type'] == 'bit2int':
                transforms.append(partial(transform_bit_to_int))
            else:
                raise ValueError(f'Unsupported transform {c["type"]}')
        return transforms
    else:
        return None


def run_transform(transform, tp, value):
    for c in transform:
        value = c(tp, value)
        if value is None:
            return None
    return value


# data conversion functions


def value_to_data(o, offset, transform, data_in):
    value = data_in[offset]
    if transform is not None:
        value = run_transform(transform, None, value)
    set_data(o, value)


def int16_to_data(o, offset, signed, transform, data_in):
    value = data_in[offset]
    if signed and value > MAX_INT16:
        value -= 65536
    if transform is not None:
        value = run_transform(transform,
                              DATA_TYPE_INT16 if signed else DATA_TYPE_UINT16,
                              value)
    set_data(o, value)


def int32_to_data(o, offset, signed, transform, data_in):
    value = data_in[offset] * 65536 + data_in[offset + 1]
    if signed and value > MAX_INT32:
        value -= 4294967296
    if transform is not None:
        value = run_transform(transform,
                              DATA_TYPE_INT32 if signed else DATA_TYPE_UINT32,
                              value)
    set_data(o, value)


def real32_to_data(o, offset, transform, data_in):
    value = struct.unpack(
        'f',
        struct.pack('H', data_in[offset]) +
        struct.pack('H', data_in[offset + 1]))[0]
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_REAL32, value)
    set_data(o, value)


def bit_to_data(o, offset, bit, transform, data_in):
    value = (data_in[offset] >> bit) & 1 == 1
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_BIT, value)
    set_data(o, value)
