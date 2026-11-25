"""Freeze a 960x720 rotated-camera calibration prediction before measuring its original."""
import json
import pathlib
import re
import subprocess
import sys
from datetime import datetime, timezone
from statistics import median

import numpy as np
from count_rays import Raster, U, buffer, group, instrument, raster, row_layout

root = pathlib.Path(sys.argv[1]).parent
folder = root/'paired'
folder.mkdir(exist_ok=True)
scenes = dict(ball='start/ball',base='start/otter',walrus='walrus/tq',heron='heron/h02_hero_q3',otter='otter/hero_q3')


def calculate_counts(name):
    text = (root/'model'/f'{name}.ir').read_text().rsplit(' (kinds ',1)[1]
    modules = json.loads((root/'model'/f'{name}.show').read_text())['modules']
    def vec(key):
        return np.fromstring(re.search(key+r'[^#]*#\(([^)]+)',text)[1],sep=' ')
    at,eye=vec(r'\(camera \(at'),vec(r'\(fixed_eye')
    angle=np.deg2rad(7.)
    rot=np.array([[np.cos(angle),0,np.sin(angle)],[0,1,0],[-np.sin(angle),0,np.cos(angle)]])
    fov=float(re.search(r'\(fov \. ([^)]+)',text)[1])
    params=buffer(np.fromstring(re.search(r'\(params ([^)]+)',text)[1],sep=' ',dtype=np.uint32),U.STORAGE)
    luts=buffer(np.fromstring(re.search(r'\(luts ([^)]+)',text)[1],sep=' ',dtype=np.float32),U.STORAGE)
    view=Raster(at+1.1*rot@(eye-at),at,fov,960,720,params)
    out = {}
    for m in re.finditer(r'\(\(name \. "([^"]+)"\) \(module \. (\d+)\) \(stride \. \d+\) \(rows ([^)]+)\) \(n_rows \. (\d+)\)',text.split(' (instances')[0]):
        kind,module,data,n=m.groups()
        n=int(n)
        probe=raster(instrument(modules[int(module)]['wgsl']),True)
        rows=group(row_layout,[(0,buffer(np.fromstring(data,sep=' ',dtype=np.uint32),U.STORAGE)),
                               (1,buffer(np.arange(n,dtype=np.uint32),U.STORAGE)),
                               (2,buffer(np.array([len(out),0,n,0],np.uint32),U.UNIFORM)),(3,luts)])
        out[kind]=view.draw([(None,probe,rows,n)],True)
    return out


def native(name, phase, tag):
    scene=root/phase/'source/fixtures'/(scenes[name]+'.loft')
    # First quiet run or the first flagged run, as in the main study.
    attempts=[]
    for i in range(2):
        f=folder/f'{name}-{tag}-{i}.json'
        if not f.exists():
            subprocess.run([root/'baseline/loft','profile',scene,'--size','960x720',
                            '--frames','120','--json',f],check=True)
        d=json.loads(f.read_text())
        attempts.append(d)
        if not d['contended']:
            return d
    return attempts[0]


def vis(d):
    return sorted(r[1] for r in d['gpu_pass_ms'])[59]


for name in sys.argv[2:]:
    dest=folder/f'{name}-prediction.json'
    if not dest.exists():
        cf=folder/f'{name}-rotated-counts.json'
        if not cf.exists():
            cf.write_text(json.dumps(calculate_counts(name)))
        counts=json.loads(cf.read_text())
        inputs=json.loads((root/'model'/f'{name}.counts.json').read_text())
        empty=median(json.loads(f.read_text())['cal_empty_ms'] for f in (root/'model').glob('*.counts.json'))
        wtest=wcal=0.
        for kind,r in inputs['kinds'].items():
            def units(c):
                return c[0] if r['draw']=='Analytic' else c[2]+c[3]
            rate=max(0.,median(r['calibration_four_draw_ms'])-empty)/(4*units(r['calibration']))
            wtest+=rate*units(r['target'])
            wcal+=rate*units(counts[kind])
        cal=native(name,'cal','rotated')
        pred=empty+(vis(cal)-empty)*wtest/wcal
        dest.write_text(json.dumps(dict(ms=pred,cal_vis=vis(cal),cal_contended=cal['contended'],
                                        target_weight=wtest,rotated_weight=wcal,
                                        frozen=datetime.now(timezone.utc).isoformat()),indent=2))
        print(name,'frozen',pred,flush=True)
    target=native(name,'baseline','original')
    print(name,'target',vis(target),'contended',target['contended'],flush=True)
