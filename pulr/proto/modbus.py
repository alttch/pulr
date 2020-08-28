from pulr import config, register_puller, set_data

from pulr.dp import (parse_int, prepare_transform, run_transform, DATA_TYPE_BIT,
                     DATA_TYPE_INT16, DATA_TYPE_INT32, DATA_TYPE_REAL32,
                     DATA_TYPE_UINT16, DATA_TYPE_UINT32, DATA_TYPE_UINT64,
                     MAX_INT16, MAX_INT32)

from functools import partial
import struct

import pymodbus.client.sync
import jsonschema

client = None

SCHEMA_PROTO = {
    'type': 'object',
    'properties': {
        'name': {
            'type': 'string',
            'enum': ['modbus/tcp', 'modbus/udp']
        },
        'source': {
            'type': 'string'
        },
        'default-unit': {
            'type': ['integer', 'string']
        }
    },
    'additionalProperties': False,
    'required': ['name', 'source']
}

SCHEMA_PULL = {
    'type': 'array',
    'items': {
        'type': 'object',
        'properties': {
            'reg': {
                'type': 'string'
            },
            'count': {
                'type': 'integer',
                'minimum': 1
            },
            'unit': {
                'type': ['integer', 'string']
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
                                'dword', 'sint16', 'int16', 'sint32', 'int32'
                            ]
                        },
                        'transform': {
                            'type': 'array'
                        }
                    },
                    'additionalProperties': False,
                    'required': ['offset', 'set-id']
                }
            }
        },
        'additionalProperties': False,
        'required': ['reg', 'count', 'process']
    }
}

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


def parse_offset(offset, addr):
    if isinstance(offset, int):
        bit = None
    else:
        if '/' in offset:
            offset, bit = offset.split('/')
            bit = int(bit)
        else:
            bit = None
        if offset.startswith('='):
            absolute = True
            offset = offset[1:]
        else:
            absolute = False
        offset = parse_int(offset)
        if absolute:
            offset -= addr
    if offset < 0:
        raise ValueError('offset is negative')
    if bit is not None and bit < 0:
        raise ValueError('bit is negative')
    return offset, bit


def process_data(fn, dtype):
    rr = fn()
    if rr.isError():
        raise Exception(f'Modbus error {fn}')
    return rr.bits if dtype == 'b' else rr.registers


def init(cfg_proto, cfg_pull, timeout=5):
    global client
    jsonschema.validate(cfg_proto, SCHEMA_PROTO)
    jsonschema.validate(cfg_pull, SCHEMA_PULL)
    if cfg_proto['name'] in ['modbus/tcp', 'modbus/udp']:
        try:
            host, port = cfg_proto['source'].rsplit(':', 1)
        except:
            host = cfg_proto['source']
            port = 502
        if cfg_proto['name'] == 'modbus/tcp':
            client = pymodbus.client.sync.ModbusTcpClient(host, int(port))
        else:
            client = pymodbus.client.sync.ModbusUdpClient(host, int(port))
    else:
        raise ValueError(f'Unsupported protocol: {cfg_proto["name"]}')

    client.timeout = timeout

    for p in cfg_pull:
        try:
            unit = p['unit']
        except KeyError:
            unit = cfg_proto.get('default-unit', 1)
        u = parse_int(unit)
        reg = p['reg']
        addr = parse_int(reg[1:])
        if reg[0] == 'c':
            pfn = client.read_coils
        elif reg[0] == 'd':
            pfn = client.read_discrete_inputs
        elif reg[0] == 'h':
            pfn = client.read_holding_registers
        elif reg[0] == 'i':
            pfn = client.read_input_registers
        else:
            raise ValueError(f'Invalid register type: {reg[0]}')
        pmap = []
        for m in p.get('process', []):
            offset = m['offset']
            o = m['set-id']
            tp = m.get('type')
            transform = m.get('transform')
            if reg[0] in ['h', 'i']:
                offset, bit = parse_offset(offset, addr)
                if bit is None:
                    if tp in ['real', 'real32']:
                        fn = partial(real32_to_data, o, offset,
                                     prepare_transform(o, transform))
                    elif not tp or tp in ['uint16', 'word']:
                        fn = partial(int16_to_data, o, offset, False,
                                     prepare_transform(o, transform))
                    elif tp in ['uint32', 'dword']:
                        fn = partial(int32_to_data, o, offset, False,
                                     prepare_transform(o, transform))
                    elif tp in ['sint16', 'int16']:
                        fn = partial(int16_to_data, o, offset, True,
                                     prepare_transform(o, transform))
                    elif tp in ['sint32', 'int32']:
                        fn = partial(int32_to_data, o, offset, True,
                                     prepare_transform(o, transform))
                    else:
                        raise ValueError(f'type unsupported: {tp}')
                else:
                    fn = partial(bit_to_data, o, offset, bit,
                                 prepare_transform(o, transform))
            else:
                fn = partial(value_to_data, o, offset,
                             prepare_transform(o, transform))
            pmap.append(fn)
        register_puller(
            partial(process_data,
                    partial(pfn, addr, count=p.get('count', 1), unit=u),
                    ('b' if reg[0] in ['c', 'd'] else 'w')), pmap)
    client.connect()


def shutdown():
    client.close()
