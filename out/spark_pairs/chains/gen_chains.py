"""Iteration chains for spark-pair voting: shape / color / texture / expression.

Each chain renders steps 0..10 with nested deltas (step t+1 = step t + more of
the axis trait), so consecutive pairs are close and step-0-vs-later pairs show
the full improvement arc. Writes PNG steps, steps.jsonl (logger schema),
pairs.jsonl (voting UI format), and gallery.html (self-contained ballot UI).
Deterministic: fixed seeds, no network, no fonts.
"""
import json
import math
import os
from datetime import datetime, timezone

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.dirname(os.path.abspath(__file__))  # out/spark_pairs/chains
SIZE = 512
BG = (232, 230, 226)
TS = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
AGENT = "spark-iter-01"


def canvas():
    img = Image.new("RGB", (SIZE, SIZE), BG)
    d = ImageDraw.Draw(img)
    # soft studio vignette: darker corners via concentric rects
    for i in range(24):
        a = int(6 + i * 0.9)
        d.rectangle([i * 4, i * 4, SIZE - i * 4, SIZE - i * 4],
                    outline=(BG[0] - a, BG[1] - a, BG[2] - a))
    return img, d


def superellipse(cx, cy, a, b, n, teeth=160):
    pts = []
    for i in range(teeth):
        th = 2 * math.pi * i / teeth
        c, s = math.cos(th), math.sin(th)
        x = a * math.copysign(abs(c) ** (2.0 / n), c)
        y = b * math.copysign(abs(s) ** (2.0 / n), s)
        pts.append((cx + x, cy + y))
    return pts


STONE = (148, 138, 122)
STONE_DK = (110, 100, 86)
CRACK = (88, 78, 64)


def golem_body(d, n_exp, cx=256, cy=280, a=130, b=150):
    # stubby arms first (behind torso): identity anchor across steps
    for s in (-1, 1):
        x0, y0 = cx + s * (a - 6), cy + 30
        x1, y1 = cx + s * (a + 42), cy + 92
        d.line([(x0, y0), (x1, y1)], fill=STONE_DK, width=40)
        d.ellipse([x0 - 20, y0 - 20, x0 + 20, y0 + 20], fill=STONE_DK)
        d.ellipse([x1 - 22, y1 - 22, x1 + 22, y1 + 22], fill=STONE)
    d.polygon(superellipse(cx, cy, a + 14, b + 14, n_exp), fill=STONE_DK)
    d.polygon(superellipse(cx, cy, a, b, n_exp), fill=STONE)
    # stacked-block seams (fixed positions so only roundness varies)
    for yy in (cy - 70, cy, cy + 70):
        d.line([(cx - a + 18, yy), (cx + a - 18, yy)], fill=CRACK, width=4)
    d.line([(cx, cy - b + 12), (cx, cy + b - 12)], fill=CRACK, width=4)


def golem_face(d, smile, cx=256, cy=250):
    # eyes: warmth grows with t (bigger amber eyes, lids relax)
    er = 16 + 8 * smile
    for ex in (cx - 52, cx + 52):
        d.ellipse([ex - er, cy - er, ex + er, cy + er], fill=(245, 240, 228))
        pr = 7 + 3 * smile
        d.ellipse([ex - pr, cy - pr + 3, ex + pr, cy + pr + 3],
                  fill=(255, 170, 40))
        d.ellipse([ex - 3, cy, ex + 3, cy + 6], fill=(40, 30, 20))
    # spectacles (fixed, identity anchor)
    for ex in (cx - 52, cx + 52):
        d.ellipse([ex - er - 8, cy - er - 8, ex + er + 8, cy + er + 8],
                  outline=(70, 55, 40), width=4)
    d.line([(cx - 52 + er + 8, cy), (cx + 52 - er - 8, cy)],
           fill=(70, 55, 40), width=4)
    # mouth: flat line -> full smile arc
    if smile <= 0.02:
        d.line([(cx - 34, cy + 78), (cx + 34, cy + 78)], fill=(60, 48, 38),
               width=6)
    else:
        d.arc([cx - 52, cy + 78 - 58 * smile, cx + 52, cy + 78 + 58 * smile],
              start=15, end=165, fill=(60, 48, 38), width=7)


