// Count is the affine support kind: flight, point, segment, or a counterclockwise polygon.
struct Hull { points:array<vec2f,16>, count:u32 }
struct Contact { closest:vec2f, margin:f32 }
fn wedge(a:vec2f,b:vec2f)->f32 { return a.x*b.y-a.y*b.x; }
fn push_hull(h:ptr<function,Hull>,p:vec2f,base:u32){
  loop {
    if (*h).count<base+2u {break;}
    let n=(*h).count;
    if wedge((*h).points[n-1]-(*h).points[n-2],p-(*h).points[n-1])>0.000001 {break;}
    (*h).count--;
  }
  (*h).points[(*h).count]=p; (*h).count++;
}
fn hull(s:ptr<function,State>,mask:u32)->Hull {
  var pts:array<vec2f,8>; var n=0u;
  for(var i=0u;i<8;i++){
    if (mask & (1u<<i))==0u {continue;}
    let p=(*s).pin[i].at.xz; var j=n;
    loop {
      if j==0u {break;}
      let q=pts[j-1]; if q.x<p.x || (q.x==p.x && q.y<=p.y) {break;}
      pts[j]=q;j--;
    }
    pts[j]=p;n++;
  }
  var h:Hull;
  for(var i=0u;i<n;i++){if i==0u || any(pts[i]!=pts[i-1]){push_hull(&h,pts[i],0u);}}
  if h.count<2u {return h;}
  let base=h.count-1u;
  for(var j=i32(n)-2;j>=0;j--){if any(pts[j]!=pts[j+1]){push_hull(&h,pts[j],base);}}
  h.count--;return h;
}
fn contact(h:ptr<function,Hull>,p:vec2f)->Contact {
  var q=p; var margin=1e10;var best=1e20;
  switch (*h).count {
    case 0u:{return Contact(p,-1e10);}
    case 1u:{q=(*h).points[0];return Contact(q,-distance(p,q));}
    default:{
      for(var j=0u;j<(*h).count;j++){
        let a=(*h).points[j];let edge=(*h).points[(j+1u)%(*h).count]-a;
        let candidate=a+edge*clamp(dot(p-a,edge)/dot(edge,edge),0.0,1.0);
        let d=dot(p-candidate,p-candidate);
        if d<best {best=d;q=candidate;}
        margin=min(margin,wedge(edge,p-a)/length(edge));
      }
      if (*h).count==2u || margin<0.0 {return Contact(q,-sqrt(best));}
      return Contact(p,margin);
    }
  }
}
fn centroid(s:ptr<function,State>,mask:u32)->vec2f {
  var sum=vec2f(0);
  for(var j=0u;j<8;j++){if (mask & (1u<<j))!=0u {sum+=(*s).pin[j].at.xz;}}
  return sum/f32(max(1u,countOneBits(mask)));
}
fn shares(s:ptr<function,State>,mask:u32,q:vec2f)->array<f32,8>{
  var w:array<f32,8>;var free_mask=mask;
  for(var round=0u;round<8 && free_mask!=0u;round++){
    let mean=centroid(s,free_mask);var xx=0.0;var zz=0.0;var xz=0.0;
    for(var j=0u;j<8;j++){if (free_mask & (1u<<j))!=0u {let d=(*s).pin[j].at.xz-mean;xx+=d.x*d.x;zz+=d.y*d.y;xz+=d.x*d.y;}}
    let det=xx*zz-xz*xz;let d=q-mean;var v=vec2f(0);
    if det>1e-6*(xx+zz)*(xx+zz) {v=vec2f(zz*d.x-xz*d.y,xx*d.y-xz*d.x)/det;}
    else if xx+zz>1e-8 {v=d/(xx+zz);}
    var least=0.0;var drop=0u;
    for(var j=0u;j<8;j++){
      w[j]=0.0;if (free_mask & (1u<<j))!=0u {
        w[j]=1.0/f32(countOneBits(free_mask))+dot((*s).pin[j].at.xz-mean,v);
        if w[j]<least {least=w[j];drop=1u<<j;}
      }
    }
    if drop==0u {break;} free_mask &= ~drop;
  }
  return w;
}
