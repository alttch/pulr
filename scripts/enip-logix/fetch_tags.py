#!/usr/bin/env python3

"""
Python script to dump tags from AB-Logix PLC to JSON file

Usage:

./fetch-tags.py <IP[PATH]>  > tags.json
"""

import argparse

try:
    import rapidjson as json
except:
    import json

from pycomm3 import LogixDriver

ap = argparse.ArgumentParser()

ap.add_argument("source", metavar="PLC", help="PLC host/ip and path")

a = ap.parse_args()

with LogixDriver(a.source) as plc:
    print(json.dumps(plc.get_tag_list()))
