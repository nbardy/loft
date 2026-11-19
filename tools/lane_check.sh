#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
exec uv run --no-project python - "$@" <<'PY'
import re
import subprocess
import sys
import tomllib
from pathlib import PurePosixPath as Path
from os import environ

def git(*args):
    return subprocess.check_output(["git", *args])

if len(sys.argv)<3:
    sys.exit("usage: lane_check.sh BRANCH 'allowed/path/**' ...")
branch=git("rev-parse","--verify","--end-of-options",sys.argv[1]+"^{commit}").decode().strip()
base=git("merge-base",environ.get("LOFT_BASE","origin/main"),branch).decode().strip()
def changed(*specs):
    return {s.decode() for s in git("diff","--no-renames","--name-only","-z",base,branch,"--",*specs).split(b"\0") if s}
bad=changed(*[":(top,glob,exclude)"+glob for glob in sys.argv[2:]])
def blob(rev,path):
    obj=rev+":"+path
    if subprocess.run(["git","cat-file","-e",obj],stderr=subprocess.DEVNULL).returncode: return b""
    return git("show",obj)
def depnames(obj):
    names=set()
    for key,value in obj.items():
        if key in ("dependencies","build-dependencies","dev-dependencies"):
            names.update(v.get("package",k) if isinstance(v,dict) else k for k,v in value.items())
        elif isinstance(value,dict): names.update(depnames(value))
    return names
for path in changed():
    p=Path(path)
    if path.startswith("crates/loft-abi/") or re.search(r"(?:^|/)tests/golden",path): bad.add(path)
    if p.name=="Cargo.toml":
        a,b=(tomllib.loads(blob(ref,path).decode()) for ref in (base,branch))
        if (b.get("package",{}).get("name") not in (None,a.get("package",{}).get("name"))
            or set(b.get("workspace",{}).get("members",[]))-set(a.get("workspace",{}).get("members",[]))
            or depnames(b)-depnames(a)): bad.add(path)
    if p.suffix==".rs":
        # Compare complete attributes so a multiline #[ignore = "..."] cannot evade a diff-line scan.
        attr=rb'#\s*\[\s*(?:ignore\b|cfg_attr\s*\([^\]]*,\s*ignore\b)[^\]]*\]'
        from collections import Counter
        before,after=(Counter(re.findall(attr,blob(ref,path))) for ref in (base,branch))
        if after-before: bad.add(path)
if bad:
    sys.exit("lane exceeds its ownership or changes a protected contract:\n"+"\n".join(sorted(bad)))
PY
