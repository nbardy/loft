"""Compare exact hit IDs and f32 distances at fixed draw order, including hardware scissors."""
# /// script
# dependencies = ["numpy", "wgpu==0.33.0"]
# ///
import json
import math
import re
import numpy as np

from count_rays import P, U, Raster, buffer, group, instrument, q, raster, row_layout
from fresh import checkpoint, out
from thin_coverage import bounds, box_faces

width, height = 960, 720
size = np.array([width, height])
unit = lambda x: x / np.linalg.norm(x)
def scissor_ranges(a, b, ra, rb, project):
    al, ah = box_faces(a-ra, a+ra, project)
    bl, bh = box_faces(b-rb, b+rb, project)
    lo = np.clip(np.floor(np.minimum(al,bl))-2,0,size).astype(int)
    hi = np.clip(np.ceil(np.maximum(ah,bh))+2,0,size).astype(int)
    result = []
    for i in range(0,len(a),8):
        low, high = lo[i:i+8].min(axis=0), hi[i:i+8].max(axis=0)
        result.append((i, min(8,len(a)-i), tuple(map(int, (*low, *(high-low))))))
    return result

for name in ["base", "walrus", "otter"]:
    text = (P / f"{name}.ir").read_text().rsplit(' (kinds ',1)[1]
    modules = {m["name"]:m["wgsl"] for m in json.loads((P/f"{name}.show").read_text())["modules"]}
    kinds, at, eye0, fov = bounds(name)
    params = buffer(np.fromstring(re.search(r'\(params ([^)]+)',text)[1],sep=' ',dtype=np.uint32), U.STORAGE)
    luts = buffer(np.zeros(1,np.float32), U.STORAGE)
    for kind, a,b,ra,rb,_,_ in kinds:
        label = next(n for n in modules if n.split("_")[-1] == kind)
        data = re.search(re.escape(f'(name . "{label}")') + r' \(module \. \d+\) \(stride \. \d+\) \(rows ([^)]+)', text)[1]
        rows = group(row_layout,[(0,buffer(np.fromstring(data,sep=' ',dtype=np.uint32),U.STORAGE)),
            (1,buffer(np.arange(len(a),dtype=np.uint32),U.STORAGE)),(2,buffer(np.array([0,0,len(a),0],np.uint32),U.UNIFORM)),(3,luts)])
        def setup(code):
            body = code[:code.index('\n}',code.index('@fragment fn fs('))+2]
            return [(raster(body,False),raster(instrument(code),True),rows,len(a))]
        cube = setup(modules[label])
        strip = setup((out/"thin"/f"{name}-{kind}.wgsl").read_text())
        records=[]
        for i in (0,60,119):
            phi=i*2*np.pi/1440
            radius=np.linalg.norm((eye0-at)[[0,2]])
            eye=at+[radius*np.sin(phi),eye0[1]-at[1],radius*np.cos(phi)]
            forward=unit(at-eye)
            right=unit(np.cross(forward,[0,1,0]))
            up=np.cross(right,forward)
            def project(points):
                rel=points-eye
                assert (rel@forward > 0.05).all()
                return size/2+np.column_stack([rel@right,-rel@up])/(rel@forward)[:,None]*height/(2*math.tan(fov/2))
            rects=scissor_ranges(a,b,ra,rb,project)
            view=Raster(eye,at,fov,width,height,params)
            for variant,spec,clip in [("original",cube,None),("strip",strip,None),("scissor8",cube,rects)]:
                ms=[view.draw(spec,timing=True,rects=clip) for _ in range(9)][2:]
                raw=q.read_texture(dict(texture=view.color),dict(bytes_per_row=width*8,rows_per_image=height),(width,height,1))
                vis=np.frombuffer(raw,np.uint32).copy().reshape(-1,2)
                if variant == "original": original=vis
                launched,hits,*_=view.draw(spec,count=True,rects=clip)
                record=dict(frame=i,variant=variant,fragments=launched,hits=hits,gpu_ms=ms,
                            row_diff=int((original[:,0]!=vis[:,0]).sum()),
                            vis_diff=int(np.any(original!=vis,axis=1).sum()))
                print(name,kind,record,flush=True)
                records.append(record)
        checkpoint("thin",f"{name}-{kind}-vis-replay.json",json.dumps(records).encode(),
                   f"Measure {kind} fragment launches and exact words with fixed order")
