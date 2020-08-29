from pulr import set_data, get_last_pull_time

from functools import partial


def parse_int(i):
    if isinstance(i, int):
        return i
    elif isinstance(i, float):
        return int(i)
    else:
        if '+' in i:
            r = 0
            for x in i.split('+'):
                r += parse_int(x)
            return r
        elif 'x' in i:
            return int(i, 16)
        else:
            return int(i)


# common data types

MAX_INT16 = 32767
MAX_INT32 = 2147483647

MAX_UINT8 = 255
MAX_UINT16 = 65535
MAX_UINT32 = 4294967295
MAX_UINT64 = 18446744073709551615

DATA_TYPE_BIT = 0

DATA_TYPE_INT8 = 1
DATA_TYPE_INT16 = 2
DATA_TYPE_INT32 = 3

DATA_TYPE_UINT8 = 10
DATA_TYPE_UINT16 = 11
DATA_TYPE_UINT32 = 12
DATA_TYPE_UINT64 = 13

DATA_TYPE_REAL32 = 20
DATA_TYPE_REAL64 = 21

MAX_UVAL = {
    DATA_TYPE_UINT8: MAX_UINT8,
    DATA_TYPE_UINT16: MAX_UINT16,
    DATA_TYPE_UINT32: MAX_UINT32,
    DATA_TYPE_UINT64: MAX_UINT64
}

# transformers

_speed_cache = {}


def clear():
    _speed_cache.clear()


def transform_speed(o, interval, tp, value):
    maxval = MAX_UVAL[tp]
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


def transform_int_to_bit(tp, value):
    return value != 0


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
                transforms.append(transform_bit_to_int)
            elif c['type'] == 'int2bit':
                transforms.append(transform_int_to_bit)
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