def _inside_superellipse(px, py, cx, cy, a, b, n):
    v = abs((px - cx) / a) ** n + abs((py - cy) / b) ** n
    return v <= 1.0


# fixed candidate moss spots per chain (nested: step t shows first k)
def moss_candidates(seed, cx=256, cy=280, a=130, b=150, n=2.2, m=900):
    rng = np.random.default_rng(seed)
    pts = []
    while len(pts) < m:
        batch = rng.uniform([cx - a, cy - b], [cx + a, cy + b],
                            size=(m * 2, 2))
        for p in batch:
            if _inside_superellipse(p[0], p[1], cx, cy, a - 12, b - 12, n):
                pts.append((float(p[0]), float(p[1])))
                if len(pts) >= m:
                    break
    return pts


MOSS = [(74, 124, 62), (58, 110, 54), (92, 140, 74)]


def paint_moss(img, pts, k, seed):
    rng = np.random.default_rng(seed + 999)
    radii = rng.uniform(2.0, 4.5, size=len(pts))
    cols = [MOSS[i % 3] for i in range(len(pts))]
    d = ImageDraw.Draw(img)
    for (x, y), r, c in zip(pts[:k], radii[:k], cols[:k]):
        d.ellipse([x - r, y - r, x + r, y + r], fill=c)


def render_shape(t):
    n_exp = 2.0 + (6.0 - 2.0) * t / 10.0  # round -> angular slab
    img, d = canvas()
    golem_body(d, n_exp)
    golem_face(d, smile=0.55)
    return img, {"n_exp": round(n_exp, 3)}


def render_texture(t, spots):
    img, d = canvas()
    golem_body(d, 2.2)
    golem_face(d, smile=0.55)
    k = int(len(spots) * (0.05 + 0.75 * t / 10.0))  # 5% -> 80% coverage
    paint_moss(img, spots, k, seed=41)
    return img, {"coverage": round(0.05 + 0.75 * t / 10.0, 3), "spots": k}


def render_expression(t):
    img, d = canvas()
    golem_body(d, 2.2)
    golem_face(d, smile=t / 10.0)  # stern -> beaming
    return img, {"smile": round(t / 10.0, 3)}


def render_color(t):
    # lantern glow: warm amber -> cool moonlight; bg tint follows slightly
    warm = (255, 170, 60)
    cool = (140, 180, 255)
    f = t / 10.0
    glow = tuple(int(w + (c - w) * f) for w, c in zip(warm, cool))
    tint = tuple(int(b + (g - b) * f * 0.35)
                 for b, g in zip(BG, (214, 222, 238)))
    img = Image.new("RGB", (SIZE, SIZE), tint)
    d = ImageDraw.Draw(img)
    halo = Image.new("RGB", (SIZE, SIZE), tint)
    hd = ImageDraw.Draw(halo)
    r = 150 - 40 * f
    hd.ellipse([256 - r, 210 - r, 256 + r, 210 + r], fill=glow)
    halo = halo.filter(ImageFilter.GaussianBlur(38))
    img = Image.blend(img, halo, 0.75)
    d = ImageDraw.Draw(img)
    # lantern body: round paper -> hex frame (shape echos, color leads)
    d.polygon(superellipse(256, 210, 96, 112, 2.0 + 4.0 * f), fill=(250, 244, 232),
              outline=(120, 90, 70))
    d.ellipse([236, 190, 276, 230], fill=glow)
    d.line([(256, 322), (256, 380)], fill=(120, 90, 70), width=6)
    d.ellipse([244, 380, 268, 404], fill=(120, 90, 70))
    d.rectangle([216, 84, 296, 100], fill=(90, 70, 55))
    return img, {"glow_rgb": list(glow), "warmth": round(1 - f, 3)}


RUST = [(154, 88, 44), (133, 72, 38), (172, 104, 54)]
MIRROR = (196, 214, 228)
MIRROR_DK = (140, 160, 178)


def rust_candidates(seed, m=700):
    rng = np.random.default_rng(seed)
    pts = []
    while len(pts) < m:
        batch = rng.uniform([176, 170], [336, 400], size=(m * 2, 2))
        for p in batch:
            pts.append((float(p[0]), float(p[1])))
            if len(pts) >= m:
                break
    return pts


