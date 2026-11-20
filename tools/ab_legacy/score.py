"""Summarize two locked benchmark runs and compare the lossless RGB frames."""

import hashlib
import json
import math
from pathlib import Path
import sys

from PIL import Image, ImageChops
import tiktoken


def read(path):
    return json.loads(path.read_text())


def legacy_profile(path):
    return json.loads(path.read_text().splitlines()[-1])["profile"]


def open_frame(path):
    with Image.open(path) as frame:
        assert frame.size == (1280, 800), (path, frame.size)
        return frame.convert("RGB")


def compare(old, new):
    delta = ImageChops.difference(old, new)
    histogram = delta.histogram()
    pixels = old.width * old.height
    total_error = sum((bin % 256) * hits for bin, hits in enumerate(histogram))
    mse = sum((bin % 256) ** 2 * hits for bin, hits in enumerate(histogram)) / (3 * pixels)
    red, green, blue = delta.split()
    max_channel = ImageChops.lighter(ImageChops.lighter(red, green), blue).histogram()
    return {
        "psnr_db": 10 * math.log10(255**2 / mse) if mse else None,
        "mae_8bit": total_error / (3 * pixels),
        "pixels_over_2": sum(max_channel[3:]) / pixels,
        "pixels_over_8": sum(max_channel[9:]) / pixels,
    }


def main(out, root):
    tokenizer = tiktoken.get_encoding("o200k_base")

    def tokens(*paths):
        return sum(len(tokenizer.encode_ordinary(path.read_text())) for path in paths)

    old_tokens = {
        "b": tokens(out / "capsules-matched.ts"),
        "a": tokens(out / "capsules-reduced.ts", root / "tools/ab_legacy/reduced.ts",
                    root / "tools/ab_legacy/reduced.wgsl"),
    }
    new_tokens = {
        "a": tokens(out / "fixture/engines/capsules.loft"),
        "b": tokens(out / "fixture/engines/capsules-matched.loft"),
    }

    results = {}
    for split in ("a", "b"):
        results[split] = {}
        sheet = Image.new("RGB", (2560, 1600))
        for row, count in enumerate((1, 20)):
            runs = []
            hashes = []
            for run in (1, 2):
                prefix = f"{split}-{count}-r{run}"
                old = legacy_profile(out / f"{prefix}-old-profile.log")
                old_load = legacy_profile(out / f"{prefix}-old-load.log")
                new = read(out / f"{prefix}-new-profile.json")
                new_load = read(out / f"{prefix}-new-load.json")
                old_frame = open_frame(out / f"{prefix}-old.png")
                new_frame = open_frame(out / f"{prefix}-new.png")
                assert old["frames"] == len(new["gpu_frame_ms"]) == 300
                assert old["size"] == [1280, 800]
                assert len(old_load["loads_ms"]) == len(new_load["device_loads_ms"]) == 2
                assert old["adapter"] in new["adapter"], (old["adapter"], new["adapter"])
                assert set(new["frame_time_bits"]) == {0}
                assert len(old["pass_ms"]) == 3
                commands = {frame["commands"] for frame in new["frame_stamps"]}
                assert len(commands) == 1, commands
                runs.append({
                    "legacy": {
                        "p95_ms": old["gpu_ms"][2],
                        "load_ms": old_load["loads_ms"],
                        "modules": old_load["modules"],
                        "pipelines": old_load["render_pipelines"] + old_load["compute_pipelines"],
                        # One draw per ordered pass and two named-camera dispatches.
                        "commands": len(old["pass_ms"]) + 2 + old["frame_kernel_dispatches"],
                        "source_tokens": old_tokens[split],
                    },
                    "native": {
                        "p95_ms": sorted(new["gpu_frame_ms"])[round(0.95 * 299)],
                        "load_ms": new_load["device_loads_ms"],
                        "modules": len(new_load["modules"]),
                        "pipelines": new_load["pipelines"],
                        "commands": commands.pop(),
                        "source_tokens": new_tokens[split],
                        "triangle_p50_ms": new["control_p50_ms"],
                    },
                    "pictures": compare(old_frame, new_frame),
                })
                hashes.append([hashlib.sha256(frame.tobytes()).hexdigest()
                               for frame in (old_frame, new_frame)])
                if run == 1:
                    sheet.paste(old_frame, (0, row * 800))
                    sheet.paste(new_frame, (1280, row * 800))
            results[split][count] = {"runs": runs, "rgb_sha256": hashes}
        sheet.save(out / f"{split}-sheet.png")
    (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main(*map(Path, sys.argv[1:]))
