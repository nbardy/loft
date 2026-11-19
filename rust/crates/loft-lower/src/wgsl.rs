//! Lane LIB. The shared WGSL library: each constant includes the same source the TS consumer imports.

pub const PRELUDE_WGSL: &str = include_str!("../../../../src/wgsl/prelude.wgsl");
pub const SCENE_DEPTH_WGSL: &str = include_str!("../../../../src/wgsl/scene_depth.wgsl");
pub const SCENE_CLEARANCE_WGSL: &str = include_str!("../../../../src/wgsl/scene_clearance.wgsl");
// The post suffix begins with the original separator newline after SCENE_DEPTH_WGSL.
pub const POST_WGSL: &str =
    concat!(include_str!("../../../../src/wgsl/scene_depth.wgsl"), include_str!("../../../../src/wgsl/post.wgsl"),);

pub const CORE_WGSL: &str = include_str!("../../../../src/wgsl/core.wgsl");
pub const MARK_WGSL: &str = include_str!("../../../../src/wgsl/mark.wgsl");
pub const INK_POST: &str = include_str!("../../../../src/wgsl/ink_post.wgsl");
pub const DEPTH_RESOLVE_WGSL: &str = include_str!("../../../../src/wgsl/depth_resolve.wgsl");
pub const CLEARANCE_WGSL: &str = include_str!("../../../../src/wgsl/clearance.wgsl");
pub const POST_COMMON: &str = include_str!("../../../../src/wgsl/post_common.wgsl");
pub const GAUSS_WGSL: &str = include_str!("../../../../src/wgsl/gauss.wgsl");
pub const BANK_SPLIT_WGSL: &str = include_str!("../../../../src/wgsl/bank_split.wgsl");
pub const CAMERA_RAY: &str = include_str!("../../../../src/wgsl/camera_ray.wgsl");

pub const LOFT_PCG: &str = include_str!("../../../../src/wgsl/loft_pcg.wgsl");
pub const LOFT_UNIT: &str = include_str!("../../../../src/wgsl/loft_unit.wgsl");
pub const LOFT_LAT: &str = include_str!("../../../../src/wgsl/loft_lat.wgsl");
pub const LOFT_H2: &str = include_str!("../../../../src/wgsl/loft_h2.wgsl");
pub const LOFT_H3: &str = include_str!("../../../../src/wgsl/loft_h3.wgsl");
pub const LOFT_GRAD2: &str = include_str!("../../../../src/wgsl/loft_grad2.wgsl");
pub const LOFT_GRAD3: &str = include_str!("../../../../src/wgsl/loft_grad3.wgsl");
pub const LOFT_CORNER3: &str = include_str!("../../../../src/wgsl/loft_corner3.wgsl");
pub const LOFT_HASH2: &str = include_str!("../../../../src/wgsl/loft_hash2.wgsl");
pub const LOFT_HASH3: &str = include_str!("../../../../src/wgsl/loft_hash3.wgsl");
pub const LOFT_HASH22: &str = include_str!("../../../../src/wgsl/loft_hash22.wgsl");
pub const LOFT_HASH33: &str = include_str!("../../../../src/wgsl/loft_hash33.wgsl");
pub const LOFT_NOISE2: &str = include_str!("../../../../src/wgsl/loft_noise2.wgsl");
pub const LOFT_NOISE3: &str = include_str!("../../../../src/wgsl/loft_noise3.wgsl");
pub const LOFT_CELLULAR2: &str = include_str!("../../../../src/wgsl/loft_cellular2.wgsl");
pub const LOFT_CELLULAR3: &str = include_str!("../../../../src/wgsl/loft_cellular3.wgsl");

pub const LOFT_TOWARD_EYE: &str = include_str!("../../../../src/wgsl/loft_toward_eye.wgsl");
pub const LOFT_LONG_DIR: &str = include_str!("../../../../src/wgsl/loft_long_dir.wgsl");
pub const LOFT_SCREEN_RATE: &str = include_str!("../../../../src/wgsl/loft_screen_rate.wgsl");
pub const PC_FACET: &str = include_str!("../../../../src/wgsl/pc_facet.wgsl");
pub const LOFT_SLOPE_ON_WATER: &str = include_str!("../../../../src/wgsl/loft_slope_on_water.wgsl");

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, path::Path, process::Command};

    #[test]
    fn shared_text_matches_ts_exports() {
        // Read the values the TS compiler uses so a miswired import or a change in composition is visible.
        let output = Command::new("bun")
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
            .args(["-e", "import { texts } from './src/wgsl/ts_exports.ts'; console.log(JSON.stringify(texts));"])
            .output()
            .expect("Bun is required to read TypeScript's shared WGSL");
        assert!(
            output.status.success(),
            "Bun could not print the TS WGSL: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: BTreeMap<String, String> = serde_json::from_slice(&output.stdout).expect("TS WGSL as JSON");
        let expected = [
            ("PRELUDE_WGSL", PRELUDE_WGSL),
            ("SCENE_DEPTH_WGSL", SCENE_DEPTH_WGSL),
            ("SCENE_CLEARANCE_WGSL", SCENE_CLEARANCE_WGSL),
            ("POST_WGSL", POST_WGSL),
            ("CORE_WGSL", CORE_WGSL),
            ("MARK_WGSL", MARK_WGSL),
            ("INK_POST", INK_POST),
            ("DEPTH_RESOLVE_WGSL", DEPTH_RESOLVE_WGSL),
            ("CLEARANCE_WGSL", CLEARANCE_WGSL),
            ("POST_COMMON", POST_COMMON),
            ("GAUSS_WGSL", GAUSS_WGSL),
            ("GAUSS_LIB", GAUSS_WGSL),
            ("BANK_SPLIT_WGSL", BANK_SPLIT_WGSL),
            ("CAMERA_RAY", CAMERA_RAY),
            ("LOFT_PCG", LOFT_PCG),
            ("LOFT_UNIT", LOFT_UNIT),
            ("LOFT_LAT", LOFT_LAT),
            ("LOFT_H2", LOFT_H2),
            ("LOFT_H3", LOFT_H3),
            ("LOFT_GRAD2", LOFT_GRAD2),
            ("LOFT_GRAD3", LOFT_GRAD3),
            ("LOFT_CORNER3", LOFT_CORNER3),
            ("LOFT_HASH2", LOFT_HASH2),
            ("LOFT_HASH3", LOFT_HASH3),
            ("LOFT_HASH22", LOFT_HASH22),
            ("LOFT_HASH33", LOFT_HASH33),
            ("LOFT_NOISE2", LOFT_NOISE2),
            ("LOFT_NOISE3", LOFT_NOISE3),
            ("LOFT_CELLULAR2", LOFT_CELLULAR2),
            ("LOFT_CELLULAR3", LOFT_CELLULAR3),
            ("LOFT_TOWARD_EYE", LOFT_TOWARD_EYE),
            ("LOFT_LONG_DIR", LOFT_LONG_DIR),
            ("LOFT_SCREEN_RATE", LOFT_SCREEN_RATE),
            ("PC_FACET", PC_FACET),
            ("LOFT_SLOPE_ON_WATER", LOFT_SLOPE_ON_WATER),
        ];
        for (name, text) in expected {
            assert_eq!(actual.get(name).map(String::as_str), Some(text), "{name}");
        }
        assert_eq!(actual.len(), expected.len(), "TS WGSL export count");
    }
}
