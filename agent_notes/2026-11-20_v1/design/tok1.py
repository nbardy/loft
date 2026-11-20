"""Read-only TOK1 census. Run at the workspace root with uv run --with tiktoken."""
import re
import subprocess
from collections import Counter, defaultdict
from functools import lru_cache
from pathlib import Path
from statistics import median

import tiktoken

ENC = tiktoken.get_encoding("o200k_base")


def count(text):
    return len(ENC.encode_ordinary(text))


# The two triples are the actual R0 clip windows. Preserve the stored widths.
STARTS = ["0", "0.25", "0.5", "0.75", "0.9375", "1.37"]
WIDTHS = ["0.25", "0.25", "0.25", "0.1875", "0.432500005", "1.52999997"]
YAW = """let d = p - VIEW_PIVOT
  let c = cos(VIEW_YAW)
  let s = sin(VIEW_YAW)
  place(VIEW_PIVOT + [c * d.x + s * d.z, d.y, -s * d.x + c * d.z], VIEW_YAW)"""
DAY = (r"emit Sun\(dir: dir\((\[[^\]\n]+\])@World\), color: (\[[^\]\n]+\])\)\n  "
       r"emit Sky\(zenith: (\[[^\]\n]+\]), horizon: (\[[^\]\n]+\])\)")
SEED_LIGHT = {
    "tidal_orbit_dir": "[-0.609761775, 0.76538378, 0.205859736]",
    "tidal_fixed_dir": "[-0.430528373, 0.76538378, 0.478364885]",
    "tidal_key": "[2.8, 2.6, 2.3]",
    "tidal_zenith": "[0.35, 0.56, 0.88]",
    "tidal_horizon": "[0.82, 0.86, 0.9]",
}


def rewrite(path, source, record):
    def score(name, old, new):
        record(name, count(old), count(new))
        return new

    if "/seeds/" in path or "/l5e/" in path:
        for offset, binding, arguments in [
            (0, "early", "time, [0, 0.25, 0.5], v3(0.25)"),
            (3, "late", "time, SEED_TAIL_STARTS, SEED_TAIL_WIDTHS"),
        ]:
            old = "\n    ".join(f"let t{i+1} = clamp((time - {STARTS[i]}s) / {WIDTHS[i]}s, 0, 1)"
                                for i in range(offset, offset + 3))
            if old not in source:
                continue
            call = f"let {binding} = ramp_weights({arguments})"
            hits = source.count(old)
            source = source.replace(old, call)
            for _ in range(hits):
                score("ramp_weights", old, call)
            for i, component in enumerate("xyz", offset + 1):
                source = re.sub(rf"\bt{i}\b", f"{binding}.{component}", source)
    def daylight(m):
        values = {v: k for k, v in SEED_LIGHT.items()}
        call = "daylight(" + ", ".join(values.get(arg, arg) for arg in m.groups()) + ")"
        return score("daylight", m[0], call)

    # The paired Shrine frame and its own controls varied. Keep that
    # optional use out of the rollout forecast until strict parity is scored.
    if path != "engines/coral_shrine.loft":
        source = re.sub(DAY, daylight, source)
    call = "place(yaw_about(p, VIEW_PIVOT, VIEW_YAW), VIEW_YAW)"
    if YAW in source:
        source = source.replace(YAW, score("yaw_about", YAW, call))
    return source


def row_defaults(source):
    """Exact modal literal per kind and column, charging the new declaration."""
    rows = re.compile(r"  emit (Sweep\w+)\(\n(.*?)^  \)\n", re.M | re.S)
    values = defaultdict(Counter)
    fields = re.compile(r"^    (paint|expo): ([^\n]+),\n", re.M)
    for row in rows.finditer(source):
        for key, value in fields.findall(row[2]):
            values[row[1], key][value] += 1
    for (kind, key), hist in values.items():
        val = hist.most_common(1)[0][0]
        prefix = rf"(kind {kind} @World \(\n  paint: rgb" if key == "paint" else rf"(kind {kind} @World \(\n  paint:[^\n]+?expo: f"
        source, hits = re.subn(prefix + ")", r"\g<1> = " + val, source)
        assert hits == 1
    def omit(row):
        body = fields.sub(lambda m: "" if values[row[1], m[1]].most_common(1)[0][0] == m[2] else m[0], row[2])
        return f"  emit {row[1]}(\n{body}  )\n"
    return rows.sub(omit, source)


