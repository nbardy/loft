# Discrete evaluation of the phase-1 laws (crab scramble under a stick script). f64, Jacobi decisions from prev state.
import math, sys
g=9.81; h=2.5; Tc=math.sqrt(h/g); rate=120; dt=1/rate
tau=Tc/2; amax=0.6*g; R=1.3; Tmin=0.12; Tl=0.25
wr=2*math.pi*2.0; zeta=0.7
legRoots=[1.35,0.5,-0.4,-1.3]; legFeet=[(2.8,6.6),(1.1,7.4),(-0.9,7.3),(-2.7,6.4)]; span=3.0+2.9+1.2; width=3.2
homes=[(x,s*z) for s in (-1,1) for (x,z) in legFeet]            # (x, z) body frame
roots=[(x,0.1,s*width*0.78) for s in (-1,1) for x in legRoots]
N=8
def nbrs(i):
    k=i%4; out=[(i+4)%8]
    if k>0: out.append(i-1)
    if k<3: out.append(i+1)
    return out
def vdes(t, script):
    for (a,b,v) in script:
        if a<=t<b: return v
    return 0.0
def mj(s): return s*s*s*(10+s*(-15+6*s))
def run(script, seconds=4.0, varied=True, verbose=False):
    n_ticks=int(round(seconds*rate))
    cz=0.0; v=0.0; roll=0.0; rollv=0.0
    feet=[dict(aim=[hx,0.0,hz], p=[hx,0.0,hz], sw=False, frm=[hx,0.0,hz], s=0.0, T=0.24, lift=1.0, held=Tmin, steps=0) for (hx,hz) in homes]
    log=dict(lifts=[], lands=[], maxlean=(0,0), slide=0.0, together=0, reach=9e9, minload=9e9, minplanted=8, shapes=[], landspeed=0.0, copmargin=9e9, maxa=0, vneg=None)
    for n in range(1,n_ticks+1):
        t=n*dt
        vd=vdes(t,script)
        a=max(-amax,min(amax,(vd-v)/tau))
        v2=v+a*dt; cz2=cz+v2*dt
        # lean follower (semi-implicit; wr*dt=0.105)
        target=math.atan(a/g)
        racc=wr*wr*(target-roll)-2*zeta*wr*rollv
        rollv2=rollv+racc*dt; roll2=roll+rollv2*dt
        # pins: decisions from prev (cz, v) and prev feet
        home=lambda j: (homes[j][0], cz+homes[j][1])
        def strain(j):
            hx,hz=home(j); p=feet[j]['p']; return math.hypot(p[0]-hx, p[2]-(hz+v*Tc))
        def wants(j): return (not feet[j]['sw']) and strain(j)>R and feet[j]['held']>=Tmin
        planted=[j for j in range(N) if not feet[j]['sw']]
        cx=sum(feet[j]['p'][0] for j in planted)/max(len(planted),1); czc=sum(feet[j]['p'][2] for j in planted)/max(len(planted),1)
        u=max(0.0,min(1.0,(math.hypot(0-cx,(cz+v*Tc)-czc)-0.5)*0.5))
        new=[]
        for i in range(N):
            f=feet[i]; hx,hz=home(i)
            if f['sw']:
                s2=f['s']+dt/f['T']
                live=(hx, 0.0, hz+v*(Tl+max(0.0,1-s2)*f['T']))
                e=min(1.0,max(0.0,(s2-0.5)/0.35)); wgt=1-e*e*(3-2*e)
                aim=tuple(f['aim'][q]+(live[q]-f['aim'][q])*wgt for q in range(3))
                if s2>=1:
                    nf=dict(f); 
                    # horizontal landing speed: last position vs aim
                    log['landspeed']=max(log['landspeed'], math.hypot(aim[0]-f['p'][0], aim[2]-f['p'][2])/dt)
                    nf.update(p=[aim[0],0.0,aim[2]], sw=False, held=0.0); log['lands'].append((t,i))
                else:
                    m=mj(s2); b=16*s2*s2*(1-s2)*(1-s2)
                    nf=dict(f); nf.update(p=[f['frm'][0]+(aim[0]-f['frm'][0])*m, f['lift']*b, f['frm'][2]+(aim[2]-f['frm'][2])*m], s=s2, aim=list(aim))
            else:
                mine=strain(i)
                blocked=any(feet[k]['sw'] or (wants(k) and (strain(k)>mine or (strain(k)==mine and k<i))) for k in nbrs(i))
                if wants(i) and not blocked:
                    tgt=(hx, hz+v*Tl); l=math.hypot(tgt[0]-f['p'][0], tgt[1]-f['p'][2])
                    T=max(0.12,min(0.40,0.30+0.03*l-0.30*u)) if varied else 0.24
                    lift=max(0.25,min(1.6,0.7+0.25*l-1.2*u)) if varied else 1.0
                    nf=dict(f); nf.update(sw=True, frm=list(f['p']), s=0.0, T=T, lift=lift, steps=f['steps']+1, aim=[tgt[0],0.0,tgt[1]+v*T])
                    log['lifts'].append((t,i)); log['shapes'].append((T,lift,l,u))
                else:
                    nf=dict(f); nf.update(held=f['held']+dt)
            new.append(nf)
        for i in range(N):
            if (not feet[i]['sw']) and (not new[i]['sw']):
                log['slide']=max(log['slide'], math.dist(feet[i]['p'],new[i]['p']))
        for i in range(N):
            for k in nbrs(i):
                if k>i and new[i]['sw'] and new[k]['sw']: log['together']+=1
        feet=new; cz,v,roll,rollv=cz2,v2,roll2,rollv2
        if v<0 and log['vneg'] is None: log['vneg']=t
        if abs(roll)>abs(log['maxlean'][0]): log['maxlean']=(roll,t)
        log['maxa']=max(log['maxa'],abs(a))
        # reach of planted legs: root in world (roll about x: y' = y cos - z sin ; z' = y sin + z cos)
        pl=[j for j in range(N) if not feet[j]['sw']]
        log['minplanted']=min(log['minplanted'],len(pl))
        for j in pl:
            rx,ry,rz=roots[j]; yw=ry*math.cos(roll)-rz*math.sin(roll); zw=ry*math.sin(roll)+rz*math.cos(roll)
            d=math.dist((rx,h+yw,cz+zw), feet[j]['p']); log['reach']=min(log['reach'],0.97*span-d)
        # loads (min-norm affine weights) and CoP
        px=[feet[j]['p'][0] for j in pl]; pz=[feet[j]['p'][2] for j in pl]; m=len(pl)
        bx=sum(px)/m; bz=sum(pz)/m
        sxx=sum((x-bx)**2 for x in px); szz=sum((z-bz)**2 for z in pz); sxz=sum((px[k]-bx)*(pz[k]-bz) for k in range(m))
        det=sxx*szz-sxz*sxz
        copx=0.0; copz=cz-(h/g)*a
        dx=copx-bx; dz=copz-bz
        ix=(szz*dx-sxz*dz)/det; iz=(-sxz*dx+sxx*dz)/det
        w=[1/m+(px[k]-bx)*ix+(pz[k]-bz)*iz for k in range(m)]
        log['minload']=min(log['minload'],min(w))
        log['copmargin']=min(log['copmargin'], min(max(pz)-copz, copz-min(pz)))
    log.update(cz=cz, v=v, roll=roll, steps=[f['steps'] for f in feet], offhome=max(math.hypot(feet[j]['p'][0]-homes[j][0], feet[j]['p'][2]-(cz+homes[j][1])) for j in range(N)))
    return log
