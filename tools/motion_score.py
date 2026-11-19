"""Read the three actual GPU takes from the Rust harness. Distances in metres and times in seconds."""
import hashlib
import json
import math
import struct
import sys
from pathlib import Path

folder = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("out/motion-score")
def frames(name, ext, floats):
    return list(struct.iter_unpack(f"<{floats}f", (folder/f"{name}.{ext}").read_bytes()))
states = frames("one", "state", 168)
poses = frames("one", "pose", 232)
low_s = frames("low", "state", 168)
low_p = frames("low", "pose", 232)
broken = frames("broken", "pose", 232)
def pin(s,i):
    return s[40+16*i:56+16*i]
def site(p,i):
    # The drawing places the distal pad at the translation plus its local +Y axis times the distal length.
    k = (3+3*i)*8
    x,y,z,_=p[k:k+4]
    qx,qy,qz,qw=p[k+4:k+8]
    return (x+2.4*(qx*qy-qw*qz), y+1.2*(1-2*(qx*qx+qz*qz)), z+2.4*(qy*qz+qw*qx))
lifts=[[] for _ in range(8)]
duration=[]; heights=[]; landing=[]; overlaps=0; max_plant=[0.]*8; disturbed=[[] for _ in range(8)]
low_err=0.; changed_y=0.; decision_diff=0; reported_error=0.
for k,s in enumerate(states):
    p=poses[k]; prev=states[max(0,k-1)]
    for i in range(8):
        a=pin(s,i); last=pin(prev,i); is_down=a[3]<0
        if not is_down and last[3]<0:
            lifts[i].append(k);duration.append(a[11]);heights.append(a[12])
        if is_down and last[3]>=0:
            landing.append((math.dist((a[0],a[2]),(last[0],last[2])),abs(a[1]),abs(last[1]-a[1])))
        if p[201+4*i]<0:
            err=math.dist(site(p,i),a[:3]);max_plant[i]=max(max_plant[i],err)
            reported_error=max(reported_error,abs(err-(0.0001-p[200+4*i])))
            disturbed[i].append(math.dist(site(broken[k],i),a[:3]))
        else:
            for j in range(i+1,8):
                if (j==i+4 or (i//4==j//4 and j==i+1)) and p[201+4*j]>=0: overlaps+=1
            standard=site(p,i)[1];low=site(low_p[k],i)[1];scale=1-.5*a[15]
            low_err=max(low_err,abs(low-standard*scale));changed_y=max(changed_y,standard-low)
        decision_diff+=((pin(low_s[k],i)[3]<0)!=is_down)
first_neg=next(k for k,s in enumerate(states) if s[3]<0)
max_lean=max(range(481), key=lambda k:abs(states[k][9]))
out=[
 ("2 steps", [len(x) for x in lifts],sum(map(len,lifts))),
 ("3 first lifts", [x[0] if x else None for x in lifts]),
 ("4 v<0 / next lift (ticks)",first_neg,min(k for x in lifts for k in x if k>first_neg)),
 ("5 accel / lean degrees / tick", max(math.hypot(*s[4:6]) for s in states),math.degrees(abs(states[max_lean][9])),max_lean),
 ("6 least share / hull margin / fewest down / rejected lifts",min(s[16] for s in states[1:]),min(s[17] for s in states[1:]),min(s[18] for s in states[1:]),sum(s[19] for s in states)),
 ("7 reach margin / leash", min(s[20] for s in states[1:]),sum(s[21] for s in states)),
 ("8 max observed plant error mm / neighbour overlaps / disagreement with GPU margin mm",1000*max(max_plant),overlaps,1000*reported_error),
 ("9 altered stance errors mm [min,max]",[(round(1000*min(x),5),round(1000*max(x),5)) for x in disturbed]),
 ("10 horizontal last tick mm / y at landing / vertical last tick mm",1000*max(x[0] for x in landing),max(x[1] for x in landing),1000*max(x[2] for x in landing)),
 ("10 duration and lift", min(duration),max(duration),min(heights),max(heights)),
 ("11 end position / velocity",states[-1][1],states[-1][3]),
 ("13 differing modes / lower arch error mm / maximum height reduction",decision_diff,low_err*1000,changed_y)
]
if (folder/"alternate.state").is_file():
    alt=frames("alternate","state",168)[-1]; base=states[-1]
    out.append(("12 max final site difference world / body relative",
                max(math.dist(pin(alt,i)[:3],pin(base,i)[:3]) for i in range(8)),
                max(abs(pin(alt,i)[2]-alt[1]-pin(base,i)[2]+base[1]) for i in range(8))))
for row in out: print(*row,sep="  ")

def module_hashes(name):
    bundle = (folder / f"{name}.loft").read_bytes()
    size = struct.unpack_from("<Q", bundle, 8)[0]
    header = json.loads(bundle[16:16+size])
    offset = 16 + size
    sections = []
    for length in header["sections"]:
        offset = (offset + 63) // 64 * 64
        sections.append(bundle[offset:offset+length])
        offset += length
    return [hashlib.sha256(sections[m["wgsl"]]).hexdigest()[:12] for m in header["modules"]]

for name in ["one", "low", "5"]:
    print("13/17 module hashes", name, *module_hashes(name))

deck = frames("deck", "pose", 232)
carry = [site(deck[120], i)[0]-site(deck[0], i)[0] for i in range(8)]
deck_error = max(abs(0.0001-p[200+4*i]) for p in deck for i in range(8))
loads = frames("pads", "pose", 232)[0][200:203]
print("Phase 2 deck: world carry range m / worst deck-frame hold mm", min(carry), max(carry), 1000*deck_error)
print("Phase 2 four pads: feasible range and least-norm share", loads)
assert max(abs(v-0.5) for v in carry) <= 0.001 and deck_error <= 0.0001
assert max(abs(v-w) for v,w in zip(loads,(.25,.5,.375))) <= 1e-5

left = frames("left", "pose", 232)
join_root = max(math.dist(p[:3],l[:3]) for p,l in zip(poses[1:],left[1:]))
join_site = max(math.dist(site(p,i),site(l,i)) for p,l in zip(poses[1:],left[1:]) for i in range(8))
print("left limit vs tick (max root / distal site) mm",1000*join_root,1000*join_site)
assert max(join_root,join_site) < 0.00001