def render_solar(t, spots):
    # rust-dominant -> mirror-dominant: rust pits shrink (nested), mirror
    # shoulder panels widen and gain shine lines (nested).
    f = t / 10.0
    img, d = canvas()
    d.polygon(superellipse(256, 285, 92, 128, 4.0), fill=(122, 70, 38))
    d.polygon(superellipse(256, 285, 80, 116, 4.0), fill=(154, 88, 44))
    rng = np.random.default_rng(77)
    radii = rng.uniform(2.0, 5.0, size=len(spots))
    k = int(len(spots) * (0.90 - 0.80 * f))  # rust recedes
    for (x, y), r in zip(spots[:k], radii[:k]):
        d.ellipse([x - r, y - r, x + r, y + r], fill=RUST[int(x + y) % 3])
    for s in (-1, 1):  # mirror pauldrons grow with f
        w = 8 + 44 * f
        x0 = 256 + s * 80
        d.ellipse([x0 - w, 168 - 26, x0 + w, 168 + 26], fill=MIRROR_DK)
        d.ellipse([x0 - w + 5, 168 - 20, x0 + w - 5, 168 + 20], fill=MIRROR)
        for i in range(int(1 + 3 * f)):  # nested shine lines
            off = -w + 12 + i * 16
            d.line([(x0 + off, 152), (x0 + off + 14, 184)], fill=(255, 255, 255), width=3)
    d.ellipse([232, 250, 280, 298], fill=(40, 60, 66))  # visor slit
    d.ellipse([240, 262, 272, 286], fill=(120, 220, 230))
    return img, {"rust_frac": round(0.90 - 0.80 * f, 3),
                 "mirror_w": round(8 + 44 * f, 1)}


