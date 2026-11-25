"""Replay the walrus with opposite within-kind row orders at 1280×800."""
import json
import re
import sys
from pathlib import Path

import numpy as np
from count_rays import Raster, U, buffer, group, q, raster, row_layout

model = Path(sys.argv[1])
text = (model/'walrus.ir').read_text().rsplit(' (kinds ',1)[1]
modules = json.loads((model/'walrus.show').read_text())['modules']


def vec(key):
    return np.fromstring(re.search(key+r'[^#]*#\(([^)]+)',text)[1], sep=' ')


at, eye = vec(r'\(camera \(at'), vec(r'\(fixed_eye')
fov = float(re.search(r'\(fov \. ([^)]+)',text)[1])
params = buffer(np.fromstring(re.search(r'\(params ([^)]+)',text)[1],sep=' ',dtype=np.uint32),U.STORAGE)
luts = buffer(np.fromstring(re.search(r'\(luts ([^)]+)',text)[1],sep=' ',dtype=np.float32),U.STORAGE)
view = Raster(eye,at,fov,1280,800,params)
forward, reverse, names = [], [], []
for m in re.finditer(r'\(\(name \. "([^"]+)"\) \(module \. (\d+)\) \(stride \. \d+\) \(rows ([^)]+)\) \(n_rows \. (\d+)\)',text.split(' (instances')[0]):
    name, module, data, n = m.groups()
    src = modules[int(module)]['wgsl']
    src = src[:src.index('\n}',src.index('@fragment fn fs('))+2]
    pipe = raster(src,False)
    n = int(n)
    rows = buffer(np.fromstring(data,sep=' ',dtype=np.uint32),U.STORAGE)
    kp = buffer(np.array([len(names),0,n,0],np.uint32),U.UNIFORM)
    names.append(name.split('_')[-1])
    for draws,ids in ((forward,np.arange(n,dtype=np.uint32)),(reverse,np.arange(n,dtype=np.uint32)[::-1].copy())):
        args = group(row_layout,[(0,rows),(1,buffer(ids,U.STORAGE)),(2,kp),(3,luts)])
        draws.append((pipe,pipe,args,n))
images = []
for draws in (forward,reverse):
    view.draw(draws)
    raw = q.read_texture(dict(texture=view.color),dict(bytes_per_row=1280*8,rows_per_image=800),(1280,800,1))
    images.append(np.frombuffer(raw,np.uint32).reshape(800,1280,2).copy())
yy,xx = np.where(np.any(images[0] != images[1],axis=2))
report = {'changed_vis_pixels':int(xx.size),'examples':[]}
for x,y in [(700,407)]+list(zip(xx[:15].tolist(),yy[:15].tolist())):
    choices=[]
    for img in images:
        key,distance=map(int,img[y,x])
        choices.append(dict(kind=names[key>>24] if key>>24<len(names) else 'empty',
                            row=key&0xffffff, distance_bits=distance))
    report['examples'].append(dict(x=x,y=y,choices=choices))
print(json.dumps(report,indent=2))
