"""Replay Loft's generated Vis on Metal. Count *all* proxy fragments, including misses.
uv run --with numpy --with wgpu count_rays.py out/perf/model ball base walrus heron otter
Inputs are loft ir -o NAME.ir and loft show --json >NAME.show.
Calibration uses a separate camera, raster extent and repeated draws of each kind.
"""
import json
import math
import re
import sys
import subprocess
from pathlib import Path

import numpy as np
import wgpu

U = wgpu.BufferUsage
P = Path(sys.argv[1])
adapter = wgpu.gpu.request_adapter_sync(power_preference='high-performance')
dev = adapter.request_device_sync(required_features=['timestamp-query'])
q = dev.queue
stages = wgpu.ShaderStage.VERTEX | wgpu.ShaderStage.FRAGMENT


def layout(form):
    return dev.create_bind_group_layout(entries=[
        dict(binding=i, visibility=stages if t != 'storage' else wgpu.ShaderStage.FRAGMENT,
             buffer=dict(type=t)) for i, t in form
    ])


def group(bgl, slots):
    return dev.create_bind_group(layout=bgl, entries=[
        dict(binding=i, resource=dict(buffer=b)) for i, b in slots
    ])


frame_layout = layout([(0, 'uniform'), (2, 'read-only-storage')])
row_layout = layout([(0, 'read-only-storage'), (1, 'read-only-storage'), (2, 'uniform'), (3, 'read-only-storage')])
counter_layout = layout([(9, 'storage')])
layouts = [dev.create_pipeline_layout(bind_group_layouts=[frame_layout, row_layout] + extra)
           for extra in ([], [counter_layout])]


def buffer(data, usage):
    return dev.create_buffer_with_data(data=data, usage=usage)


def raster(source, instrumented):
    module = dev.create_shader_module(code=source)
    return dev.create_render_pipeline(layout=layouts[instrumented],
        vertex=dict(module=module, entry_point='vs'),
        fragment=dict(module=module, entry_point='fs', targets=[dict(format='rg32uint')]),
        primitive=dict(topology='triangle-list', front_face='ccw', cull_mode='front'),
        depth_stencil=dict(format='depth32float', depth_write_enabled=True, depth_compare='less'))


def instrument(source):
    start = source.index('@fragment fn fs(')
    head, fs = source[:start], source[start:]
    fs = fs[:fs.index('\n}') + 2]
    head += '@group(2) @binding(9) var<storage, read_write> counts: array<atomic<u32>>;\n'
    # Each invocation publishes once, including those that missed or used their last step.
    fs = fs.replace('let row =', 'var steps = 0u; let row =')
    fs = fs.replace('let d = q_trace(row,', 'steps += 1u; let d = q_trace(row,')
    pos = '((u32(v.pos.x) + 17u * u32(v.pos.y)) & 255u) * 5u'
    fs = fs.replace('var steps', f'let slot = {pos}; atomicAdd(&counts[slot], 1u); var steps')
    miss = 'atomicAdd(&counts[slot+3u], steps); if steps == 192u { atomicAdd(&counts[slot+4u], 1u); } return FsOut(vec2u(0u), 1.0);'
    for cond in ('march_exit != march_hit', 't < 0.0'):
        fs = fs.replace(f'if {cond} {{ discard; }}', f'if {cond} {{ {miss} }}')
    fs = fs.replace('  let clip =', '  atomicAdd(&counts[slot+1u], 1u); atomicAdd(&counts[slot+2u], steps);\n  let clip =')
    return head + fs


