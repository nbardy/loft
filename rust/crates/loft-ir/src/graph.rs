//! The graph arena a job decodes into (plan §1.3, wire §3.2): typed nodes in recording order, so every operand precedes
//! its user, and the declarations their ops name, one array per kind. Built only by `wire::decode`, which validates
//! every bound, reference, layout and type rule; consumers read the public fields and never re-check them.
use serde::{Deserialize, Serialize};

/// A value's type. Wire code = the order here (src/record/wire.ts `TYS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ty {
    F32,
    V2,
    V3,
    V4,
    Bool,
}

impl Ty {
    pub const ALL: [Ty; 5] = [Ty::F32, Ty::V2, Ty::V3, Ty::V4, Ty::Bool];
    /// Components of a number type; None for `Bool`.
    pub fn width(self) -> Option<u8> {
        match self {
            Ty::F32 => Some(1),
            Ty::V2 => Some(2),
            Ty::V3 => Some(3),
            Ty::V4 => Some(4),
            Ty::Bool => None,
        }
    }
    /// The literal entries a value of this type takes in `Graph::lits` (a bool is one, 0 or 1).
    pub fn lits(self) -> u8 {
        self.width().unwrap_or(1)
    }
    /// The recorder's spelling (`src/expr.ts` `Ty`), which the canonical print uses.
    pub fn name(self) -> &'static str {
        match self {
            Ty::F32 => "f32",
            Ty::V2 => "vec2",
            Ty::V3 => "vec3",
            Ty::V4 => "vec4",
            Ty::Bool => "bool",
        }
    }
    pub(crate) fn of_width(w: u8) -> Option<Ty> {
        Ty::ALL.get(usize::from(w).checked_sub(1)?).copied().filter(|t| *t != Ty::Bool)
    }
}

/// A swizzle selection: `len` components (1..=4), component k in bits 2k..2k+2 of `comps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sel {
    pub len: u8,
    pub comps: u8,
}

/// What a node computes: one variant per wire op (src/record/wire.ts `Op`; codes in `wire`). Ids name the decl arrays
/// of `Graph` (`fns`, `raws`, `loops`, `kernels`, `params`, `inputs`) or a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    /// The first of `ty.lits()` entries of `Graph::lits` (exact f64 bits as authored).
    Lit(u32),
    Time,
    Param(u32),
    Input(u32),
    /// A binder of a loop or a graph function, named by symbol `.0`; read only inside its owner.
    Bind(u32),
    /// A free variable named by symbol `.0` (u, v, p), bound by the job's consumer.
    Var(u32),
    /// A graph function called with the node's args; inlined during interning.
    Call(u32),
    /// vec4 lane `lane` of the row the f32 arg names, of a kernel's resource.
    Fetch {
        kernel: u32,
        lane: u16,
    },
    /// Result component `k` of a loop.
    Loop {
        decl: u32,
        k: u16,
    },
    /// d args[0] / d args[1]: a request, rewritten to its derivative graph during interning. The wave-1 recorder writes
    /// derivatives already expanded, so no job carries one yet.
    Grad,
    /// A raw WGSL function called with the node's args. GPU only.
    Raw(u32),
    Swizzle(Sel),
    /// A vector from 1..=4 parts whose widths sum to the node's (or one f32, splatted).
    Vec,
    Prim(Prim),
}

