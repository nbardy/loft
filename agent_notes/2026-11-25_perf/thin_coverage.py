import itertools, re, json
from pathlib import Path
import numpy as np

def box_faces(lo,hi,project):
    projected=np.stack([project(np.where(c,hi,lo)) for c in itertools.product([False,True],repeat=3)])
    return projected.min(axis=0),projected.max(axis=0)

def bounds(name):
    text=Path(f'out/perf/model/{name}.ir').read_text().rsplit(' (kinds ',1)[1]; kinds=[]
    for label,stride,words in re.findall(r'\(\(name \. "([^"]+)"\) \(module \. \d+\) \(stride \. (\d+)\) \(rows ([^)]+)',text.split(' (instances')[0]):
        n=label.split('_')[-1]
        if n not in ['Strand','Whisker','Rod','Bit']:continue
        rows=np.fromstring(words,sep=' ',dtype=np.uint32).view(np.float32).astype(float).reshape(-1,int(stride))
        if n=='Strand':
            a=rows[:,9:12]; b=a+np.einsum('nji,nj->ni',rows[:,:9].reshape(-1,3,3),rows[:,12:15]); ra,rb=rows[:,15:16],rows[:,16:17]
        else:
            a,b=rows[:,:3], rows[:,3:6]; ra=rows[:,6:7]; rb=ra if n=='Rod' else rows[:,7:8]
        kinds.append((n,a,b,ra,rb,rows[:,-7:-4],rows[:,-4:-1]))
    v=lambda q:np.fromstring(re.search(q+r'[^#]*#\(([^)]+)',text)[1],sep=' ')
    return kinds,v(r'\(camera \(at'),v(r'\(fixed_eye'),float(re.search(r'\(fov \. ([^)]+)',text)[1])

def score(name,width,height):
    kinds,at,original,fov=bounds(name); dist=np.hypot(*(original-at)[[0,2]])
    counts={}; screen=np.array([width,height]); groups=[1,4,8,16,32,64]
    for i in [0,30,60,90,119]:
        phi=i*(2*np.pi/1440)
        eye=at+[dist*np.sin(phi),original[1]-at[1],dist*np.cos(phi)]
        unit=lambda x:x/np.linalg.norm(x)
        forward=unit(at-eye);right=unit(np.cross(forward,[0,1,0])); up=np.cross(right,forward)
        def project(v):
            ray=v-eye
            assert (ray@forward > 0.05).all()
            return screen/2 + np.column_stack([ray@right,-ray@up])/((ray@forward)[:,None])*height/(2*np.tan(fov/2))
        for kind,a,b,ra,rb,oldlo,oldhi in kinds:
            lo,hi=box_faces(oldlo,oldhi,project)
            al,ah=box_faces(a-ra,a+ra,project);bl,bh=box_faces(b-rb,b+rb,project)
            inner_lo=np.minimum(al,bl); inner_hi=np.maximum(ah,bh)
            # Cover the whole pixel at the boundary, including its helpers.
            lower=np.maximum(0,np.floor(inner_lo)-2); upper=np.minimum(screen,np.ceil(inner_hi)+2)
            old_area=np.prod(np.maximum(0,np.minimum(hi,screen)-np.maximum(lo,0)),axis=1)
            delta=project(b)-project(a)
            tangent=delta/np.maximum(1e-9,np.linalg.norm(delta,axis=1)[:,None])
            normal=np.stack((-tangent[:,1],tangent[:,0]),axis=-1)
            points=np.stack([project(v+r*np.array(c)) for v,r in [(a,ra),(b,rb)] for c in itertools.product([-1,1],repeat=3)])
            cross=(points*normal[None,:,:]).sum(axis=-1)
            strip=np.ptp(cross,axis=0)+4.
            strip_area=np.minimum(old_area,strip*((hi-lo)*abs(tangent)).sum(axis=-1))
            tiles=[]
            for side in [16,32,64,96]:
                extent=np.minimum(hi,np.ceil(upper/side)*side)-np.maximum(lo,np.floor(lower/side)*side)
                tiles.append(int(np.prod(np.maximum(0,extent),axis=1).sum()))
            print(name,kind,i,'strip, tiled 16/32/64/96, old rectangle',int(strip_area.sum()),tiles,int(old_area.sum()))
            totals=[]
            for g in groups:
                idx=np.arange(0,len(a),g)
                gl=np.minimum.reduceat(lower,idx); gu=np.maximum.reduceat(upper,idx)
                y=np.minimum(np.repeat(gu,g,axis=0)[:len(a)],hi)-np.maximum(np.repeat(gl,g,axis=0)[:len(a)],lo)
                totals.append(round(np.prod(np.maximum(0,y),axis=1).sum()))
            counts.setdefault(kind,[]).append(totals)
    return {k:{str(g):int(n) for g,n in zip(groups,np.median(frames,axis=0))} for k,frames in counts.items()}
if __name__ == '__main__':
    for n in ['walrus','base','otter']:
        print(n,score(n,960,720))
