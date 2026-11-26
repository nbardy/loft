#!/usr/bin/env python3
"""Build out/spark_pairs/loft3d/gallery.html from out/spark_pairs/votes.json.

Same ballot the CLI serves (list_pairs.sh), as a clickable page: filmstrips
per chain, A/B cards, keys 1/2/0, votes in localStorage, export in the
'<pair-id>: a|b' text form the CLI asks for. Fully self-contained file:// page.
"""
import json
import os

ROOT = "out/spark_pairs"
L3D = os.path.join(ROOT, "loft3d")
PREFIX = "out/spark_pairs/loft3d/"

CSS = """body{font-family:system-ui,sans-serif;max-width:1100px;margin:0 auto;
padding:16px;background:#f4f2ee;color:#222}img{width:100%;border-radius:8px;
background:#ddd}.grid{display:grid;grid-template-columns:1fr 1fr;gap:12px}
.card{background:#fff;border-radius:12px;padding:12px;margin:14px 0;
box-shadow:0 1px 4px #0002}.row{display:flex;gap:8px;margin-top:8px;flex-wrap:wrap}
button{font-size:15px;padding:8px 14px;border-radius:8px;border:1px solid #999;
background:#fff;cursor:pointer}button.picked{background:#222;color:#fff}
.strip{display:flex;gap:4px;overflow-x:auto;margin:8px 0}.strip img{width:150px}
.toolbar{position:sticky;top:0;background:#f4f2ee;padding:8px 0;display:flex;
gap:8px;align-items:center;flex-wrap:wrap}.bar{height:8px;background:#ddd;
border-radius:4px;flex:1;min-width:120px}.bar>i{display:block;height:100%;
background:#2a7;border-radius:4px}"""

JS = """const PAIRS=__PAIRS__;const CHAINS=__CHAINS__;
function rel(p){return p.replace("out/spark_pairs/loft3d/","");}
function steps(pid){const m=pid.match(/(\\d{2})v(\\d{2})$/);return m?m.slice(1):["?","?"];}
function kind(pid){const m=pid.match(/-(adjacent|baseline|stride\\d+)-/);return m?m[1]:"pair";}
const V=JSON.parse(localStorage.getItem("loft3d_votes")||"{}");
const sel=document.getElementById("f");
CHAINS.forEach(c=>{const o=document.createElement("option");o.value=c;
o.textContent=c;sel.appendChild(o)});
const st=document.getElementById("strips");
CHAINS.forEach(ch=>{const d=document.createElement("div");d.className="card";
d.innerHTML="<h3>"+ch+"</h3>";const s=document.createElement("div");
s.className="strip";for(let t=0;t<=8;t++){const im=document.createElement("img");
im.src=ch+"/step_"+String(t).padStart(2,"0")+".png";im.loading="lazy";
im.title=ch+" step "+t;s.appendChild(im)}d.appendChild(s);st.appendChild(d)});
function draw(){const f=sel.value;
const list=PAIRS.filter(p=>!f||p.id.startsWith("loft3d-"+f+"-"));
const done=list.filter(p=>V[p.id]).length;
document.getElementById("p").style.width=(100*done/Math.max(1,list.length))+"%";
document.getElementById("c").textContent=done+"/"+list.length+" voted";
const b=document.getElementById("ballot");b.innerHTML="";
list.forEach((p)=>{const sa=steps(p.id)[0],sb=steps(p.id)[1];
const d=document.createElement("div");d.className="card";
d.innerHTML="<h3>"+p.id+" <small>("+kind(p.id)+")</small></h3><p><i>"+p.criteria+"</i></p>"+
"<div class='grid'><div><img src='"+rel(p.a.render)+"'><p>A: step "+sa+"</p></div>"+
"<div><img src='"+rel(p.b.render)+"'><p>B: step "+sb+"</p></div></div>";
const r=document.createElement("div");r.className="row";
[["a","A (1)"],["b","B (2)"],["tie","Tie (0)"]].forEach(([v,l])=>{
const btn=document.createElement("button");btn.textContent=l;
if(V[p.id]===v)btn.className="picked";
btn.onclick=()=>{V[p.id]=v;localStorage.setItem("loft3d_votes",JSON.stringify(V));draw()};
r.appendChild(btn)});d.appendChild(r);b.appendChild(d)});window._list=list}
sel.onchange=draw;document.getElementById("e").onclick=()=>{
const lines=Object.entries(V).map(([id,c])=>id+": "+c).join("\\n");
const blob=new Blob([lines+"\\n"],{type:"text/plain"});
const a=document.createElement("a");a.href=URL.createObjectURL(blob);
a.download="loft3d_votes.txt";a.click()};
document.addEventListener("keydown",e=>{const list=window._list||[];
if(!list.length)return;const map={"1":"a","2":"b","0":"tie"};
if(!(e.key in map))return;
const cards=[...document.querySelectorAll("#ballot .card")];
let idx=Math.max(0,cards.findIndex(c=>{const r=c.getBoundingClientRect();
return r.top<innerHeight&&r.bottom>0}));
const p=list[Math.min(idx,list.length-1)];V[p.id]=map[e.key];
localStorage.setItem("loft3d_votes",JSON.stringify(V));draw();
const nc=document.querySelectorAll("#ballot .card")[Math.min(idx+1,list.length-1)];
if(nc)nc.scrollIntoView({block:"nearest"})});
draw();"""


def main():
    votes = json.load(open(os.path.join(ROOT, "votes.json")))
    pairs = [p for p in votes["pairs"] if p["id"].startswith("loft3d-")]
    chains = sorted({p["id"][len("loft3d-"):].rsplit("-", 2)[0]
                     for p in pairs})
    html = ("""<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Loft3d Ballot: real stills</title><style>""" + CSS + """</style></head><body>
<h1>Loft3d Ballot: real stills, close pairs</h1>
<p>Every image is a real <tt>loft still</tt> render. Keys: <b>1</b>=left,
<b>2</b>=right, <b>0</b>=tie. Votes persist in this browser; export writes the
<tt>&lt;pair-id&gt;: a|b</tt> lines the CLI ballot asks for.</p>
<div class="toolbar"><label>Chain <select id="f"><option value="">all</option>
</select></label><div class="bar"><i id="p" style="width:0%"></i></div>
<span id="c"></span><button id="e">Export votes</button></div>
<div id="strips"></div><div id="ballot"></div>
<script>""" + JS.replace("__PAIRS__", json.dumps(pairs)).replace(
        "__CHAINS__", json.dumps(chains)) + """</script></body></html>""")
    with open(os.path.join(L3D, "gallery.html"), "w") as f:
        f.write(html)
    print(f"gallery: pairs={len(pairs)} chains={chains}")


if __name__ == "__main__":
    main()
