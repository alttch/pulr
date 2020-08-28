from pulr import set_data, get_last_pull_time

import struct


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

CALC_SPEED = 0

CALCS = {'speed': CALC_SPEED}

MAX_INT16 = 32767
MAX_INT32 = 2147483647

MAX_UINT16 = 65535
MAX_UINT32 = 4294967295
MAX_UINT64 = 18446744073709551615

DATA_TYPE_INT16 = 0
DATA_TYPE_INT32 = 1

DATA_TYPE_UINT16 = 10
DATA_TYPE_UINT32 = 11
DATA_TYPE_UINT64 = 11

DATA_TYPE_REAL32 = 20

MAX_VAL = {
    DATA_TYPE_UINT16: MAX_UINT16,
    DATA_TYPE_INT32: MAX_UINT32,
    DATA_TYPE_UINT64: MAX_UINT64
}


def get_calc(calc):
    if calc is None:
        return None, None
    try:
        return CALCS[calc['type']], calc
    except KeyError:
        raise ValueError(f'Unsupported calc {calc}')


_speed_cache = {}


def calculate_speed(o, value, maxval, cfg):
    calc_interval = cfg.get('interval', 1)
    if o in _speed_cache:
        v_prev, ptime = _speed_cache[o]
        t_delta = get_last_pull_time() - ptime
        if t_delta < calc_interval:
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


def run_calc(o, value, calc, datatype):
    if calc[0] == CALC_SPEED:
        if datatype in [DATA_TYPE_UINT16, DATA_TYPE_UINT32, DATA_TYPE_UINT64]:
            return calculate_speed(o, value, MAX_VAL[datatype], calc[1])
        else:
            raise RuntimeError(
                'calc speed is not supported with this data type')
    else:
        return value


def value_to_data(o, offset, bool_to_int, calc, data_in):
    value = data_in[offset]
    if calc[0] is not None:
        value = run_calc(o, value, calc, None)
    if isinstance(value, bool) and bool_to_int:
        value = 1 if value is True else 0
    set_data(o, value)


def int16_to_data(o, offset, signed, multiplier, digits, calc, data_in):
    value = data_in[offset]
    if signed and value > MAX_INT16:
        value -= 65536
    if calc[0] is not None:
        value = run_calc(o, value, calc,
                         DATA_TYPE_INT16 if signed else DATA_TYPE_UINT16)
    value *= multiplier
    if digits is not None:
        value = round(value, digits)
    set_data(o, value)


def int32_to_data(o, offset, signed, multiplier, digits, calc, data_in):
    value = data_in[offset] * 65536 + data_in[offset + 1]
    if signed and value > MAX_INT32:
        value -= 4294967296
    if calc[0] is not None:
        value = run_calc(o, value, calc,
                         DATA_TYPE_INT32 if signed else DATA_TYPE_UINT32)
    value *= multiplier
    if digits is not None:
        value = round(value, digits)
    set_data(o, value)


def real32_to_data(o, offset, multiplier, digits, calc, data_in):
    value = struct.unpack(
        'f',
        struct.pack('H', data_in[offset]) +
        struct.pack('H', data_in[offset + 1]))[0] * multiplier
    if calc[0] is not None:
        value = run_calc(o, value, calc, DATA_TYPE_REAL32)
    if digits is not None:
        value = round(value, digits)
    set_data(o, value)


def bit_to_data(o, offset, bit, data_in):
    set_data(o, (data_in[offset] >> bit) & 1)
