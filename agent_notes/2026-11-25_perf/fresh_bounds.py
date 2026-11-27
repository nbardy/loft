"""Read the original proxy and certified interval of each pinned World-space field."""
import json
from pathlib import Path
import re
import struct
import sys

from fresh import scenes


def interval_records(path):
    payload = path.read_text().rsplit(' (kinds ', 1)[1]
    tables, resources = payload.split(' (instances', 1)
    words = [float(x) for x in re.search(r'\(luts ([^)]+)', resources)[1].split()]
    shape = r'\(\(name \. "([^"]+)"\) \(module \. \d+\) \(stride \. (\d+)\) \(rows ([^)]+)\) \(n_rows \. (\d+)\)'
    found = {}
    for kind, m in enumerate(re.finditer(shape, tables)):
        name, stride, raw, count = m.groups()
        # These five fixtures have no mesh, World BVH or scanned curve. The
        # per-kind offsets precede the interval records and the GGX energy LUT.
        base, = struct.unpack('I', struct.pack('f', words[kind]))
        if not base:
            continue
        n, stride = int(count), int(stride)
        assert base + 10 * n <= len(words)
        data = list(map(int, raw.split()))
        rows = []
        for i in range(n):
            record = words[base + 10 * i:base + 10 * (i + 1)]
            end = (i + 1) * stride
            bbox = struct.unpack('6f', struct.pack('6I', *data[end - 7:end - 1]))
            rows.append(dict(outer=list(bbox), inner=record[:6], center=record[6:9], reach=record[9]))
        found[name.rsplit('_', 1)[-1]] = rows
    return found


if __name__ == '__main__':
    directory = Path(sys.argv[1])
    results = {}
    for scene, fixture in scenes.items():
        path = directory / ('check-' + Path(fixture).name + '.ir')
        if path.exists() and path.stat().st_size:
            results[scene] = interval_records(path)
    print(json.dumps(results, indent=2))
