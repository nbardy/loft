"""Adapt only std1c sources in an isolated checkout of D with B applied.

D's emitter only declares SurfaceHit and LightSummary. Specialize the two
frozen records to numeric arrays in the test copy while leaving the style
arithmetic and the B/D implementations intact.
"""
import re
import sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
if not (root / "stdlib/lighting/surface.loft").is_file():
    raise SystemExit("Expected a D checkout with its published lighting/surface.loft")
if not (root / "stdlib/noise/cellular.loft").is_file():
    raise SystemExit("Expected the B stdlib overlay")

styles = root / "stdlib/styles"
scene = root / "scenes/showcase/std1c"
contract = styles / "surface.loft"
expected = {
    "Surface": ("p", "anchor", "n", "view", "uv", "footprint", "pigment", "rough", "kind_id", "row_id"),
    "Lighting": ("key", "key_visibility", "ao", "direct", "indirect_diffuse", "indirect_gloss", "emitted"),
}
for name, fields in expected.items():
    match = re.search(rf"pub struct {name} \{{([^}}]+)\}}", contract.read_text())
    if match is None or tuple(re.findall(r"(\w+):", match[1])) != fields:
        raise SystemExit(f"Unexpected frozen {name} fields. Apply only to a fresh std1c source copy.")

surface = dict(zip(expected["Surface"], (
    "[0].xyz", "[1].xyz", "[2].xyz", "[3].xyz", "[4].xy", "[4].zw",
    "[5].xyz", "[5].w", "[6].x", "[6].y",
)))
lighting = dict(zip(expected["Lighting"], (
    "[0].xyz", "[0].w", "[1].w", "[1].xyz", "[2].xyz", "[3].xyz", "[4].xyz",
)))
contract.write_text("pub type Surface = array[v4, 7]\npub type Lighting = array[v4, 5]\n")
constructors = """
fn surface_record(p: v3, a: v3, n: v3, v: v3, uv: v2, px: v2, c: rgb, r: f, k: u, i: u) -> Surface =
  wgsl(p: v3, a: v3, n: v3, v: v3, uv: v2, px: v2, c: rgb, r: f, k: u, i: u) -> Surface {
    return array(vec4f(p, 0.0), vec4f(a, 0.0), vec4f(n, 0.0), vec4f(v, 0.0),
      vec4f(uv, px), vec4f(c, r), vec4f(bitcast<f32>(k), bitcast<f32>(i), 0.0, 0.0));
  }

fn lighting_record(k: v3, vis: f, ao: f, direct: rgb, diffuse: rgb, gloss: rgb, emitted: rgb) -> Lighting =
  wgsl(k: v3, vis: f, ao: f, direct: rgb, diffuse: rgb, gloss: rgb, emitted: rgb) -> Lighting {
    return array(vec4f(k, vis), vec4f(direct, ao), vec4f(diffuse, 0.0), vec4f(gloss, 0.0), vec4f(emitted, 0.0));
  }
"""
for path in [*styles.glob("*.loft"), styles / "print/paint.loft", *scene.glob("*.loft")]:
    text = path.read_text()
    if path == scene / "bridge.loft":
        text = text.replace("let n = s.n;", "let n = s[2].xyz;").replace("s.view", "s[3].xyz")
        text = text.replace("s.pigment", "s[5].xyz").replace("s.rough", "s[5].w")
    elif path == scene / "papercraft_render.loft":
        text = text.replace("replace_normal(h, s.n)", "replace_normal(h, s[2].xyz)").replace("n: n3@World", "n: v3")
    elif path == scene / "fixed_records.loft":
        text = text.replace("coords(facet.n)", "facet[2].xyz")
    elif path.is_relative_to(styles):
        text = text.replace("folded.n = normalize(mix(h.n, normalize(flat + 0.24 * tangent), filtered));",
                            "folded[2] = vec4f(normalize(mix(h.n, normalize(flat + 0.24 * tangent), filtered)), 0.0);")
        text = re.sub(r"coords\(h\.(anchor|p|n|view)\)", r"h.\1", text)
        for field, select in surface.items():
            text = re.sub(rf"\bh\.{field}\b", "h" + select, text)
        for field, select in lighting.items():
            text = re.sub(rf"\bl\.{field}\b", "l" + select, text)
    if "loft_type_Surface(" in text or "loft_type_Lighting(" in text:
        text = text.replace("loft_type_Surface(", "loft_user_surface_record(")
        text = text.replace("loft_type_Lighting(", "loft_user_lighting_record(")
        text += constructors
    path.write_text(text)
