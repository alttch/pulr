# TODO redistribute-compile libplctag in some way

from pulr import config, register_puller, set_data

from pulr.dp import (parse_int, prepare_transform, run_transform, DATA_TYPE_BIT,
                     DATA_TYPE_INT16, DATA_TYPE_INT32, DATA_TYPE_REAL32,
                     DATA_TYPE_UINT16, DATA_TYPE_UINT32, DATA_TYPE_UINT64,
                     DATA_TYPE_INT8, DATA_TYPE_UINT8, DATA_TYPE_REAL64)

from functools import partial
from time import sleep
import ctypes
import platform
import threading

import jsonschema

plc_timeout = 2000
plc_path = ''

system = platform.system()
if system == 'Windows':
    libfile = 'plctag.dll'
elif system == 'Darwin':
    libfile = 'libplctag.dylib'
else:
    libfile = 'libplctag.so'
plc_lib = ctypes.cdll.LoadLibrary(libfile)

plc_tag_create = plc_lib.plc_tag_create
plc_tag_create.restype = ctypes.c_int
plc_tag_create.argtypes = [ctypes.c_char_p, ctypes.c_int]

plc_tag_status = plc_lib.plc_tag_status
plc_tag_status.restype = ctypes.c_int
plc_tag_status.argtypes = [ctypes.c_int]

plc_tag_destroy = plc_lib.plc_tag_destroy
plc_tag_destroy.restype = ctypes.c_int
plc_tag_destroy.argtypes = [ctypes.c_int]

plc_tag_read = plc_lib.plc_tag_read
plc_tag_read.restype = ctypes.c_int
plc_tag_read.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_float32 = plc_lib.plc_tag_get_float32
plc_tag_get_float32.restype = ctypes.c_float
plc_tag_get_float32.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_float64 = plc_lib.plc_tag_get_float64
plc_tag_get_float64.restype = ctypes.c_double
plc_tag_get_float64.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_uint8 = plc_lib.plc_tag_get_uint8
plc_tag_get_uint8.restype = ctypes.c_ubyte
plc_tag_get_uint8.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_int8 = plc_lib.plc_tag_get_int8
plc_tag_get_int8.restype = ctypes.c_byte
plc_tag_get_int8.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_uint16 = plc_lib.plc_tag_get_uint16
plc_tag_get_uint16.restype = ctypes.c_ushort
plc_tag_get_uint16.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_int16 = plc_lib.plc_tag_get_int16
plc_tag_get_int16.restype = ctypes.c_short
plc_tag_get_int16.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_uint32 = plc_lib.plc_tag_get_uint32
plc_tag_get_uint32.restype = ctypes.c_uint
plc_tag_get_uint32.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_int32 = plc_lib.plc_tag_get_int32
plc_tag_get_int32.restype = ctypes.c_int
plc_tag_get_int32.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_uint64 = plc_lib.plc_tag_get_uint64
plc_tag_get_uint64.restype = ctypes.c_uint64
plc_tag_get_uint64.argtypes = [ctypes.c_int, ctypes.c_int]

plc_tag_get_int64 = plc_lib.plc_tag_get_int64
plc_tag_get_int64.restype = ctypes.c_int64
plc_tag_get_int64.argtypes = [ctypes.c_int, ctypes.c_int]

active_tags = {}

SLEEP_STEP = 0.01

SCHEMA_PROTO = {
    'type': 'object',
    'properties': {
        'name': {
            'type': 'string',
            'enum': ['enip/ab_eip']
        },
        'source': {
            'type': 'string'
        },
        'path': {
            'type': 'string'
        },
        'cpu': {
            'type': 'string',
            'enum': ['LGX', 'MLGX', 'PLC', 'MLGX800']
        }
    },
    'additionalProperties': False,
    'required': ['name', 'source', 'cpu']
}

SCHEMA_PULL = {
    'type': 'array',
    'items': {
        'type': 'object',
        'properties': {
            'tag': {
                'type': 'string'
            },
            'size': {
                'type': 'integer',
                'minimal': 1
            },
            'count': {
                'type': 'integer',
                'minimal': 1
            },
            'process': {
                'type': 'array',
                'items': {
                    'type': 'object',
                    'properties': {
                        'offset': {
                            'type': ['integer', 'string'],
                        },
                        'set-id': {
                            'type': 'string',
                        },
                        'type': {
                            'type':
                                'string',
                            'enum': [
                                'real', 'real32', 'uint16', 'word', 'uint32',
                                'dword', 'sint16', 'int16', 'sint32', 'int32',
                                'uint8', 'int8', 'sint8', 'byte', 'int64',
                                'sint64', 'uint64', 'qword', 'real64'
                            ]
                        },
                        'transform': {
                            'type': 'array'
                        }
                    },
                    'additionalProperties': False,
                    'required': ['offset', 'type', 'set-id']
                }
            }
        },
        'additionalProperties': False,
        'required': ['tag', 'process']
    }
}

