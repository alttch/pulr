from pulr import config, register_puller, get_object_id, set_data
from pulr.converters import value_to_data, parse_value

import netsnmp

from functools import partial

import jsonschema

session = None

SCHEMA_PROTO = {
    'type': 'object',
    'properties': {
        'name': {
            'type': 'string',
            'enum': ['snmp']
        },
        'source': {
            'type': 'string'
        },
        'community': {
            'type': ['integer', 'string']
        },
        'version': {
            'type': 'integer',
            'minimal': 1
        },
        'retries': {
            'type': 'integer',
            'minimal': 0
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
            'oids': {
                'type': 'array',
                'items': {
                    'type': 'string'
                }
            },
            'map': {
                'type': 'array',
                'items': {
                    'type': 'object',
                    'properties': {
                        'offset': {
                            'type': ['integer', 'string'],
                        },
                        'oid': {
                            'type': 'string',
                        },
                        'id': {
                            'type': 'string',
                        },
                        'digits': {
                            'type': 'integer',
                            'minimum': 0
                        },
                        'multiplier': {
                            'type': 'number',
                            'minimum': 0
                        },
                        'divisor': {
                            'type': 'number',
                            'minimum': 0
                        }
                    },
                    'additionalProperties': False,
                    'required': ['oid']
                }
            },
            'ignore': {
                'type': 'array',
                'items': {
                    'type': 'string'
                }
            }
        },
        'additionalProperties': False,
        'required': ['oids']
    }
}


def process_varlist(oid_map, ignore_list, data_in):
    for v in data_in:
        if v.iid != '' and v.iid is not None:
            oid = f'{v.tag}.{v.iid}'
        else:
            oid = v.tag
        if oid in ignore_list:
            continue
        value = parse_value(v.val)
        m = oid_map.get(oid)
        if m:
            if m[0]:
                oid = m[0]
            if m[2] is not None:
                value = float(value) * m[2]
            if m[1] is not None:
                value = round(float(value), m[1])
        set_data(oid, value)


def snmp_get(walk_oids, get_oids):
    result = []
    if walk_oids:
        varlist = netsnmp.VarList(*walk_oids)
        data = session.walk(varlist)
        result += varlist
    if get_oids:
        if len(get_oids) > 1 and session.Version == 2:
            varlist = netsnmp.VarList(*get_oids)
            data = session.getbulk(0, 1, varlist)
            result += varlist
        else:
            for g in get_oids:
                varlist = netsnmp.VarList(g)
                data = session.get(varlist)
                result += varlist
    return result


def init(cfg_proto, cfg_pull, timeout=5):
    global session
    jsonschema.validate(cfg_proto, SCHEMA_PROTO)
    jsonschema.validate(cfg_pull, SCHEMA_PULL)

    try:
        host, port = cfg_proto['source'].rsplit(':', 1)
    except:
        host = cfg_proto['source']
        port = 161

    def get_multiplier(m):
        multiplier = m.get('multiplier')
        if 'divisor' in m:
            if multiplier is not None:
                raise ValueError('both divisor and multiplier specified')
            multiplier = 1 / float(m['divisor'])
        return multiplier

    session = netsnmp.Session(Version=cfg_proto.get('version', 2),
                              DestHost=host,
                              RemotePort=port,
                              Community=cfg_proto.get('community', 'public'),
                              Timeout=int(timeout * 1000000),
                              Retries=cfg_proto.get('retries', 1))
    for p in cfg_pull:
        pfn = partial(snmp_get, [v[:-2] for v in p['oids'] if v.endswith('.*')],
                      [v for v in p['oids'] if not v.endswith('.*')])
        pmap = [
            partial(
                process_varlist, {
                    v['oid']: (v.get('id'), v.get('digits'), get_multiplier(v))
                    for v in p.get('map', [])
                }, p.get('ignore', []))
        ]
        register_puller(pfn, pmap)


def shutdown():
    pass