def render_lure(t):
    # lure stalk sweeps wide side arc -> tall readable arc; helmet fixed.
    f = t / 10.0
    img, d = canvas()
    for s in (-1, 1):  # shoulders hint a diver body, fixed
        d.ellipse([256 + s * 90 - 34, 380 - 34, 256 + s * 90 + 34, 380 + 34],
                  fill=STONE_DK)
    d.ellipse([156, 150, 356, 350], fill=(96, 74, 52))    # brass helmet
    d.ellipse([168, 162, 344, 338], fill=(112, 88, 62))
    d.ellipse([206, 206, 306, 306], fill=(30, 42, 50))    # porthole
    d.ellipse([216, 216, 296, 296], fill=(56, 120, 132))
    d.ellipse([236, 236, 266, 266], fill=(150, 230, 235))
    x0, y0 = 256, 150
    cx = 256 - 150 + 150 * f       # control point sweeps in
    cy = 40 + 30 * f
    x1 = 150 + 110 * f             # bulb travels toward centre-top
    y1 = 96 - 30 * f
    pts = []
    for i in range(41):
        u = i / 40.0
        pts.append(((1 - u) ** 2 * x0 + 2 * (1 - u) * u * cx + u * u * x1,
                    (1 - u) ** 2 * y0 + 2 * (1 - u) * u * cy + u * u * y1))
    d.line(pts, fill=(70, 54, 38), width=8)
    halo = Image.new("RGB", (SIZE, SIZE), (0, 0, 0))
    hd = ImageDraw.Draw(halo)
    hd.ellipse([x1 - 44, y1 - 44, x1 + 44, y1 + 44], fill=(120, 230, 235))
    img = Image.composite(img, halo.filter(ImageFilter.GaussianBlur(20)),
                          halo.filter(ImageFilter.GaussianBlur(20)).convert("L").point(lambda v: v * 0.55 // 1))
    d = ImageDraw.Draw(img)
    d.ellipse([x1 - 16, y1 - 16, x1 + 16, y1 + 16], fill=(150, 235, 240))
    d.ellipse([x1 - 16, y1 - 16, x1 + 16, y1 + 16], outline=(70, 54, 38), width=3)
    return img, {"bulb_x": round(x1, 1), "bulb_y": round(y1, 1)}


def rune_branches(seed, m=30):
    # fixed candidate ornament strokes on a 6-glyph tablet row (nested)
    rng = np.random.default_rng(seed)
    out = []
    for i in range(6):
        gx = 96 + i * 64
        out.append((gx, 200, gx, 320))  # stem, always drawn
        for _ in range(5):
            y = float(rng.uniform(215, 305))
            s = 1 if rng.uniform() < 0.5 else -1
            out.append((gx, y, gx + s * float(rng.uniform(10, 26)),
                        y - float(rng.uniform(4, 18))))
    return out


def render_runes(t, branches):
    # legible stems -> full ornament: extra branch strokes accrete (nested)
    img, d = canvas()
    d.rounded_rectangle([60, 160, 452, 360], radius=26, fill=(148, 138, 122),
                        outline=(110, 100, 86), width=8)
    k = 6 + int((len(branches) - 6) * t / 10.0)
    for (x0, y0, x1, y1) in branches[:k]:
        d.line([(x0, y0), (x1, y1)], fill=(60, 50, 40), width=6)
    return img, {"strokes": k}


CHAINS = [
    {"id": "golem-shape", "axis": "shape", "fn": "shape",
     "subject": "mossy stone golem guardian, centered three-quarter view",
     "criteria": "Which golem reads as the friendlier forest guardian? Judge silhouette only.",
     "reflect": "Body exponent rises each step: round boulder mass at 0, cut-slab guardian at 10. Watch whether corners add character or threat."},
    {"id": "lantern-color", "axis": "color", "fn": "color",
     "subject": "floating paper lantern at night, centered, plain background",
     "criteria": "Which lantern feels cozier? Judge glow color and warmth only.",
     "reflect": "Glow cools amber to moonlight each step while the room tint follows at 35%. Watch the step where cozy becomes cold."},
    {"id": "golem-texture", "axis": "texture", "fn": "texture",
     "subject": "mossy stone golem guardian, centered three-quarter view",
     "criteria": "Which golem has better moss integration? Judge moss coverage only.",
     "reflect": "Moss spots are nested: each step adds new tufts, none move. Coverage 5% to 80%. Watch the step where lush becomes overgrown."},
    {"id": "golem-expression", "axis": "expression", "fn": "expression",
     "subject": "mossy stone golem guardian face, centered three-quarter view",
     "criteria": "Which golem looks friendlier? Judge face and smile only.",
     "reflect": "Mouth opens flat to full smile, eyes widen and warm. Watch the step where calm becomes grin."},
    {"id": "solar-material", "axis": "material", "fn": "solar",
     "subject": "desert solar-scavenger robot torso, centered three-quarter view",
     "criteria": "Which robot reads more solar-scavenger? Judge rust vs mirror only.",
     "reflect": "Rust pits recede 90% to 10% while mirror pauldrons widen and gain shine. Watch the step where scavenger becomes showroom."},
    {"id": "abyss-lure", "axis": "shape", "fn": "lure",
     "subject": "deep-sea diver helmet with bioluminescent lure stalk, centered",
     "criteria": "Which lure reads better at thumbnail size? Judge stalk silhouette only.",
     "reflect": "Bulb travels from a wide side arc to a tall centre-top arc. Watch the step where the arc stops merging with the helmet."},
    {"id": "rune-legibility", "axis": "texture", "fn": "runes",
     "subject": "carved stone tablet row of six glyphs, centered front view",
     "criteria": "Which tablet is more legible? Judge glyph clarity vs ornament only.",
     "reflect": "Stems first, then ornament branches accrete to full carving. Watch the step where rich becomes busy."},
]

FN = {"shape": render_shape, "color": render_color,
      "expression": render_expression}


def step_prompt(chain, t, params):
    return (f"Procedural concept render, centered subject on plain studio "
            f"background, soft even lighting: {chain['subject']}; "
            f"iteration step {t}/10 along axis {chain['axis']} "
            f"({json.dumps(params, sort_keys=True)}).")


def main():
    spots = moss_candidates(seed=41)
    steps_rows, pairs = [], []
    for chain in CHAINS:
        cdir = os.path.join(ROOT, chain["id"])
        os.makedirs(cdir, exist_ok=True)
        meta = []
        for t in range(11):
            if chain["fn"] == "texture":
                img, params = render_texture(t, spots)
            else:
                img, params = FN[chain["fn"]](t)
            rel = f"out/spark_pairs/chains/{chain['id']}/step_{t:02d}.png"
            img.save(os.path.join(cdir, f"step_{t:02d}.png"))
            prompt = step_prompt(chain, t, params)
            note = (f"t={t}: {chain['reflect'].split('.')[0]}. "
                    f"Delta vs t-{t - 1}: +1/10 {chain['axis']}." if t else
                    f"t=0 baseline: no {chain['axis']} trait applied yet.")
            meta.append({"t": t, "render": rel, "prompt": prompt,
                         "params": params})
            steps_rows.append({"ts": TS, "agent": AGENT, "prompt": prompt,
                                "seed": 41, "params": {"chain": chain["id"],
                                                       "t": t, **params},
                                "render_path": rel, "model_path": None,
                                "note": note})
        seen = set()

        def add_pair(a, b, kind):
            key = (a, b)
            if key in seen:
                return
            seen.add(key)
            ma, mb = meta[a], meta[b]
            pairs.append({
                "id": f"{chain['id']}-{kind}-{a:02d}v{b:02d}",
                "chain": chain["id"], "axis": chain["axis"],
                "kind": kind, "step_a": a, "step_b": b,
                "a": {"render": ma["render"], "prompt": ma["prompt"],
                      "params": ma["params"]},
                "b": {"render": mb["render"], "prompt": mb["prompt"],
                      "params": mb["params"]},
                "criteria": chain["criteria"], "created_by": AGENT})

        for t in range(10):  # close neighbours
            add_pair(t, t + 1, "adjacent")
        for t in range(1, 11):  # full arc from baseline, incl. 0v10
            add_pair(0, t, "baseline")
    with open(os.path.join(ROOT, "steps.jsonl"), "w") as f:
        for r in steps_rows:
            f.write(json.dumps(r, sort_keys=True) + "\n")
    with open(os.path.join(ROOT, "pairs.jsonl"), "w") as f:
        for p in pairs:
            f.write(json.dumps(p, sort_keys=True) + "\n")
    with open(os.path.join(ROOT, "gallery.html"), "w") as f:
        f.write(GALLERY.replace("__PAIRS__", json.dumps(pairs))
                       .replace("__CHAINS__", json.dumps(CHAINS))
                       .replace("__TS__", TS))
    print(f"renders={len(steps_rows)} pairs={len(pairs)}")


GALLERY = """<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Spark Pairs Ballot __TS__</title>
<style>body{font-family:system-ui,sans-serif;max-width:1100px;margin:0 auto;
padding:16px;background:#f4f2ee;color:#222}img{width:100%;border-radius:8px;
background:#ddd}.grid{display:grid;grid-template-columns:1fr 1fr;gap:12px}
.card{background:#fff;border-radius:12px;padding:12px;margin:14px 0;
box-shadow:0 1px 4px #0002}.row{display:flex;gap:8px;margin-top:8px;flex-wrap:wrap}
button{font-size:15px;padding:8px 14px;border-radius:8px;border:1px solid #999;
background:#fff;cursor:pointer}button.picked{background:#222;color:#fff}
.strip{display:flex;gap:4px;overflow-x:auto;margin:8px 0}.strip img{width:90px}
.toolbar{position:sticky;top:0;background:#f4f2ee;padding:8px 0;display:flex;
gap:8px;align-items:center;flex-wrap:wrap}.bar{height:8px;background:#ddd;
border-radius:4px;flex:1;min-width:120px}.bar>i{display:block;height:100%;
background:#2a7;border-radius:4px}</style></head><body>
<h1>Spark Pairs Ballot</h1>
<p>Vote the close pairs. Keys: <b>1</b>=left, <b>2</b>=right, <b>0</b>=tie.
Votes persist in this browser; export any time.</p>
<div class="toolbar"><label>Chain <select id="f"><option value="">all</option>
</select></label><div class="bar"><i id="p" style="width:0%"></i></div>
<span id="c"></span><button id="e">Export votes</button></div>
<div id="strips"></div><div class="card"><h3>Loft scenes + contrast</h3>
<p>Same steps, authored in Loft (lint exit 0) for later GPU renders:</p>
<ul><li><a href="golem-shape/chain.loft">golem-shape/chain.loft</a> (shape)</li>
<li><a href="lantern-color/chain.loft">lantern-color/chain.loft</a> (color) + <a href="lantern-color/lantern_glow.glsl">lantern_glow.glsl</a> (GLSL mirror)</li>
<li><a href="golem-texture/chain.loft">golem-texture/chain.loft</a> (texture)</li>
<li><a href="golem-expression/chain.loft">golem-expression/chain.loft</a> (expression)</li></ul>
<p><a href="LOFT_VS_GLSL.md">Loft vs GLSL note</a> · <a href="pairs.jsonl">pairs.jsonl</a> · <a href="steps.jsonl">steps.jsonl</a></p></div>
<div id="ballot"></div>
<script>const PAIRS=__PAIRS__;const CHAINS=__CHAINS__;
const V=JSON.parse(localStorage.getItem("spark_votes")||"{}");
const sel=document.getElementById("f");
[...new Set(PAIRS.map(p=>p.chain))].forEach(c=>{const o=document.createElement(
"option");o.value=c;o.textContent=c;sel.appendChild(o)});
const st=document.getElementById("strips");
CHAINS.forEach(ch=>{const d=document.createElement("div");d.className="card";
d.innerHTML="<h3>"+ch.id+" <small>("+ch.axis+")</small></h3><p>"+ch.reflect+
"</p>";const s=document.createElement("div");s.className="strip";
for(let t=0;t<=10;t++){const im=document.createElement("img");
im.src=ch.id+"/step_"+String(t).padStart(2,"0")+".png";im.loading="lazy";
im.title="step "+t;s.appendChild(im)}d.appendChild(s);st.appendChild(d)});
function draw(){const f=sel.value;const list=PAIRS.filter(p=>!f||p.chain===f);
const done=list.filter(p=>V[p.id]).length;
document.getElementById("p").style.width=(100*done/Math.max(1,list.length))+"%";
document.getElementById("c").textContent=done+"/"+list.length+" voted";
const b=document.getElementById("ballot");b.innerHTML="";
list.forEach((p,i)=>{const d=document.createElement("div");d.className="card";
d.innerHTML="<h3>"+p.id+"</h3><p><i>"+p.criteria+"</i></p>"+
"<div class='grid'><div><img src='"+rel(p.a.render)+"'><p>A: step "+p.step_a+
"</p></div><div><img src='"+rel(p.b.render)+"'><p>B: step "+p.step_b+
"</p></div></div>";const r=document.createElement("div");r.className="row";
[["a","A (1)"],["b","B (2)"],["tie","Tie (0)"]].forEach(([v,l])=>{
const btn=document.createElement("button");btn.textContent=l;
if(V[p.id]===v)btn.className="picked";
btn.onclick=()=>{V[p.id]=v;localStorage.setItem("spark_votes",
JSON.stringify(V));draw()};r.appendChild(btn)});
d.appendChild(r);d.id="card-"+i;b.appendChild(d)});window._list=list}
sel.onchange=draw;document.getElementById("e").onclick=()=>{
const rows=Object.entries(V).map(([pair_id,choice])=>({pair_id,choice,
ts:new Date().toISOString()}));
const blob=new Blob([rows.map(r=>JSON.stringify(r)).join("\\n")],
{type:"application/jsonl"});
const a=document.createElement("a");a.href=URL.createObjectURL(blob);
a.download="spark_votes.jsonl";a.click()};
document.addEventListener("keydown",e=>{const vis=[...document.querySelectorAll(
".card button")];});
let idx=0;document.addEventListener("keydown",e=>{const list=window._list||[];
const cards=[...document.querySelectorAll("#ballot .card")];
const cur=cards.findIndex(c=>{const r=c.getBoundingClientRect();
return r.top<innerHeight&&r.bottom>0});idx=Math.max(0,cur);
if(!list.length)return;
const map={"1":"a","2":"b","0":"tie"};if(!(e.key in map))return;
const p=list[Math.min(idx,list.length-1)];V[p.id]=map[e.key];
localStorage.setItem("spark_votes",JSON.stringify(V));draw();
const nc=document.querySelectorAll("#ballot .card")[Math.min(idx+1,
list.length-1)];if(nc)nc.scrollIntoView({block:"nearest"})});
draw();</script></body></html>"""


if __name__ == "__main__":
    main()

