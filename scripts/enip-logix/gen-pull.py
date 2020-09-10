#!/usr/bin/env python3

"""
Generates Pulr "pull" config section from JSON, created with fetch-tags.py
"""

import sys
import argparse

try:
    import rapidjson as json
except:
    import json

import yaml

pulls = []

ap = argparse.ArgumentParser()

ap.add_argument('tagfile', metavar='FILE', help='JSON tags file')
ap.add_argument('tag', metavar='TAG', help='Tag to parse')

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


with open(a.tagfile) as fh:
    tags = json.loads(fh.read())

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


data = find_tag(a.tag, tags)
if data['tag_type'] == 'struct':
    pulls.append({
        '1tag': a.tag,
        'process': gen_process(data['data_type']['internal_tags'], 0, a.tag)
    })
else:
    tags_count = 1
    pulls.append({
        '1tag':
            a.tag,
        'process': [{
            'offset': 0,
            'set-id': tag,
            'type': DATA_TYPES[data['data_type']]
        }]
    })

print(
    yaml.dump(dict(pull=pulls),
              default_flow_style=False).replace('\n- 1tag', '\n- tag'))

print(f'{tags_count} tags generated', file=sys.stderr)
