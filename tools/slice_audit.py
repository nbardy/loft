"""Score the two source checks. Run with uv run --no-project --with tiktoken tools/slice_audit.py."""
from pathlib import Path
import json
import re
import subprocess

import tiktoken

base = "5d94531"
codec = tiktoken.get_encoding("o200k_base")
def excerpt(path, start, end=None):
    body = Path(path).read_text()
    at = body.index(start)
    return body[at:body.index(end, at) if end else None].strip()

# Imports, fixtures and blank space outside the edited excerpt are excluded on both sides.
cases = {
    "2": (263, [excerpt("src/kinds/shapes.ts", "export const washer", "const sphereKind"),
                excerpt("examples/slice/washer.ts", "export const ring", "export const card")]),
    "3": (231, [excerpt("examples/slice/looks.ts", "const shell", "const things")]),
    "4 shade": (171, [excerpt("src/engines/print.ts", "export const toon", "export const print"),
                      excerpt("examples/slice/looks.ts", "export default scene")]),
    "4 world": (69, [excerpt("examples/slice/looks.ts", "export const shadows", "const cameras")]),
    "5": (377, [excerpt("src/engines/reference.ts", "export const jittered"), Path("src/light/transport.wgsl").read_text().strip()]),
    "6": (107, [excerpt("src/engines/inspect.ts", "export const inspect")]),
}
counts = {}
for name, (prediction, blocks) in cases.items():
    measured = len(codec.encode("\n\n".join(blocks)))
    counts[name] = dict(predicted=prediction, measured=measured, passed=abs(measured/prediction-1)<=0.15)

paths = ["rust/crates/loft-ir/src", "rust/crates/loft-core/src", "rust/crates/loft-gpu/src", "src/record", "src/text"]
added = subprocess.check_output(["git","diff",base,"--unified=0","--",*paths],text=True)
vocab = set("shape material albedo brdf bsdf shadow light sun sky ray hit trace visible surface normal ink washer bead lamp scatter ggx emit emitter bounce occlusion medium radiance toon sphere capsule furnace".split())
hits = set()
for line in added.splitlines():
    if not line.startswith("+") or line.startswith("+++"):
        continue
    code = re.sub(r'"(?:\\.|[^"\\])*"', '""', line[1:].split("//",1)[0])
    for word in re.findall(r'[A-Za-z_][A-Za-z0-9_]*', code):
        parts = re.sub(r'([a-z])([A-Z])',r'\1_\2',word).lower().split('_')
        if vocab.intersection(parts):
            hits.add(word)
# Decl.Medium is repeated on the existing wire enum line extended by Kind and Open.
# crate::emit is the existing graph WGSL emitter used to share table accessors.
# Existing Resource.shape and Shape describe buffer/texture layout. The other two names belong to naga/wgpu.
inherited = {"emit","Medium","Shape","shape","Emit","emit_to_string","occlusion_query_set"}
print(json.dumps({"16":{"vocabulary":sorted(vocab),"hits":sorted(hits),"domain_identifiers":sorted(hits-inherited),"passed":not (hits-inherited)},
                  "17":counts},indent=2))
