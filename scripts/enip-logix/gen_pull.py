#!/usr/bin/env python3
"""
Generates Pulr "pull" config section from JSON, created with fetch-tags.py
"""

import sys
import argparse

from textwrap import dedent

try:
    import rapidjson as json
except:
    import json

import yaml

DEFAULT_FREQ = 1
DEFAULT_PATH = '1,0'
DEFAULT_CPU = 'LGX'
DEFAULT_TIMEOUT = 2


def generate(tag_list,
             tag_file=None,
             tag_data=None,
             config=None,
             id_prefix='',
             id_suffix='',
             print_stats=False,
             print_config=False):

    def find_tag_in_struct(tag, data):
        if '.' in tag:
            tag_to_find, rest = tag.split('.', 1)
        else:
            tag_to_find = tag
            rest = None
        t = data[tag_to_find]
        if rest is None:
            return t
        else:
            if t['tag_type'] != 'struct':
                raise ValueError(f'{tag_to_find} is not a struct!')
            return find_tag_in_struct(
                rest,
                t['data_type']['internal_tags'],
            )

    def find_tag(tag, data):
        if '.' in tag:
            tag_to_find, rest = tag.split('.', 1)
        else:
            tag_to_find = tag
            rest = None
        for t in data:
            if t['tag_name'] == tag_to_find:
                if rest is None:
                    return t
                else:
                    if t['tag_type'] != 'struct':
                        raise ValueError(f'{tag_to_find} is not a struct!')
                    else:
                        return find_tag_in_struct(
                            rest, t['data_type']['internal_tags'])

    if tag_data is None:
        if tag_file:
            with open(tag_file) as fh:
                tags = json.loads(fh.read())
        else:
            tags = json.loads(sys.stdin.read())
    else:
        tags = tag_data

    DATA_TYPES = {
        'BOOL': 'uint8',
        'BYTE': 'byte',
        'WORD': 'word',
        'DWORD': 'dword',
        'LWORD': 'qword',
        'SINT': 'sint8',
        'USINT': 'uint8',
        'INT': 'sint16',
        'UINT': 'uint16',
        'DINT': 'sint32',
        'UDINT': 'uint32',
        'LINT': 'sint64',
        'ULINT': 'uint64',
        'REAL': 'real32',
        'LREAL': 'real64'
    }

    DATA_TYPE_SIZE = {
        'BOOL': 1,
        'BYTE': 1,
        'WORD': 2,
        'DWORD': 4,
        'LWORD': 8,
        'SINT': 1,
        'USINT': 1,
        'INT': 2,
        'UINT': 2,
        'DINT': 4,
        'UDINT': 4,
        'LINT': 8,
        'ULINT': 8,
        'REAL': 4,
        'LREAL': 8
    }

    def gen_offset(o1, o2, int_if_possible=False):
        if o1:
            o = f'{o1}+{o2}'
        else:
            o = o2 if int_if_possible else f'{o2}'
        return o

    def add_tag_info(tag_name, tag_data, coll, offset=0, base_offset=0):

        arr = tag_data.get('array', 0)
        if arr:
            for aofs in range(0, arr):
                coll.append({
                    'offset':
                        gen_offset(base_offset,
                                   offset +
                                   aofs * DATA_TYPE_SIZE[tag_data['data_type']],
                                   int_if_possible=True),
                    'set-id':
                        f'{id_prefix}{tag_name}{id_suffix}[{aofs}]',
                    'type':
                        DATA_TYPES[tag_data['data_type']]
                })
        else:
            coll.append({
                'offset': gen_offset(base_offset, offset, int_if_possible=True),
                'set-id': f'{id_prefix}{tag_name}{id_suffix}',
                'type': DATA_TYPES[tag_data['data_type']]
            })

    tags_count = 0

    pulls = []

    def gen_process(data, offset, tag_name, result=[]):
        nonlocal tags_count

        for tag, d in data.items():
            if d['tag_type'] == 'struct':
                gen_process(d['data_type']['internal_tags'],
                            gen_offset(offset, d['offset']),
                            tag_name + '.' + tag, result)
            else:
                tags_count += 1
                add_tag_info(f'{tag_name}.{tag}',
                             d,
                             result,
                             offset=d['offset'],
                             base_offset=offset)
        return result

    for TAG in tag_list:
        data = find_tag(TAG, tags)
        if data is None:
            raise ValueError(f'Tag not found: {TAG}')
        if data['tag_type'] == 'struct':
            pulls.append({
                '1tag':
                    TAG,
                'process':
                    gen_process(data['data_type']['internal_tags'], 0, TAG, [])
            })
        else:
            tags_count += 1

            result = []
            add_tag_info(TAG, data, result)
            pulls.append({'1tag': TAG, 'process': result})

    CFG = ''

    if config:
        CFG += dedent(f"""
            version: 2
            timeout: {config.get("timeout", DEFAULT_TIMEOUT)}
            freq: {config.get("freq", DEFAULT_FREQ)}
            proto:
              name: enip/ab_eip
              source: {config["source"]}
              path: {config.get("path", DEFAULT_PATH)}
              cpu: {config.get("cpu", DEFAULT_CPU)}
            """).lstrip()

    CFG += yaml.dump(dict(pull=pulls),
                     default_flow_style=False).replace('\n- 1tag', '\n- tag')

    if print_config:
        print(CFG)

    if print_stats:
        print(f'{tags_count} tag(s) generated', file=sys.stderr)

    return CFG


if __name__ == '__main__':
    ap = argparse.ArgumentParser()

    ap.add_argument('tag',
                    metavar='TAG',
                    help='Tags to parse (comma separated)')

    ap.add_argument('-F',
                    '--tag_file',
                    metavar='FILE',
                    help='JSON tags file (default: stdin)')

    ap.add_argument('-s',
                    '--source',
                    metavar='ADDR',
                    help='PLC IP[:port] (full config is generated is defined')

    ap.add_argument('-f',
                    '--freq',
                    metavar='HERZ',
                    help='Pull frequency',
                    default=DEFAULT_FREQ,
                    type=int)

    ap.add_argument('--path',
                    metavar='PATH',
                    help='PLC path',
                    default=DEFAULT_PATH)
    ap.add_argument('--cpu', metavar='CPU', help='CPU', default=DEFAULT_CPU)

    ap.add_argument('--timeout',
                    metavar='SEC',
                    help='PLC TIMEOUT',
                    type=float,
                    default=DEFAULT_TIMEOUT)

    ap.add_argument('--id-prefix',
                    metavar='VALUE',
                    help='ID prefix',
                    default='')

    ap.add_argument('--id-suffix',
                    metavar='VALUE',
                    help='ID suffix',
                    default='')

    a = ap.parse_args()

    if a.source:
        config = dict(source=a.source,
                      freq=a.freq,
                      path=a.path,
                      cpu=a.cpu,
                      timeout=a.timeout)
    else:
        config = None

    generate(tag_file=a.tag_file,
             tag_list=a.tag.split(','),
             config=config,
             id_prefix=a.id_prefix,
             id_suffix=a.id_suffix,
             print_stats=True,
             print_config=True)