macro_rules! prims {
    ($($p:ident $name:literal $rule:ident),* $(,)?) => {
        /// The operators, in the order of `src/expr.ts` `Fn` (wire code 16 + index), the order of loft-core's `Prim`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Prim { $($p),* }
        impl Prim {
            pub const ALL: &[Prim] = &[$(Prim::$p),*];
            /// The recorder's name, as `src/expr.ts` spells it (`Fn`), which the canonical print uses.
            pub fn name(self) -> &'static str { match self { $(Prim::$p => $name),* } }
            fn rule(self) -> Rule { match self { $(Prim::$p => Rule::$rule),* } }
        }
    };
}
prims! {
    Add "add" Map2, Sub "sub" Map2, Mul "mul" Map2, Div "div" Map2, Neg "neg" Map1, Min "min" Map2, Max "max" Map2,
    Pow "pow" Map2, Mod "mod" Map2, Atan2 "atan2" Map2, Step "step" Map2,
    Abs "abs" Map1, Sign "sign" Map1, Floor "floor" Map1, Fract "fract" Map1, Sqrt "sqrt" Map1, Exp "exp" Map1,
    Log "log" Map1, Sin "sin" Map1, Cos "cos" Map1, Tan "tan" Map1, Saturate "saturate" Map1,
    Normalize "normalize" Map1,
    Mix "mix" Map3, Clamp "clamp" Map3, Smoothstep "smoothstep" Map3, Length "length" Norm,
    Distance "distance" Dot, Dot "dot" Dot, Cross "cross" Cross,
    Lt "lt" Cmp, Gt "gt" Cmp, Le "le" Cmp, Ge "ge" Cmp, Eq "eq" Cmp, Ne "ne" Cmp,
    And "and" Logic, Or "or" Logic, Not "not" Not, Select "select" Select,
    Ceil "ceil" Map1, Trunc "trunc" Map1, RoundEven "roundEven" Map1, Rem "rem" Map2, InverseSqrt "inverseSqrt" Map1,
    Exp2 "exp2" Map1, Log2 "log2" Map1, Asin "asin" Map1, Acos "acos" Map1, Atan "atan" Map1, Sinh "sinh" Map1,
    Cosh "cosh" Map1, Tanh "tanh" Map1,
}

enum Rule {
    Map1,
    Map2,
    Map3,
    Norm,
    Dot,
    Cross,
    Cmp,
    Logic,
    Not,
    Select,
}

/// Component-wise result of two number types: equal, or a scalar splats to the other's width.
fn wider(a: Ty, b: Ty) -> Result<Ty, (Ty, Ty)> {
    match (a.width(), b.width()) {
        (Some(_), Some(_)) if a == b => Ok(a),
        (Some(1), Some(_)) => Ok(b),
        (Some(_), Some(1)) => Ok(a),
        _ => Err((a, b)),
    }
}

fn number(a: Ty) -> Result<Ty, (Ty, Ty)> {
    a.width().map(|_| a).ok_or((Ty::F32, a))
}

fn exactly(want: Ty, got: Ty) -> Result<Ty, (Ty, Ty)> {
    if want == got { Ok(got) } else { Err((want, got)) }
}

impl Prim {
    /// The result type of this operator over `args`. Err is (wanted, given) for the first operand that fails. Operand
    /// counts are the wire's to check (`takes`); a count that slips through is refused as (F32, F32).
    pub fn result(self, args: &[Ty]) -> Result<Ty, (Ty, Ty)> {
        match (self.rule(), args) {
            (Rule::Map1, [a]) => number(*a),
            (Rule::Map2, [a, b]) => wider(number(*a)?, number(*b)?),
            (Rule::Map3, [a, b, c]) => wider(wider(number(*a)?, number(*b)?)?, number(*c)?),
            (Rule::Norm, [a]) => number(*a).map(|_| Ty::F32),
            (Rule::Dot, [a, b]) => exactly(number(*a)?, *b).map(|_| Ty::F32),
            (Rule::Cross, [a, b]) => exactly(Ty::V3, *a).and(exactly(Ty::V3, *b)),
            (Rule::Cmp, [a, b]) => exactly(Ty::F32, *a).and(exactly(Ty::F32, *b)).map(|_| Ty::Bool),
            (Rule::Logic, [a, b]) => exactly(Ty::Bool, *a).and(exactly(Ty::Bool, *b)),
            (Rule::Not, [a]) => exactly(Ty::Bool, *a),
            (Rule::Select, [c, a, b]) => exactly(Ty::Bool, *c).and(wider(number(*a)?, number(*b)?)),
            (
                Rule::Map1
                | Rule::Map2
                | Rule::Map3
                | Rule::Norm
                | Rule::Dot
                | Rule::Cross
                | Rule::Cmp
                | Rule::Logic
                | Rule::Not
                | Rule::Select,
                _,
            ) => Err((Ty::F32, Ty::F32)),
        }
    }
    /// The operand count this operator takes.
    pub fn arity(self) -> usize {
        match self.rule() {
            Rule::Map1 | Rule::Norm | Rule::Not => 1,
            Rule::Map2 | Rule::Dot | Rule::Cross | Rule::Cmp | Rule::Logic => 2,
            Rule::Map3 | Rule::Select => 3,
        }
    }
}