TAG_STATUS_OK = 0
TAG_STATUS_PENDING = 1


def read_tag(tag_path):
    if tag_path in active_tags:
        tag_id = active_tags[tag_path]
    else:
        tag_id = plc_tag_create(ctypes.c_char_p((plc_path + tag_path).encode()),
                                plc_timeout)
        if tag_id < 0:
            raise RuntimeError(f'{tag_path} error {tag_id}')
        while True:
            rc = plc_tag_status(tag_id)
            if rc == TAG_STATUS_PENDING:
                sleep(SLEEP_STEP)
                continue
            elif rc != TAG_STATUS_OK:
                raise RuntimeError(f'{tag_path} status error {rc}')
            break
        active_tags[tag_path] = tag_id
    rc = plc_tag_read(active_tags[tag_path], plc_timeout)
    if rc != TAG_STATUS_OK:
        raise RuntimeError(tag_path)
    return tag_id


def real32_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_float32(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_REAL32, value)
    set_data(o, value)


def real64_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_float64(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_REAL64, value)
    set_data(o, value)


def uint8_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_uint8(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_UINT8, value)
    set_data(o, value)


def int8_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_int8(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_INT8, value)
    set_data(o, value)


def uint16_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_uint16(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_UINT16, value)
    set_data(o, value)


def int16_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_int16(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_INT16, value)
    set_data(o, value)


def uint32_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_uint32(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_UINT32, value)
    set_data(o, value)


def int32_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_int32(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_INT32, value)
    set_data(o, value)


def uint64_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_uint64(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_UINT64, value)
    set_data(o, value)


def int64_to_data(o, offset, transform, tag_id):
    value = plc_tag_get_int64(tag_id, offset)
    if value is None:
        raise ValueError
    if transform is not None:
        value = run_transform(transform, DATA_TYPE_INT64, value)
    set_data(o, value)


def init(cfg_proto, cfg_pull, timeout=5):
    global plc_timeout, plc_path

    jsonschema.validate(cfg_proto, SCHEMA_PROTO)
    jsonschema.validate(cfg_pull, SCHEMA_PULL)
    if cfg_proto['name'] in ['enip/ab_eip']:
        try:
            host, port = cfg_proto['source'].rsplit(':', 1)
        except:
            host = cfg_proto['source']
            port = 44818
    else:
        raise ValueError(f'Unsupported protocol: {cfg_proto["name"]}')

    path = cfg_proto.get('path', '')

    cpu = cfg_proto['cpu']

    plc_timeout = timeout * 1000

    plc_path = f'protocol=ab_eip&gateway={host}:{port}&path={path}&cpu={cpu}'

    for p in cfg_pull:
        tag = p['tag']
        size = p.get('size', 1)
        count = p.get('count')
        tag_path = f'&elem_size={size}'
        if count:
            tag_path += f'&elem_count={count}'
        tag_path += f'&name={tag}'
        pmap = []
        for m in p.get('process', []):
            offset = m['offset']
            o = m['set-id']
            tp = m.get('type')
            transform = m.get('transform')
            offset = parse_int(offset)
            if tp in ['real', 'real32']:
                fn = partial(real32_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['real64']:
                fn = partial(real64_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['uint8', 'byte']:
                fn = partial(uint8_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['uint16', 'word']:
                fn = partial(uint16_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['uint32', 'dword']:
                fn = partial(uint32_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['uint64', 'qword']:
                fn = partial(uint64_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['sint8', 'int8']:
                fn = partial(int8_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['sint16', 'int16']:
                fn = partial(int16_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['sint32', 'int32']:
                fn = partial(int32_to_data, o, offset,
                             prepare_transform(o, transform))
            elif tp in ['sint64', 'int64']:
                fn = partial(int64_to_data, o, offset,
                             prepare_transform(o, transform))
            else:
                raise ValueError(f'type unsupported: {tp}')
            pmap.append(fn)
        register_puller(partial(read_tag, tag_path), pmap)


def shutdown():
    for p, t in active_tags.items():
        plc_tag_destroy(t)
    active_tags.clear()