def frame_uniform(eye, at, fov, width, height):
    def unit(v): return v / np.linalg.norm(v)
    forward = unit(at-eye)
    side = unit(np.cross(forward, [0,1,0]))
    rotation = np.stack([side, np.cross(side,forward), -forward])
    view = np.eye(4)
    view[:3,:3], view[:3,3] = rotation, -rotation @ eye
    h = 1 / math.tan(fov/2)
    proj = np.array([[h*height/width,0,0,0],[0,h,0,0],[0,0,400/(.05-400),20/(.05-400)],[0,0,-1,0]])
    vp = proj @ view
    data = np.zeros(64, np.float32)
    data[:16], data[16:32] = vp.T.flatten(), np.linalg.inv(vp).T.flatten()
    data[32:35] = eye
    data.view(np.uint32)[40:44] = [width,height,(width+7)//8,((width+7)//8)*((height+7)//8)]
    return buffer(data, U.UNIFORM)


class Raster:
    def __init__(self, eye, at, fov, w, h, params):
        self.w, self.h = w, h
        self.frame = group(frame_layout, [(0,frame_uniform(eye,at,fov,w,h)),(2,params)])
        def tex(fmt):
            return dev.create_texture(size=(w,h,1),format=fmt,
                usage=wgpu.TextureUsage.RENDER_ATTACHMENT | wgpu.TextureUsage.COPY_SRC)
        self.color, self.depth = tex('rg32uint'), tex('depth32float')
        self.cview, self.dview = self.color.create_view(), self.depth.create_view()
        self.query = dev.create_query_set(type='timestamp',count=2)
        self.resolve = dev.create_buffer(size=16,usage=U.QUERY_RESOLVE|U.COPY_SRC)
        self.count = dev.create_buffer(size=256*5*4,usage=U.STORAGE|U.COPY_SRC|U.COPY_DST)
        self.cg = group(counter_layout,[(9,self.count)])

    def draw(self, kinds, count=False, repetitions=1, timing=False, occluded=False, rects=None, tile_draws=None):
        enc=dev.create_command_encoder()
        if count: enc.clear_buffer(self.count)
        stamps=dict(query_set=self.query,beginning_of_pass_write_index=0,end_of_pass_write_index=1) if timing else None
        rp=enc.begin_render_pass(timestamp_writes=stamps,
            color_attachments=[dict(view=self.cview,resolve_target=None,load_op='clear',store_op='store',clear_value=(2**32-1,0,0,0))],
            depth_stencil_attachment=dict(view=self.dview,depth_load_op='clear',depth_clear_value=0 if occluded else 1,depth_store_op='store'))
        for beauty, probe, rows, n in kinds:
            rp.set_pipeline(probe if count else beauty)
            rp.set_bind_group(0,self.frame); rp.set_bind_group(1,rows)
            if count: rp.set_bind_group(2,self.cg)
            for _ in range(repetitions):
                if tile_draws is not None:
                    indirect, rectangles = tile_draws
                    for offset, rect in rectangles:
                        rp.set_scissor_rect(*rect)
                        rp.draw_indirect(indirect, offset)
                elif rects is None:
                    rp.draw(36,n)
                else:
                    for first, count_rows, (x,y,w,h) in rects:
                        if w > 0 and h > 0:
                            rp.set_scissor_rect(x,y,w,h)
                            rp.draw(36,count_rows,0,first)
        rp.end()
        q.submit([enc.finish()])
        if timing:
            q.on_submitted_work_done_sync()
            enc=dev.create_command_encoder()
            enc.resolve_query_set(self.query,0,2,self.resolve,0)
            q.submit([enc.finish()])
            ticks=np.frombuffer(q.read_buffer(self.resolve),np.uint64)
            return float(ticks[1]-ticks[0])/1e6
        if count:
            return np.frombuffer(q.read_buffer(self.count),np.uint32).reshape(-1,5).sum(axis=0).astype(int).tolist()


def collect(name):
    # The lane can also run from its pinned local baseline build and fixture copies.
    basename = {'ball':'start/ball','base':'start/otter','walrus':'walrus/tq',
                'heron':'heron/h02_hero_q3','otter':'otter/hero_q3'}[name]
    scene = P.parent/'baseline/source/fixtures'/(basename+'.loft')
    for ext in ('ir','show'):
        dest = P/f'{name}.{ext}'
        if not dest.exists() or not dest.stat().st_size:
            tmp = dest.with_suffix('.counter-tmp')
            with tmp.open('w') as file:
                args = ['ir',scene] if ext=='ir' else ['show',scene,'--json']
                subprocess.run([P.parent/'baseline/loft',*args],stdout=file,check=True)
            tmp.replace(dest)
    text=(P/f'{name}.ir').read_text().rsplit(' (kinds ',1)[1]
    modules=json.loads((P/f'{name}.show').read_text())['modules']
    def vec(key): return np.fromstring(re.search(key+r'[^#]*#\(([^)]+)',text)[1],sep=' ')
    at, eye = vec(r'\(camera \(at'), vec(r'\(fixed_eye')
    fov=float(re.search(r'\(fov \. ([^)]+)',text)[1])
    params=buffer(np.fromstring(re.search(r'\(params ([^)]+)',text)[1],sep=' ',dtype=np.uint32),U.STORAGE)
    luts=buffer(np.fromstring(re.search(r'\(luts ([^)]+)',text)[1],sep=' ',dtype=np.float32),U.STORAGE)
    test=Raster(eye,at,fov,960,720,params)
    # Calibration rotates the camera 7 degrees and pulls back by 10 percent.
    a=math.radians(7); rot=np.array([[math.cos(a),0,math.sin(a)],[0,1,0],[-math.sin(a),0,math.cos(a)]])
    cal=Raster(at+1.1*rot@(eye-at),at,fov,672,504,params)
    empty=float(np.median([cal.draw([],timing=True) for _ in range(9)][2:]))
    records={}; all_kinds=[]
    for m in re.finditer(r'\(\(name \. "([^"]+)"\) \(module \. (\d+)\) \(stride \. (\d+)\) \(rows ([^)]+)\) \(n_rows \. (\d+)\) \(lipschitz ([^)]+)\) \(draw \. (\w+)\)',text.split(' (instances')[0]):
        kind,idx,stride,data,n,ls,draw=m.groups(); n=int(n)
        src=modules[int(idx)]['wgsl']
        body=src[:src.index('\n}',src.index('@fragment fn fs('))+2]
        rows=group(row_layout,[(0,buffer(np.fromstring(data,sep=' ',dtype=np.uint32),U.STORAGE)),
            (1,buffer(np.arange(n,dtype=np.uint32),U.STORAGE)),
            (2,buffer(np.array([len(all_kinds),0,n,0],np.uint32),U.UNIFORM)),(3,luts)])
        spec=(raster(body,False),raster(instrument(src),True),rows,n)
        all_kinds.append(spec)
        counts=test.draw([spec],True)
        cc=cal.draw([spec],True)
        # Four draws amortize the single attachment clear/store. All use late frag_depth.
        tt=[cal.draw([spec],repetitions=4,timing=True) for _ in range(9)][2:]
        hidden=[cal.draw([spec],repetitions=4,timing=True,occluded=True) for _ in range(5)][1:]
        records[kind]=dict(calibration_occluded_four_ms=hidden,rows=n,draw=draw,L=max(map(float,ls.split())),target=counts,
                           calibration=cc,calibration_four_draw_ms=tt)
        print(name,kind,counts, round(np.median(tt),3),flush=True)
    if not records: raise ValueError('no raster kinds parsed')
    combined = [cal.draw(all_kinds,timing=True) for _ in range(12)][3:]
    test.draw(all_kinds)
    raw=q.read_texture(dict(texture=test.color),dict(bytes_per_row=960*8,rows_per_image=720),(960,720,1))
    ids=np.frombuffer(raw,np.uint32).reshape(-1,2)[:,0]>>24
    for i,record in enumerate(records.values()): record['shaded']=int(np.sum(ids==i))
    (P/f'{name}.counts.json').write_text(json.dumps(dict(adapter=adapter.info['device'],cal_empty_ms=empty,cal_combined_ms=combined,kinds=records),indent=2))

if __name__ == '__main__':
    for name in sys.argv[2:]:
        collect(name)