/// A source position, the file resolved from the job's symbols.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Site {
    pub file: Box<str>,
    pub line: u32,
    pub col: u32,
}

/// One node: its operation, its type, and its operands as `args[args..args + n]` (node ids, all below its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub op: Op,
    pub ty: Ty,
    pub args: u32,
    pub n: u32,
}

/// A graph function: one body over binder nodes (`Op::Bind`), each call's args bound to them in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphFn {
    pub name: u32,
    pub ret: Ty,
    pub binders: Box<[u32]>,
    pub body: u32,
}

/// A raw WGSL function: its name and text (symbols), the raw functions its text calls (`Graph::raws`, each
/// declared before it), and the resources its text reads by name rather than through its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawFn {
    pub name: u32,
    pub ret: Ty,
    pub text: u32,
    pub deps: Box<[u32]>,
    pub reads: Box<[Read]>,
}
/// A resource raw text reads by name (src/expr.ts `LibRead`). Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read {
    /// A table (`Kernel::Table`), through its accessor `t_<name>(i: u32) -> vec4<f32>`.
    Table(u32),
    /// Output `output` of a compute kernel (`Kernel::Compute`), by its symbol.
    Output { kernel: u32, output: u32 },
    /// The texture of an offscreen pass (`Graph::passes`, `Target::Texture`), as `loft_passtex_<name>`.
    Pass(u32),
}

/// A loop: `count` steps (f32) from `init`, each step reading the state binders and the f32 index binder and giving
/// `next`, stopped early when `stop` (bool, over the same binders) holds. `init`, `state` and `next` are parallel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loop {
    pub count: u32,
    pub init: Box<[u32]>,
    pub state: Box<[u32]>,
    pub index: u32,
    pub next: Box<[u32]>,
    pub stop: Option<u32>,
}

impl Loop {
    /// Every node the loop names: count, starts, binders, index, steps and stop.
    pub fn nodes(&self) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(self.count)
            .chain(self.init.iter().copied())
            .chain(self.state.iter().copied())
            .chain(std::iter::once(self.index))
            .chain(self.next.iter().copied())
            .chain(self.stop)
    }
}

/// What a `Fetch` reads: a table the job fills, or supplied samples (an author curve); or an author compute pass,
/// which no `Fetch` names (raw text reads its outputs, `Read::Output`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kernel {
    /// `entries` rows of `lanes` vec4s, filled by `fill`, read for `role`.
    Table {
        name: u32,
        lanes: u32,
        entries: u32,
        role: Role,
        fill: Fill,
    },
    /// Supplied samples `rows[at..at + len]`, read as one lane.
    Rows {
        label: u32,
        at: u32,
        len: u32,
    },
    Compute(Box<Compute>),
}
/// An author compute pass (src/compute.ts `compute`): WGSL text with `@compute` entries, run on a schedule, writing
/// named outputs. What its text reads through holes are `values` (roots), read as `loft_value_<k>()`; the raw
/// functions it calls are `libs` (`Graph::raws`), whose reads name other passes' outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compute {
    pub name: u32,
    pub when: When,
    pub entry: u32,
    pub dispatch: [u32; 3],
    pub text: u32,
    pub libs: Box<[u32]>,
    pub writes: Box<[Output]>,
    pub values: Box<[u32]>,
}
/// When a compute pass runs. Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Build,
    Load,
    Frame,
    /// `count` steps at `rate` (a lit) a second from the entry `start` seeds, every `keep`-th state kept.
    Steps {
        start: u32,
        count: u32,
        rate: u32,
        keep: u32,
    },
}
/// One output of a compute pass: the text's variable `key`, bound under `symbol` for readers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub key: u32,
    pub symbol: u32,
    pub shape: OutShape,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutShape {
    Buffer {
        entries: u32,
        lanes: u32,
    },
    /// `dims` 2 or 3; `size[2]` is 1 for a 2D texture. `format` a symbol (`rgba8unorm`, `r32float`, ...).
    Texture {
        dims: u32,
        size: [u32; 3],
        format: u32,
    },
}