SURFACES = {
    "camera defaults": lambda s: re.sub(r", period: 24(?:\.0)?s?(?=,)", "", s).replace(", height: 0.5, fov:", ", fov:"),
    "origin splat": lambda s: re.sub(r"\[0, 0, 0\](?=@\w+)", "[0]", s),
    "modal row defaults": row_defaults,
    "positional full rows": lambda s: re.sub(r"  emit Sweep\d+\(\n(?:    \w+: [^\n]+,\n){8}  \)", lambda row: re.sub(r"(?m)^    \w+: ", "    ", row[0]), s),
    "infer camera units": lambda s: re.sub(r"(?m)^ *(?:camera \w+ \{|orbit\().*?(?:\}|\)\n)", lambda m:
        re.sub(r"(\b(?:radius|height|period): -?[\d.e+-]+)(?:m|s)(?=\s*[,})])", r"\1", m[0]), s, flags=re.S),
}

# Assign each line to its enclosing declaration/member; generated row fields
# have their own categories. Type/space/unit spellings take precedence within it.
ANNOT = re.compile(r"@\w+(?:>\w+)?|(?<=\d)(?:mm|ms|deg|rad|kg|m|s)\b|"
                   r":\s*(?:p3|v[234]|xf|d3|n3|f|u|m|s|rgb|bool|rad|Bsdf)\b|"
                   r"->\s*\w+")
PARTS = [
    ("eyes", r"eye|iris|pupil|brow|stalk|glint"),
    ("legs", r"leg|toe|foot|femur|tibia|knee"),
    ("shells", r"shell|carapace|whorl"),
    ("other limbs", r"claw|arm|finger|palm|jaw|pincer|elbow|limb|antenna|feeler"),
]


def part(label):
    return next((name for name, pat in PARTS if re.search(pat, label, re.I)), "other")


def category(line, top, member, path):
    s = line.strip()
    if not s or s.startswith(("#", "//")):
        return "comments/blank lines"
    if re.match(r"(emit (Sun|Sky|PointLamp)\b|kind (Sun|Sky|PointLamp)\b|shade_tiles\()", s):
        return "lights/sky"
    if s.startswith("expo:") and top.startswith("build"):
        return "section exponent data"
    if s.startswith("paint:") and top.startswith("build"):
        return "material/palette"
    if re.match(r"(center|side|up)_[ab]_d\d+:", s) and top.startswith("build"):
        return "keyframe data"
    if re.match(r"(center|side|up)_[ab]:", s) and top.startswith("build"):
        return "ring data"
    if re.search(r"\b(?:mirror_[xyz]|sym\.(?:pt|in|out)|map (?:sym|side|ear)|select\(-1\.0, 1\.0, i)\b", s):
        return "symmetry/maps"
    if top.startswith(("schedule", "camera", "world", "space", "use ", "param ")):
        return "setup/schedules"
    if "material(" in member or top.startswith("lobe ") or path.endswith(("/bsdf.loft", "/woodblock.loft")) or "palette" in top:
        return "material/palette"
    if path.endswith(("/light.loft", "/point_lamp.loft", "/restir.loft", "/reference.loft", "/spectral_caustic.loft")):
        return "lights/transport"
    if (top.startswith(("law ", "check ", "solver ", "plan ")) or path.endswith(("/act.loft", "/roam.loft", "/motion.loft"))
            or re.match(r"let (?:t[1-6]|\w+_now) =", s) or re.search(r"\b(?:solve_\w+|ik_reachable_end)\(", s)):
        return "motion/solvers/checks"
    if top.startswith("build") or top.startswith("pub build"):
        return "build/row construction"
    if top.startswith("kind ") and not member:
        return "row/schema"
    # Only geometry gets assigned to anatomy here. A named subpart on the
    # line is more specific than its enclosing kind/helper.
    label = part(s)
    if label == "other":
        label = part(top.split("(")[0])
    if label != "other":
        return "body/" + label
    return "geometry/other code"


