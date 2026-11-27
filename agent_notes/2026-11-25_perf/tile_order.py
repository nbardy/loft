"""Exercise the two native binning entry points with deliberate row reordering and holes."""
# /// script
# dependencies = ["wgpu==0.33.0"]
# ///
import json
import struct

import wgpu
from fresh import checkpoint, root

device=wgpu.gpu.request_adapter_sync(power_preference="high-performance").request_device_sync()
queue=device.queue
U=wgpu.BufferUsage
def words(values):
    return struct.pack(f"<{len(values)}I", *values)
def buf(data, usage=U.STORAGE):
    return device.create_buffer_with_data(data=data, usage=usage|U.COPY_SRC|U.COPY_DST)
def group(pipeline, index, resources):
    return device.create_bind_group(layout=pipeline.get_bind_group_layout(index),entries=[
        dict(binding=i,resource=dict(buffer=b)) for i,b in resources.items()])
width,height=95,65
n=145
dims=(3,3)
identity=[float(i%5==0) for i in range(16)]
frame=buf(struct.pack("<40f",*(identity*2+[0.]*8))+words([width,height,0,0]),U.UNIFORM)
empty=2**32-1
# A compacted span need not start at zero or preserve authored order. Single-kind culls can contain holes.
slots=[empty if j%23==0 else (41*j+7)%n for j in range(n-3)]
membership=[]; balls=[]
for i in range(n):
    tx,ty=i%3,(i//3)%3
    x,y=32*tx+8,min(height-1,32*ty+8)
    # Last raster row straddles the y=64 tile edge after the two-pixel guard.
    radius=-1.0 if i%59==0 else 0.00001
    balls.extend([2*x/width-1,1-2*y/height,0.5,radius]*2)
    if radius<0:
        membership.append(set(range(9)))
    else:
        membership.append({k for k in range(9) if k%3==tx and (k//3==ty or ty==2 and k//3==1)})
ends=buf(struct.pack(f"<{len(balls)}f",*balls))
visible=buf(words([empty]*4+slots))
kp=buf(words([1,4,n,0]))
source=(root/"crates/loft-passes/src/bundle/segment_tiles.wgsl").read_text().replace("TILE_SIDE","32")
report=[]
for stride in (4,5):
    shader=device.create_shader_module(code=source.replace("= 4u;",f"= {stride}u;"))
    arg_values=[0]*(2*stride);arg_values[stride+1]=len(slots)
    dispatch_args=buf(words(arg_values))
    rects=buf(bytes(16*n)); result=buf(bytes(4*n*9)); tile_args=buf(bytes(16*9))
    resources={0:ends,1:visible,2:kp,3:dispatch_args,4:rects,5:result,6:tile_args}
    encoder=device.create_command_encoder()
    work=encoder.begin_compute_pass()
    for entry,used,groups in [('project',[0,1,2,3,4],(3,1,1)),('gather',[1,2,3,4,5,6],(3,3,1))]:
        pipeline=device.create_compute_pipeline(layout="auto",compute=dict(module=shader,entry_point=entry))
        work.set_pipeline(pipeline)
        work.set_bind_group(0,group(pipeline,0,{0:frame}))
        work.set_bind_group(1,group(pipeline,1,{i:resources[i] for i in used}))
        work.dispatch_workgroups(*groups)
    work.end();queue.submit([encoder.finish()])
    ids=struct.unpack(f"<{n*9}I",queue.read_buffer(result))
    draws=list(struct.iter_unpack("<4I",queue.read_buffer(tile_args)))
    for tile,(vertices,count,first,instance) in enumerate(draws):
        expected=[r for r in slots if r!=empty and tile in membership[r]]
        assert (vertices,count,first,instance)==(36,len(expected),36*(tile+1),0),(stride,tile,draws[tile],len(expected))
        assert list(ids[n*tile:n*tile+count])==expected,(stride,tile)
    report.append(dict(stride=stride,slot_count=len(slots),tile_counts=[d[1] for d in draws]))
checkpoint("tiles","permuted-cull.json",json.dumps(report).encode(),"Verify stable GPU tile lists with a permuted cull, holes and partial edge tiles")