/// How a table is filled (src/expr.ts `TableSource`). Bodies are the decl's own roots over its own binders (`Op::Bind`),
/// because a row has several lanes and a step several states, which a one-result graph function cannot say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fill {
    /// Runs laid end to end: run k fills `count` rows, each row's `lanes` (vec4 roots) over its f32 `index` binder.
    Map(Box<[Run]>),
    Scan(Scan),
    /// Supplied numbers: `entries * lanes * 4` f32 bits of `Graph::rows` from `at`.
    Data {
        at: u32,
    },
    /// One of a recurrence's tables (src/map/recur.ts): its kept states, which carry the recurrence, or its previous
    /// state or a replay buffer, which name that table.
    Recurrence(Recur),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recur {
    Kept(Box<Kept>),
    /// The previous state (what `prev` fetches inside the step) or a checkpoint replay's pair, of kernel `kept`.
    Part {
        part: Part,
        kept: u32,
    },
}
/// Wire code = the order here, from 1 (0 is the kept states).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Prev,
    Even,
    Odd,
}
/// S(n + 1) = step(S(n), t_n), t_n = from + n / rate, from `start` at `from`, kept from step `first` for `steps`
/// steps (every `every`-th state; 1 keeps all). Each kept state is `cells + 1` rows: the state, then the clock. `start`
/// and `step` are `lanes` vec4 roots over the `index` binder (f32 for a list, vec2 for a grid); `step` also reads the
/// f32 `clock` binder (its own time) and fetches `prev`. `rate`, `from` and `until` are lits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub cells: Cells,
    pub rate: u32,
    pub from: u32,
    pub until: u32,
    pub first: u32,
    pub steps: u32,
    pub every: u32,
    pub prev: u32,
    pub index: u32,
    pub clock: u32,
    pub start: Box<[u32]>,
    pub step: Box<[u32]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cells {
    List(u32),
    Grid(u32, u32),
}
impl Cells {
    pub fn count(self) -> u64 {
        match self {
            Cells::List(n) => u64::from(n),
            Cells::Grid(w, h) => u64::from(w) * u64::from(h),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub count: u32,
    pub index: u32,
    pub lanes: Box<[u32]>,
}

/// Rows `first..first + steps` in order: the state starts at `init`; step j reads the `state` binders and the f32
/// `index` binder (j) and gives `next` and row `first + j`'s `lanes`. `init`, `state` and `next` are parallel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scan {
    pub first: u32,
    pub steps: u32,
    pub init: Box<[u32]>,
    pub state: Box<[u32]>,
    pub index: u32,
    pub next: Box<[u32]>,
    pub lanes: Box<[u32]>,
}

/// Who fills a table (src/expr.ts `TableSource["role"]`). Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Stations,
    Data,
    Static,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Stations, Role::Data, Role::Static];
    pub fn name(self) -> &'static str {
        match self {
            Role::Stations => "stations",
            Role::Data => "data",
            Role::Static => "static",
        }
    }
}

/// An authored f32 parameter: default, minimum and maximum are `lits` indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: u32,
    pub def: u32,
    pub min: u32,
    pub max: u32,
}

/// A host input of a number type; its default is `ty.lits()` entries of `lits` from `def`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    pub name: u32,
    pub ty: Ty,
    pub def: u32,
}

