"""Count the guarded fields on the combined run's actual native p95-ranked camera."""
# /// script
# dependencies = ["numpy", "wgpu==0.33.0"]
# ///
import gzip
import json
import math
import re
import sys

import numpy as np
from count_rays import U, Raster, buffer, group, instrument, raster, row_layout
from fresh import checkpoint, notes, out, scenes

phase = sys.argv[2] if len(sys.argv) > 2 else "scissor"
only_size = sys.argv[3] if len(sys.argv) > 3 else None
assert phase in ("scissor", "tiles")
output = "residual-tiles" if phase == "tiles" else "residual"
targets = {"base":{"Otter"}, "walrus":{"Hide","Cap"}, "heron":{"HeronHead"}, "otter":{"Head4","Torso"}}
pattern = r'\(\(name \. "([^"]+)"\) \(module \. (\d+)\) \(stride \. \d+\) \(rows ([^)]+)\) \(n_rows \. (\d+)\)'
for scene, fields in targets.items():
    filename = (out / scenes[scene]).name
    text = (out/"revised"/f"check-{filename}.ir").read_text().rsplit(" (kinds ",1)[1]
    modules = json.loads((out/"residual"/f"{scene}.show").read_text())["modules"]
    params = buffer(np.fromstring(re.search(r'\(params ([^)]+)',text)[1],sep=" ",dtype=np.uint32),U.STORAGE)
    luts = buffer(np.fromstring(re.search(r'\(luts ([^)]+)',text)[1],sep=" ",dtype=np.float32),U.STORAGE)
    at = np.fromstring(re.search(r'\(camera \(at \. #\(([^)]+)',text)[1],sep=" ")
    scalar = lambda name: float(re.search(r'\('+name+r' \. ([^)]+)',text)[1])
    for width,height in ([(960,720)] if scene=="base" else [(960,720),(1280,800)]):
        if only_size and only_size != f"{width}x{height}": continue
        stem = f"{scene}-{width}x{height}"
        paths = sorted((notes/"orbit"/phase).glob(stem+"-[0-9].json.gz"))
        attempts = [json.loads(gzip.decompress(p.read_bytes())) for p in paths]
        timing = next((a for a in attempts if not a["contended"]),attempts[0])
        index = sorted(range(120),key=timing["gpu_frame_ms"].__getitem__)[113]
        angle = math.tau*(index/60)/scalar("period")
        eye = at + [scalar("radius")*math.sin(angle),scalar("height"),scalar("radius")*math.cos(angle)]
        view = Raster(eye,at,scalar("fov"),width,height,params)
        measurements = dict(frame_index=index, native_frame_ms=timing["gpu_frame_ms"][index],
                            empty_ms=[view.draw([],timing=True) for _ in range(9)][2:], fields={})
        for kind_id, row in enumerate(re.finditer(pattern,text.split(" (instances")[0])):
            label,module,words,n = row.groups()
            kind = label.split("_")[-1]
            if kind not in fields: continue
            n=int(n)
            rows=group(row_layout,[(0,buffer(np.fromstring(words,sep=" ",dtype=np.uint32),U.STORAGE)),
                (1,buffer(np.arange(n,dtype=np.uint32),U.STORAGE)),
                (2,buffer(np.array([kind_id,0,n,0],np.uint32),U.UNIFORM)),(3,luts)])
            src=modules[int(module)]["wgsl"]
            assert "let reach = luts[" in src
            body=src[:src.index("\n}",src.index("@fragment fn fs("))+2]
            spec=[(raster(body,False),raster(instrument(src),True),rows,n)]
            launched,hits,hit_steps,miss_steps,limited=view.draw(spec,count=True)
            durations=[view.draw(spec,timing=True) for _ in range(9)][2:]
            measurements["fields"][kind]=dict(fragments=launched,hits=hits,hit_steps=hit_steps,
                                               miss_steps=miss_steps,limit=limited,ms=durations)
            print(stem,index,kind,hit_steps,miss_steps,float(np.median(durations)),flush=True)
        assert set(measurements["fields"])==fields
        checkpoint(output,stem+".json",json.dumps(measurements).encode(),
                   f"Count surviving field evaluations on the {phase} {stem} native tail pose")
