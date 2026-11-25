"""Count occupied bins and row references with the native endpoint-ball projection."""
# /// script
# dependencies = ["numpy"]
# ///
import json
import numpy as np
from fresh import checkpoint
from thin_coverage import bounds

records=[]
for name in ("base","walrus","otter"):
    kinds,at,original,fov=bounds(name)
    radius=np.linalg.norm((original-at)[[0,2]])
    for width,height in ((960,720),(1280,800)):
        size=np.array([width,height])
        cols,rows=(size+31)//32
        for i in (0,30,60,90,119):
            angle=i*2*np.pi/1440
            eye=at+[radius*np.sin(angle),original[1]-at[1],radius*np.cos(angle)]
            forward=at-eye;forward/=np.linalg.norm(forward)
            right=np.cross(forward,[0,1,0]);right/=np.linalg.norm(right)
            up=np.cross(right,forward)
            gradient=np.array([height/width,1.])/np.tan(fov/2)
            def project(p,r):
                v=p-eye;w=(v@forward)[:,None]
                r=r+0.00001*(1+np.abs(p).sum(axis=1)[:,None]+np.abs(w))
                assert np.all(w-r>0.05)
                center=np.column_stack([v@right,-v@up])*gradient/w
                extent=r*(gradient+np.abs(center))/(w-r)
                return (np.clip(np.floor(size/2*(1+center-extent)-2),0,size).astype(int),
                        np.clip(np.ceil(size/2*(1+center+extent)+2),0,size).astype(int))
            for kind,a,b,ra,rb,*_ in kinds:
                al,ah=project(a,ra);bl,bh=project(b,rb)
                lo=np.minimum(al,bl);hi=np.maximum(ah,bh)
                use=(hi>lo).all(axis=1)
                lo=lo[use]//32;hi=(hi[use]-1)//32+1
                diff=np.zeros((rows+1,cols+1),dtype=int)
                for x,y,sign in [(lo[:,0],lo[:,1],1),(hi[:,0],lo[:,1],-1),(lo[:,0],hi[:,1],-1),(hi[:,0],hi[:,1],1)]:
                    np.add.at(diff,(y,x),sign)
                overlaps=diff.cumsum(axis=0).cumsum(axis=1)[:-1,:-1]
                records.append(dict(scene=name,kind=kind,size=f"{width}x{height}",frame=i,
                                    occupied=int((overlaps>0).sum()),tiles=int(cols*rows),
                                    row_references=int(overlaps.sum()),max_rows=int(overlaps.max())))
checkpoint("tiles","occupancy.json",json.dumps(records).encode(),"Count occupied native 32-pixel bins before GPU timing")
