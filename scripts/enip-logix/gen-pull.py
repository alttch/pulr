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

pulls = []

ap = argparse.ArgumentParser()

ap.add_argument('tag', metavar='TAG', help='Tags to parse (comma separated)')

ap.add_argument('-F',
                '--tagfile',
                metavar='FILE',
                help='JSON tags file (default: stdin)')

ap.add_argument('-i',
                '--source',
                metavar='ADDR',
                help='PLC IP[:port] (full config is generated is defined')

ap.add_argument('-f',
                '--freq',
                metavar='HERZ',
                help='Pull frequency',
                default=1,
                type=int)

ap.add_argument('--path', metavar='PATH', help='PLC path', default="1,0")
ap.add_argument('--cpu', metavar='CPU', help='CPU', default="LGX")

ap.add_argument('--timeout',
                metavar='SEC',
                help='PLC TIMEOUT',
                type=float,
                default=2)

a = ap.parse_args()


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
                    return find_tag_in_struct(rest,
                                              t['data_type']['internal_tags'])


if a.tagfile:
    with open(a.tagfile) as fh:
        tags = json.loads(fh.read())
else:
    tags = json.loads(sys.stdin.read())

DATA_TYPES = {
    'DINT': 'uint32',
    'DWORD': 'uint32',
    'REAL': 'real32',
    'BOOL': 'uint8',
    'INT': 'sint32'
}

tags_count = 0


def gen_process(data, offset, tag_name, result=[]):
    global tags_count

    def gen_offset(o1, o2, int_if_possible=False):
        if o1:
            o = f'{o1}+{o2}'
        else:
            o = o2 if int_if_possible else f'{o2}'
        return o

    for tag, d in data.items():
        if d['tag_type'] == 'struct':
            gen_process(d['data_type']['internal_tags'],
                        gen_offset(offset, d['offset']), tag_name + '.' + tag,
                        result)
        else:
            tags_count += 1
            result.append({
                'offset': gen_offset(offset, d['offset'], int_if_possible=True),
                'type': DATA_TYPES[d['data_type']],
                'set-id': tag_name + '.' + tag
            })
    return result


for TAG in a.tag.split(','):
    data = find_tag(TAG, tags)
    if data is None:
        raise ValueError(f'Tag not found: {TAG}')
    if data['tag_type'] == 'struct':
        pulls.append({
            '1tag': TAG,
            'process': gen_process(data['data_type']['internal_tags'], 0, TAG)
        })
    else:
        tags_count += 1
        pulls.append({
            '1tag':
                TAG,
            'process': [{
                'offset': 0,
                'set-id': TAG,
                'type': DATA_TYPES[data['data_type']]
            }]
        })

from collections import OrderedDict

if a.source:
    print(
        dedent(f"""
        version: 2
        timeout: {a.timeout}
        freq: {a.freq}
        proto:
          name: enip/ab_eip
          source: {a.source}
          path: {a.path}
          cpu: {a.cpu}
        """).lstrip())

print(
    yaml.dump(dict(pull=pulls),
              default_flow_style=False).replace('\n- 1tag', '\n- tag'))

print(f'{tags_count} tag(s) generated', file=sys.stderr)
