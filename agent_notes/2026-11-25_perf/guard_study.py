"""Generate a scene-local fresh-ray ablation. This obsoletes no production bounds.
The inner boxes are hypotheses for these fixed poses; the source proxy, sun camera,
original march entry, step, threshold and cap are retained. Validate lossless pictures.
"""
import re
import struct
import sys
from pathlib import Path

root = Path(sys.argv[1])
settings = {
    'base': ('start/otter', {'Otter':([.11,-.18,-.20],[.75,.25,.20])}),
    'walrus': ('walrus/walrus_l1', {'Hide':([-.44,-.12,-.52],[.45,.75,.67]),
                                    'Cap':([-.35,.22,-.20],[.37,.8,.46])}),
    'heron': ('heron/heron', {'HeronHead':([-.18,.39,-.21],[.24,.78,.21])}),
    'otter': ('otter/forms', {'Head4':('c.xyz - rgb(0.16)','c.xyz + rgb(0.16)')}),
}

phase = 'bounds'
if sys.argv[2:] == ['torso']:
    phase = 'bounds2'
    joints = ['pelvis', 'chest', 'foot_l', 'foot_r', 'knee_l', 'knee_r',
              'elbow_l', 'elbow_r', 'hand_l', 'hand_r']
    def extent(op):
        value = joints[0]+'.xyz'
        for joint in joints[1:]:
            value = f'{op}({value}, {joint}.xyz)'
        return value
    settings = {'otter': ('otter/otter', {'Torso': (extent('min')+' - rgb(0.13)',
                                                  extent('max')+' + rgb(0.16)')})}


def literal(val):
    if isinstance(val,str): return val
    return '['+', '.join(format(v,'.12f') for v in val)+']'


for name,(module,targets) in settings.items():
    orig=root/'baseline/source/fixtures'/(module+'.loft')
    source=orig.read_text()
    ir=(root/'model'/f'{name}.ir').read_text().rsplit(' (kinds ',1)[1].split(' (instances')[0]
    for kind,(lo,hi) in targets.items():
        row = next(data for label,data in re.findall(r'\(\(name \. "([^"]+)"\) \(module \. \d+\) \(stride \. \d+\) \(rows ([^)]+)\) \(n_rows \. 1\)',ir)
                   if label.split('_')[-1]==kind)
        bounds=struct.unpack('7f',struct.pack('7I',*map(int,row.split()[-7:])))
        where=re.search(r'\bkind '+kind+r'\b[^{}]*\{\s*trace\(p\)\s*',source)
        start=where.end()
        if source[start]=='=':
            body=source[start+1:source.index('\n',start)].strip().splitlines()
        else:
            assert source[start]=='{'
            end=source.index('\n  }',start)
            body=source[start+1:end].strip().splitlines()
        body[-1]='    let perf_d = '+body[-1].strip()
        query=f'''visible(perf_ro, perf_rd) {{
    let perf_inv = [1.0 / perf_rd.x, 1.0 / perf_rd.y, 1.0 / perf_rd.z]
    let perf_a = ({literal(bounds[:3])} - perf_ro.xyz) * perf_inv
    let perf_b = ({literal(bounds[3:6])} - perf_ro.xyz) * perf_inv
    let perf_near = min(perf_a, perf_b)
    let perf_far = max(perf_a, perf_b)
    var perf_end = min(min(perf_far.x, perf_far.y), perf_far.z)
    var perf_t = max(0.0, max(max(perf_near.x, perf_near.y), perf_near.z))
    # The remote orthographic shadow origin always uses its original interval.
    if length(perf_ro.xyz) < 3.0 {{
      let lower = ({literal(lo)} - perf_ro.xyz) * perf_inv
      let upper = ({literal(hi)} - perf_ro.xyz) * perf_inv
      let enter = min(lower, upper)
      let leave = max(lower, upper)
      perf_end = min(perf_end, min(min(leave.x, leave.y), leave.z))
      if max(perf_t, max(max(enter.x, enter.y), enter.z)) > perf_end {{ return -1.0 }}
    }}
    for perf_i in 0..192 {{
      let p = perf_ro + perf_rd * perf_t
      {chr(10).join(body)}
      if perf_d < 0.0001 + 0.0002 * perf_t {{ return perf_t }}
      perf_t = perf_t + perf_d * (1.0 / {bounds[-1]:.12f})
      if perf_t > perf_end {{ return -1.0 }}
    }}
    -1.0
  }}
  '''
        at=source.index('trace(p)',where.start())
        source=source[:at]+query+source[at:]
    dest=root/phase/'source/fixtures'/(module+'.loft')
    dest.write_text(source)
