# TODO coils, validate config
from pulr import config, register_puller, get_object_id
from pulr.converters import (parse_int, bit_to_data, int16_to_data,
                             int32_to_data, real32_to_data)

import pymodbus.client.sync
from functools import partial

client = None


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
        for m in p.get('map'):
            offset = m['offset']
            digits = m.get('digits')
            o = get_object_id(m['id'])
            tp = m.get('type')
            if tp and '*' in tp:
                tp, multiplier = tp.split('*')
                tp = tp.strip()
                multiplier = float(multiplier.strip())
            else:
                multiplier = 1
            if reg[0] in ['h', 'i']:
                offset, bit = parse_offset(offset, addr)
                if bit is None:
                    if tp in ['real', 'real32']:
                        fn = partial(real32_to_data, o, offset, multiplier,
                                     digits)
                    elif not tp or tp in ['uint16', 'word']:
                        fn = partial(int16_to_data, o, offset, False,
                                     multiplier, digits)
                    elif tp in ['uint32', 'dword']:
                        fn = partial(int32_to_data, o, offset, False,
                                     multiplier, digits)
                    elif tp in ['sint16', 'int16']:
                        fn = partial(int16_to_data, o, offset, True, multiplier,
                                     digits)
                    elif tp in ['sint32', 'int32']:
                        fn = partial(int32_to_data, o, offset, True, multiplier,
                                     digits)
                    else:
                        raise ValueError(f'type unsupported: {tp}')
                else:
                    fn = partial(bit_to_data, o, offset, bit)
            pmap.append(fn)
        register_puller(
            partial(process_data,
                    partial(pfn, addr, count=p.get('count', 1), unit=u),
                    ('b' if reg[0] in ['c', 'd'] else 'w')), pmap)
    client.connect()

def shutdown():
    client.close()
