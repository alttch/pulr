# TODO: counters
from pulr import set_data

import struct


def parse_int(i):
    if isinstance(i, int):
        return i
    elif 'x' in i:
        return int(i, 16)
    else:
        return int(i)


# common data postprocessors


def value_to_data(o, offset, bool_to_int, data_in):
    value = data_in[offset]
    if isinstance(value, bool) and bool_to_int:
        value = 1 if value is True else 0
    set_data(o, value)


def int16_to_data(o, offset, signed, multiplier, digits, data_in):
    value = data_in[offset]
    if signed and value > 32767:
        value -= 65536
    value *= multiplier
    if digits is not None:
        value = round(value, digits)
    set_data(o, value)


def int32_to_data(o, offset, signed, multiplier, digits, data_in):
    value = data_in[offset] * 65536 + data_in[offset + 1]
    if signed and value > 2147483647:
        value -= 4294967296
    value *= multiplier
    if digits is not None:
        value = round(value, digits)
    set_data(o, value)


def real32_to_data(o, offset, multiplier, digits, data_in):
    value = struct.unpack(
        'f',
        struct.pack('H', data_in[offset]) +
        struct.pack('H', data_in[offset + 1]))[0] * multiplier
    if digits is not None:
        value = round(value, digits)
    set_data(o, value)


def bit_to_data(o, offset, bit, data_in):
    x = data_in[offset]
    set_data(o, (x >> bit) & 1)
