"""Project the authored inner boxes onto the primary camera at 960x720."""
from pathlib import Path
import sys
import math,json,re
import numpy as np
p=Path(sys.argv[1])
result={}
for name in ('base','walrus','heron','otter'):
 t=(p/'model'/f'{name}.ir').read_text().rsplit(' (kinds ',1)[1]
 def vec(key):
  return np.fromstring(re.search(key+r'[^#]*#\(([^)]+)',t)[1],sep=' ')
 at,eye=vec(r'\(camera \(at'),vec(r'\(fixed_eye');fov=float(re.search(r'\(fov \. ([^)]+)',t)[1])
 def unit(v):return v/np.linalg.norm(v)
 f=unit(at-eye);side=unit(np.cross(f,[0,1,0]));up=np.cross(side,f); w,h=960,720
 yy,xx=np.mgrid[:h,:w]
 rd=f+side*((xx+.5-w/2)*2*math.tan(fov/2)/h)[:,:,None]-up*((yy+.5-h/2)*2*math.tan(fov/2)/h)[:,:,None]
 if name=='base': boxes={'Otter':([.11,-.18,-.20],[.75,.25,.20])}
 if name=='walrus': boxes={'Hide':([-.44,-.12,-.52],[.45,.75,.67]),'Cap':([-.35,.22,-.20],[.37,.8,.46])}
 if name=='heron': boxes={'HeronHead':([-.18,.39,-.21],[.24,.78,.21])}
 if name=='otter':
  rows={}
  for label,raw in re.findall(r'\(\(name \. "([^"]+)"\) \(module \. \d+\) \(stride \. \d+\) \(rows ([^)]+)\) \(n_rows \. 1\)',t):
   rows[label.split('_')[-1]]=np.fromstring(raw,sep=' ',dtype=np.uint32).view(np.float32)
  c=rows['Head4'][:3];v=rows['Torso'];j=np.array([v[i:i+3] for i in [0,3,*range(7,31,3)]])
  boxes={'Head4':(c-.16,c+.16),'Torso':(j.min(axis=0)-.13,j.max(axis=0)+.16)}
 for k,(lo,hi) in boxes.items():
  a=(lo-eye)/rd;b=(hi-eye)/rd
  near=np.maximum(np.minimum(a,b).max(axis=2),0.);far=np.maximum(a,b).min(axis=2)
  result[k]=dict(entered=int(np.count_nonzero(near<=far)),full=w*h,lo=np.asarray(lo).tolist(),hi=np.asarray(hi).tolist())
print(json.dumps(result,indent=2))
