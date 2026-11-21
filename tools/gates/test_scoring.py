# /// script
# dependencies = ["numpy", "pillow"]
# ///
"""Focused controls for the strict picture boundary, estimator and paired timing."""
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

import pictures
import timing


class Scoring(unittest.TestCase):
    def test_picture_mutants(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            baseline, candidate = root / "main", root / "candidate"
            baseline.mkdir()
            candidate.mkdir()
            size = (100, 30)
            clean = np.full((30, 100, 4), 100, dtype=np.uint8)
            approved = root / "approved.png"
            Image.fromarray(clean).save(approved)
            mode = pictures.Deterministic(size, pictures.rgba_hash(clean), approved,
                                          baseline, candidate, "test")
            plane = b"\x00" * 12000
            for folder in (baseline, candidate):
                Image.fromarray(clean).save(folder / "test.png")
                for suffix in ("id.u32", "depth.f32"):
                    (folder / f"test.{suffix}").write_bytes(plane)
            cases = {}
            for label, count, delta, held in (("exact", 0, 0, True), ("inside", 2, 1, True),
                                               ("density", 3, 1, False), ("amplitude", 1, 2, False)):
                frame = clean.copy()
                frame[0, :count, 0] += delta
                Image.fromarray(frame).save(candidate / "test.png")
                result = pictures.score(mode)
                self.assertEqual(result["held"], held)
                cases[label] = result["comparisons"]["main"]
            Image.fromarray(clean).save(candidate / "test.png")
            for suffix in ("id.u32", "depth.f32"):
                file = candidate / f"test.{suffix}"
                file.write_bytes(b"\x01" + plane[1:])
                self.assertFalse(pictures.score(mode)["held"])
                file.write_bytes(plane)
            print("Pictures", json.dumps(cases, sort_keys=True))

    def test_temporal_mutant(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            paths = []
            for index, color in enumerate((128, 144, 112, 144, 112)):
                path = root / f"{index}.png"
                Image.new("RGBA", (4, 4), (color, color, color, 255)).save(path)
                paths.append(path)
            ref, *frames = paths
            cfg = {"size": [4, 4], "min_reference_psnr_db": 35,
                   "min_psnr_db": 23, "max_temporal_variance": 0.0025}
            result = pictures.score(pictures.Stochastic(cfg, ref, ref, tuple(frames)))
            self.assertTrue(all(db >= 23 for db in result["frame_psnr_db"]))
            self.assertFalse(result["held"])
            print("Temporal mutant", json.dumps(result, sort_keys=True))

    def test_paired_controls(self):
        def report(ms, cold, warm, triangle):
            return {"adapter": "fixture", "preflight_control_p95_ms": triangle,
                    "scene": {"speed": {"control_p95_ms": triangle, "p95_ms": ms},
                              "load": {"cold_s": cold, "warm_s": warm}}}

        baseline = [report(0.5 + offset / 100, 0.3, 0.01, 0.1) for offset in range(6)]
        candidate = [report(0.49 + offset / 100, 0.6, 0.01, 0.1) for offset in range(6)]
        budgets = {"scene": {"p95_ms": 0.8, "cold_s": 0.5, "warm_s": 0.2}}
        pairs = list(zip(baseline, candidate))
        result = timing.score(pairs, ["scene"], budgets, 0.05)["scenes"]["scene"]
        metrics = result["metrics"]
        self.assertEqual([row["verdict"] for row in metrics.values()], ["held", "missed", "held"])
        self.assertAlmostEqual(metrics["p95_ms"]["paired_delta_ci95"][0], -0.01)
        self.assertAlmostEqual(metrics["p95_ms"]["paired_delta_ci95"][1], -0.01)
        candidate[-1]["preflight_control_p95_ms"] = 0.150
        noisy = timing.score(pairs, ["scene"], budgets, 0.05)["scenes"]["scene"]
        self.assertTrue(all(row["verdict"] == "unknown" for row in noisy["metrics"].values()))
        with self.assertRaises(ValueError):
            timing.score(pairs[:-1], ["scene"], budgets, 0.05)
        print("Timing controls", json.dumps(metrics, sort_keys=True))
        print("Contended", json.dumps({key: row["verdict"] for key, row in noisy["metrics"].items()}))


if __name__ == "__main__":
    unittest.main()
