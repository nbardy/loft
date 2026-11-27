"""Count camera pixel centres admitted by the original and certified World boxes."""
# /// script
# dependencies = ["numpy"]
# ///
import json
import re
import sys
from pathlib import Path

import numpy as np
from fresh import scenes
from fresh_bounds import interval_records


def coverage(folder, width=960, height=720):
    yy, xx = np.mgrid[:height, :width]
    results = {}
    for scene, source in scenes.items():
        ir = folder / ("check-" + Path(source).name + ".ir")
        camera = ir.read_text().rsplit(" (camera (at . #(", 1)[1].split(" (sun ", 1)[0]
        target = np.fromstring(camera.split(")")[0], sep=" ")
        radius, height_above, period, fov = [float(re.search(r"\(" + k + r" \. ([^)]+)", camera)[1])
                                            for k in ("radius", "height", "period", "fov")]
        scale = 2 * np.tan(fov / 2) / height
        for frame in (0, 60, 119):
            angle = 2 * np.pi * frame / (60 * period)
            eye = target + [radius * np.sin(angle), height_above, radius * np.cos(angle)]
            forward = target - eye
            forward /= np.linalg.norm(forward)
            right = np.cross(forward, [0., 1., 0.])
            right /= np.linalg.norm(right)
            up = np.cross(right, forward)
            rays = (forward + right * ((xx + .5 - width / 2) * scale)[..., None]
                    - up * ((yy + .5 - height / 2) * scale)[..., None])
            for kind, rows in interval_records(ir).items():
                if kind not in {"Pelt", "Otter", "Hide", "Cap", "HeronHead", "Head4", "Torso"}:
                    continue
                counts = []
                for box in [rows[0]["outer"], rows[0]["inner"]]:
                    a = (np.array(box[:3]) - eye) / rays
                    b = (np.array(box[3:]) - eye) / rays
                    near = np.maximum(0., np.minimum(a, b).max(axis=2))
                    far = np.maximum(a, b).min(axis=2)
                    counts.append(int((near <= far).sum()))
                results.setdefault(scene + "/" + kind, {})[frame] = counts
    return results


if __name__ == "__main__":
    print(json.dumps(coverage(Path(sys.argv[1])), indent=2))
