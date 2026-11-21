use loft_abi::VerdictReason;

pub fn reason(name: &str) -> Option<VerdictReason> {
    Some(match name {
        "IterCap" => VerdictReason::IterCap,
        "NonFinite" => VerdictReason::NonFinite,
        "MissingSurface" => VerdictReason::MissingSurface,
        "MissingFrame" => VerdictReason::MissingFrame,
        "SolveFailed" => VerdictReason::SolveFailed,
        _ => return None,
    })
}
