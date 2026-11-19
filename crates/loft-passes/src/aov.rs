/// A diagnostic replaces shade and present at compile time. The beauty path retains its shader identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Aov {
    #[default]
    Beauty,
    Albedo,
    Normal,
    Depth,
    Id,
    SdfSteps,
    LipschitzRatio,
    Overdraw,
}

impl std::str::FromStr for Aov {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "beauty" => Self::Beauty,
            "albedo" => Self::Albedo,
            "normal" => Self::Normal,
            "depth" => Self::Depth,
            "id" => Self::Id,
            "sdf_steps" => Self::SdfSteps,
            "lipschitz_ratio" => Self::LipschitzRatio,
            "overdraw" => Self::Overdraw,
            _ => return Err("expected beauty, albedo, normal, depth, id, sdf_steps, lipschitz_ratio or overdraw"),
        })
    }
}

pub(super) const TRACE_STEPS: &str = r"
fn loft_steps(t: f32) -> vec3<f32> {
  return vec3<f32>(max(2.0*t-1.0, 0.0), 1.0-abs(2.0*t-1.0), max(1.0-2.0*t, 0.0));
}";

pub(super) const LIPSCHITZ_RATIO: &str = r"
fn loft_ratio(t: f32) -> vec3<f32> {
  if t > 1.0 { return vec3<f32>(1.0, 0.0, 0.0); }
  return mix(vec3<f32>(0.0, 0.0, 0.35), vec3<f32>(0.0, 1.0, 0.0), smoothstep(0.3, 1.0, t));
}";

pub(super) const ID_HASH: &str = r"
fn loft_id(x: u32) -> vec3<f32> {
  var h = x ^ (x >> 16u);
  h = h * 0x7feb352du;
  h = (h ^ (h >> 15u)) * 0x846ca68bu;
  h = h ^ (h >> 16u);
  return (vec3<f32>(vec3<u32>(h, h >> 8u, h >> 16u) & vec3<u32>(255u)) + 32.0) / 287.0;
}";

pub(super) const OVERDRAW: &str = r"
@fragment fn fs_overdraw() -> @location(0) vec4<f32> {
  return vec4<f32>(vec3<f32>(0.0625), 0.0);
}";