@lru_cache(maxsize=None)
def token_bytes(token):
    b = ENC.decode_single_token_bytes(token)
    return len(b), len(b) - len(b.lstrip()) if b.strip() else 0


def attribution(path, source, tokens, by_part):
    # First non-whitespace byte of each ORIGINAL token. Whitespace-only tokens
    # use their first byte. Byte masks keep multibyte comments and BPE joins exact.
    cats, parts = bytearray(), bytearray()
    labels, anatomy = [], []
    top = member = body = ""
    for line in source.splitlines(keepends=True):
        s = line.strip()
        if re.match(r"^(?:(?:pub |build )?fn|pub build fn|kind|law|check|schedule|space|use|param|build|bake_tex|scan|const|lobe|struct|kernel|camera|world)\b", line):
            top, member, body = s, "", ""
        if top.startswith("kind ") and re.match(r"^  (?:surface|trace|visible|material|map)\b", line):
            member = s
        if top.startswith("build") and line.startswith("  # "):
            body = s[2:]
        cat = category(line, top, member, path)
        if cat not in labels:
            labels.append(cat)
        raw = line.encode()
        mask = bytearray([labels.index(cat)]) * len(raw)
        if cat != "comments/blank lines":
            if "annotations/units" not in labels:
                labels.append("annotations/units")
            for match in ANNOT.finditer(line):
                a = len(line[:match.start()].encode())
                b = a + len(match[0].encode())
                mask[a:b] = bytes([labels.index("annotations/units")]) * (b - a)
        cats.extend(mask)
        tag = "other"
        if cat in ("ring data", "keyframe data"):
            tag = part(body)
        if tag not in anatomy:
            anatomy.append(tag)
        parts.extend(bytes([anatomy.index(tag)]) * len(raw))
    cnt = Counter()
    pos = 0
    for token in tokens:
        size, offset = token_bytes(token)
        cat = labels[cats[pos + offset]]
        cnt[cat] += 1
        if cat in ("ring data", "keyframe data"):
            by_part[anatomy[parts[pos + offset]], cat] += 1
        pos += size
    assert pos == len(cats) and sum(cnt.values()) == len(tokens)
    return cnt


def main():
    names = subprocess.check_output(["git", "ls-files", "--", "engines/*.loft", "scenes/*.loft", "lib/*.loft", "corpus/*.loft"]).decode().splitlines()
    constructs, anatomy, scores, files = defaultdict(Counter), Counter(), defaultdict(list), defaultdict(list)
    surfaces, site_files = Counter(), defaultdict(set)
    for name in names:
        path = Path(name)
        if path.is_symlink() or "/quarantine/" in name:
            continue
        scope = name.split("/")[0]
        src = path.read_text()
        tokens = ENC.encode_ordinary(src)
        before = len(tokens)
        constructs[scope].update(attribution(name, src, tokens, anatomy))
        def record(key, old, call):
            scores[key].append((old, call))
            site_files[key].add(name)
        std = rewrite(name, src, record)
        basic = std
        for key, transform in SURFACES.items():
            newer = transform(src)
            if newer != src:
                surfaces[key, scope] += before - count(newer)
            if key in ("camera defaults", "origin splat", "modal row defaults"):
                basic = transform(basic)
        stats = (before, count(std), count(basic))
        files[scope].append(stats)
        if scope == "corpus":
            files[name.split("/")[1]].append(stats)
        elif scope == "engines":
            print(name, stats)
    for group, rows in files.items():
        print(group, len(rows), "sums", list(map(sum, zip(*rows))), "medians", list(map(median, zip(*rows))))
    print("constructs", dict(constructs))
    print("ring fields by part", anatomy)
    for key, vals in sorted(scores.items(), key=lambda item: -sum(a-b for a,b in item[1])):
        print(key, len(site_files[key]), Counter(vals), "code savings", sum(a-b for a,b in vals))
    print("surface savings", surfaces)


if __name__ == "__main__":
    main()
