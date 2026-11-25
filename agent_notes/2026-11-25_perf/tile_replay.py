"""Measure actual production tile lists under a fixed permutation, with an oracle for empty draws."""
# /// script
# dependencies = ["numpy", "wgpu==0.33.0"]
# ///
import json
import math
import re

import numpy as np

from count_rays import P, U, Raster, buffer, dev, frame_uniform, group, instrument, q, raster, row_layout
from fresh import checkpoint, out, root
from thin_coverage import bounds

width, height, side = 960, 720, 32
nx, ny = math.ceil(width / side), math.ceil(height / side)
compute = (root / 'crates/loft-passes/src/bundle/segment_tiles.wgsl').read_text()
shader = dev.create_shader_module(code=compute.replace('TILE_SIDE', str(side)))
project, gather = [dev.create_compute_pipeline(layout='auto', compute=dict(module=shader, entry_point=entry))
                   for entry in ('project', 'gather')]

for name in ('walrus', 'otter', 'base'):
    text = (P / f'{name}.ir').read_text().rsplit(' (kinds ', 1)[1]
    original = json.loads((P / f'{name}.show').read_text())['modules']
    compiled = json.loads((out / 'tiles' / f'{name}.show').read_text())['modules']
    params = buffer(np.fromstring(re.search(r'\(params ([^)]+)', text)[1], sep=' ', dtype=np.uint32), U.STORAGE)
    luts = buffer(np.zeros(1, np.float32), U.STORAGE)
    kinds, at, eye0, fov = bounds(name)
    angle = 60 * math.tau / 1440
    radius = np.linalg.norm((eye0 - at)[[0, 2]])
    eye = at + [radius * math.sin(angle), eye0[1] - at[1], radius * math.cos(angle)]
    view = Raster(eye, at, fov, width, height, params)
    frame = frame_uniform(eye, at, fov, width, height)
    for kind, a, b, ra, rb, _, _ in kinds:
        index = next(i for i, m in enumerate(original) if m['name'].split('_')[-1] == kind)
        label = original[index]['name']
        data = re.search(re.escape(f'(name . "{label}")') + r' \(module \. \d+\) \(stride \. \d+\) \(rows ([^)]+)', text)[1]
        fields = buffer(np.fromstring(data, sep=' ', dtype=np.uint32), U.STORAGE)
        n = len(a)
        ends = np.stack((np.column_stack((a, ra)), np.column_stack((b, rb))), axis=1)
        slots = np.random.default_rng(917).permutation(n).astype(np.uint32)
        compact = buffer(slots, U.STORAGE)
        kp = buffer(np.array([0, 0, n, 0], np.uint32), U.STORAGE | U.UNIFORM)
        resources = [buffer(ends.astype(np.float32), U.STORAGE), compact, kp,
                     buffer(np.array([36, n, 0, 0], np.uint32), U.STORAGE),
                     dev.create_buffer(size=16*n, usage=U.STORAGE),
                     dev.create_buffer(size=4*n*nx*ny, usage=U.STORAGE),
                     dev.create_buffer(size=16*nx*ny, usage=U.STORAGE | U.INDIRECT | U.COPY_SRC)]
        enc = dev.create_command_encoder()
        cp = enc.begin_compute_pass()
        for pipeline, bindings, grid in [(project, [0,1,2,3,4], (math.ceil(n/64),1,1)),
                                         (gather, [1,2,3,4,5,6], (nx,ny,1))]:
            cp.set_pipeline(pipeline)
            cp.set_bind_group(0, group(pipeline.get_bind_group_layout(0), [(0,frame)]))
            cp.set_bind_group(1, group(pipeline.get_bind_group_layout(1), [(i,resources[i]) for i in bindings]))
            cp.dispatch_workgroups(*grid)
        cp.end(); q.submit([enc.finish()])
        args = np.frombuffer(q.read_buffer(resources[6]), np.uint32).reshape(-1, 4)
        dense = [(16*i, (x, y, min(side, width-x), min(side, height-y)))
                 for i, (y, x) in enumerate((y, x) for y in range(0,height,side) for x in range(0,width,side))]
        sparse = [item for item, arg in zip(dense, args) if arg[1] > 0]
        variants = []
        for label, modules, ids, mask in [('original', original, compact, None), ('dense', compiled, resources[5], dense),
                                           ('sparse-oracle', compiled, resources[5], sparse)]:
            src = modules[index]['wgsl']
            if mask is not None:
                # The two emit roots have different path prefixes in their mangled names.
                # Match the identical fixture-relative module path and the declared kind.
                old_path, old_kind = original[index]['name'].removeprefix('loft_m_').split('_', 1)
                new_path, new_kind = modules[index]['name'].removeprefix('loft_m_').split('_', 1)
                assert bytes.fromhex(new_path).endswith(bytes.fromhex(old_path)) and new_kind == old_kind
                assert 'vi / 36u' in src
            body = src[:src.index('\n}',src.index('@fragment fn fs('))+2]
            row_group = group(row_layout, [(0,fields), (1,ids), (2,kp), (3,luts)])
            spec = [(raster(body,False), raster(instrument(src),True), row_group, n)]
            tile_draws = (resources[6], mask) if mask is not None else None
            times = [view.draw(spec, timing=True, tile_draws=tile_draws) for _ in range(9)][2:]
            raw = q.read_texture(dict(texture=view.color),dict(bytes_per_row=width*8, rows_per_image=height),(width,height,1))
            words = np.frombuffer(raw, np.uint32).copy().reshape(-1,2)
            if mask is None:
                reference = words
            launches, hits, *_ = view.draw(spec, count=True, tile_draws=tile_draws)
            variants.append(dict(variant=label, gpu_ms=times, fragments=launches, hits=hits,
                                 vis_diff=int(np.any(words!=reference,axis=1).sum())))
        result = dict(frame=60, rows=n, active_tiles=len(sparse), tiles=len(dense),
                      row_references=int(args[:,1].sum()), variants=variants)
        print(name, kind, result, flush=True)
        checkpoint('tiles',f'{name}-{kind}-fragment-replay.json',json.dumps(result).encode(),
                   f'Measure actual {kind} tile fragment count, Vis64 and sparse-oracle draw cost')