/// The program a job asks for (plan §3.1). Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Program {
    Scene,
    Solve,
    Evaluate,
    Check,
}

/// A drawn thing (plan §1.5): how it covers pixels and the graph function (`Graph::fns`) that paints them,
/// `paint(p: vec3, n: vec3, id: f32) -> vec4`. Signatures of every role are loft-core `draw`'s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draw {
    pub name: u32,
    pub layer: Layer,
    pub cover: Cover,
    pub paint: u32,
    pub raster: Raster,
}
/// Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layer {
    Subject,
    Surroundings,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Raster {
    pub cull: bool,
    pub depth_write: bool,
    pub blend: Blend,
}
/// Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    Opaque,
    Alpha,
    Add,
}
/// Fn ids name `Graph::fns`, kernel ids `Graph::kernels`, lit ids the first of `Graph::lits`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cover {
    /// `count` copies of a u × v grid of `place(uv: vec2, id: f32) -> vec3`.
    Grid {
        u: u32,
        v: u32,
        place: u32,
        count: u32,
    },
    /// `count` copies of the triangles in a table's rows.
    Mesh {
        kernel: u32,
        count: u32,
    },
    /// `count` camera-facing quads, `place(id: f32) -> vec4` (centre, radius).
    Quads {
        place: u32,
        count: u32,
    },
    Ray {
        proxy: Proxy,
        find: Find,
    },
    Screen,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proxy {
    /// min then max: six lits from `lits`.
    Box { lits: u32 },
    /// One box per row of a table (centre xyz, scale w), `local` its min then max: six lits.
    Instances { kernel: u32, local: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Find {
    /// `hit(o: vec3, d: vec3, span: vec2, width: vec2) -> vec4` (normal, t), t < 0 a miss.
    Hit(u32),
    /// A medium of `Graph::media`.
    Medium(u32),
}
/// One medium, two integrations (plan §1.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Medium {
    /// `density(p: vec3) -> f32` marched inside `within` (six lits).
    March { density: u32, within: u32, steps: u16, shadow_steps: u16 },
    /// Gaussians in a table's rows.
    Gaussians { kernel: u32 },
}
/// Draws `first..end` of `Graph::draws` seen by a camera of `Graph::cameras` (plan §1.5), into `target`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass {
    pub camera: u32,
    pub draws: [u32; 2],
    pub clear: Clear,
    pub ground: Ground,
    pub outline: Outline,
    pub target: Target,
}
/// Where a pass draws. Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    View,
    /// An offscreen texture `loft_passtex_<name>` that raw text reads (`Read::Pass`). `scene`: it also draws every
    /// subject draw of the view passes (an author pass of the scene, src/frame/pass.ts), seen by its own camera.
    Texture {
        name: u32,
        size: Size,
        keep: Keep,
        scene: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// The view's size times a lit.
    View(u32),
    Fixed(u32, u32),
}
/// What an offscreen pass keeps (src/frame/pass.ts `PassKeep`). Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    Colour,
    Depth,
    Light,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clear {
    Load,
    /// rgba: four lits.
    Colour(u32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    None,
    /// Traced over the whole view: (hit, paint) graph functions with a draw's signatures.
    Traced {
        hit: u32,
        paint: u32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    None,
    /// `outline(colour: vec3, q: vec3) -> vec4` (ink rgb, width in world units).
    Ink(u32),
}
/// A named camera; its parts are nodes, so a camera can move with time, params and inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Camera {
    pub name: u32,
    pub lens: Lens,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lens {
    /// eye, target, up (vec3) and the vertical field of view in radians (f32).
    Look { eye: u32, target: u32, up: u32, fovy: u32 },
    /// clip from world, four vec4 columns.
    Matrix { cols: [u32; 4] },
    /// A table's one row of `struct Frame` lanes (src/frame/pass.ts `cameraLanes`), which the job fills.
    Table { kernel: u32 },
}
/// A check (plan §1.7): a formula over leaves, decided by proof or by samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub label: u32,
    pub how: Strategy,
    pub body: Formula,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Prove { boxes: u32 },
    Sample { n: u32 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Formula {
    /// `margin` (f32) > 0 holds, < 0 fails; at 0 `test` (bool) decides.
    Atom {
        margin: u32,
        test: u32,
    },
    Always(Quant),
    Eventually(Quant),
    And(Box<[Formula]>),
    Or(Box<[Formula]>),
    Not(Box<Formula>),
}
/// Leaves (Time, Param, Input or Var nodes) each in [lo, hi] (f32 nodes), parallel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quant {
    pub leaves: Box<[u32]>,
    pub lo: Box<[u32]>,
    pub hi: Box<[u32]>,
    pub body: Box<Formula>,
}
/// A solve (src/geom/solve.ts `SolveSpec`): unknown curves over [t0, t1] that meet the holds, preferring the least
/// jerk and acceleration (and nearness to given curves). The solver is loft-core's (feat/lr-solve).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solve {
    pub label: u32,
    /// t0, t1, delta, jerk weight, acceleration weight, near weight: six lits.
    pub numbers: u32,
    pub unknowns: Box<[Unknown]>,
    pub near: Box<[Near]>,
    pub holds: Box<[Hold]>,
}
/// A uniform cubic B-spline of `numbers` controls, started from `from` (f32 over time). A hold reads it through its
/// four `jets` binders: its value and first three time derivatives at the hold's time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unknown {
    pub name: u32,
    pub numbers: u32,
    pub from: u32,
    pub jets: [u32; 4],
}
/// Prefer unknown `unknown` near `to` (f32 over time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Near {
    pub unknown: u32,
    pub to: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hold {
    pub kind: HoldKind,
    pub times: Times,
    /// The comparison as written (`equal`, `ge`, ...) and where: symbols.
    pub said: u32,
    pub site: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldKind {
    /// `margin` (any number type) within the lit `within` of zero.
    Equal { margin: u32, within: u32 },
    /// `margin` (f32) at least zero.
    Bound { margin: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Times {
    At {
        t: u32,
    },
    /// from then to: two lits.
    During {
        lits: u32,
    },
    Span,
}

/// A decoded, validated job graph.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Graph {
    pub nodes: Box<[Node]>,
    pub args: Box<[u32]>,
    /// f64 bits, exactly as authored.
    pub lits: Box<[u64]>,
    /// f32 bits of supplied samples (curve controls, author tables).
    pub rows: Box<[u32]>,
    pub blobs: Box<[u8]>,
    pub syms: Box<str>,
    /// Symbol k is `syms[sym_at[k]..sym_at[k + 1]]`.
    pub sym_at: Box<[u32]>,
    pub sites: Box<[Site]>,
    /// Per node: 0 = no site, k = `sites[k - 1]`.
    pub node_sites: Box<[u32]>,
    pub fns: Box<[GraphFn]>,
    pub raws: Box<[RawFn]>,
    pub loops: Box<[Loop]>,
    pub kernels: Box<[Kernel]>,
    pub params: Box<[Param]>,
    pub inputs: Box<[Input]>,
    pub draws: Box<[Draw]>,
    pub passes: Box<[Pass]>,
    pub media: Box<[Medium]>,
    pub cameras: Box<[Camera]>,
    pub checks: Box<[Check]>,
    pub solves: Box<[Solve]>,
    /// The program's roots.
    pub roots: Box<[u32]>,
}

impl Graph {
    pub fn args_of(&self, node: &Node) -> &[u32] {
        &self.args[node.args as usize..(node.args + node.n) as usize]
    }
    pub fn sym(&self, i: u32) -> &str {
        &self.syms[self.sym_at[i as usize] as usize..self.sym_at[i as usize + 1] as usize]
    }
    pub fn site(&self, node: u32) -> Option<&Site> {
        self.node_sites[node as usize].checked_sub(1).map(|k| &self.sites[k as usize])
    }
}
