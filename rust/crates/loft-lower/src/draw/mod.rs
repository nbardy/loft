//! The drawables' emit (src/compile/context.ts `Drawable.draws`), one lane per family. Each turns its things' typed
//! decls (lane W's envelope) into the emission src/draw/* writes: shaders, draws, table declarations, in TS order.

/// Lane D2: src/draw/{field,march,trace,field_shadow,traced}.ts, src/map/{brick,nets}.ts.
pub mod field;
/// Lane D4: src/draw/{cast,light,shadows}.ts.
pub mod light;
/// Lane D5: src/draw/volume.ts, src/soft/*, src/spacetime/{smear,telegraph,wake}.ts.
pub mod soft;
/// Lane D3: src/draw/{mesh,copies,impostors,shader}.ts.
pub mod solid;
/// Lane D1: src/draw/{surface,painted,grid_detail,relief,ground,sky,ink}.ts.
pub mod surface;

use crate::input::Input;

/// The `probe.tape.<k>.<kind>` sections every draw lane generates from its things' decls (file name, text), in thing
/// order. A lane that has not landed generates none, so its kinds' sections are absent and fail in its scope.
pub fn tape(input: &Input) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for lane in [surface::tape, field::tape, solid::tape, light::tape, soft::tape] {
        out.extend(lane(input));
    }
    out.sort();
    out
}