script=[(0.2,1.7,1.2),(1.7,3.2,-2.1)]
for varied in (False, True):
    L=run(script, 4.0, varied)
    firsts=[min(t for (t,i) in L['lifts'] if i==k) for k in range(8)]
    after=min(t for (t,i) in L['lifts'] if t>L['vneg'])
    lastland=max(t for t,i in L['lands'])
    print('varied' if varied else 'fixed', 'steps',sum(L['steps']),L['steps'],'first lifts %.3f..%.3f'%(min(firsts),max(firsts)),
      'v<0 from %.3f next lift %.3f'%(L['vneg'],after),'max lean %.2f deg at %.3f'%(math.degrees(L['maxlean'][0]),L['maxlean'][1]),
      'slide',L['slide'],'together',L['together'],'reach margin %.3f'%L['reach'],'min load share %.4f'%L['minload'],'cop margin %.2f'%L['copmargin'],
      'min planted',L['minplanted'],'land speed %.4f'%L['landspeed'],'end z %.4f v %.4f'%(L['cz'],L['v']),'last land %.3f'%lastland,'maxa %.3f'%L['maxa'],'feet off home at end %.2f'%L['offhome'])
    if varied:
        Ts=[s[0] for s in L['shapes']]; ls=[s[1] for s in L['shapes']]
        print('  swing s %.3f..%.3f lift %.3f..%.3f'%(min(Ts),max(Ts),min(ls),max(ls)))
        print('  lifts', [(round(t,3),i) for t,i in L['lifts']])
