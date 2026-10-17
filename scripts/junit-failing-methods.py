#!/usr/bin/env python3
"""Print failed/error JUnit testcase names as JSON, including self-closing cases."""

import json
import sys
import xml.etree.ElementTree as ET


failures = []
for report in sys.argv[1:]:
    root = ET.parse(report).getroot()
    for case in root.iter():
        if case.tag.rsplit("}", 1)[-1] != "testcase":
            continue
        if any(child.tag.rsplit("}", 1)[-1] in {"failure", "error"} for child in case):
            failures.append(case.attrib["name"])
print(json.dumps(sorted(failures)))
