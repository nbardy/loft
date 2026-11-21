//! World calls at the Layer 2 lowering boundary (06 §6). V is the operand
//! representation of the consumer, e.g. a checked type, expression or SSA id.
use loft_ir::{Code, Diag, Res, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LightSelection<V> {
    Indexed(V),
    Pick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldQuery<V> {
    Trace { ray: V },
    Visible { a: V, b: V },
    Material { hit: V },
    Light { selection: LightSelection<V>, point: V, rng: V },
    Sky { direction: V },
    Medium { point: V },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldCall<V> {
    /// None selects the scene world in a single-world program.
    pub world: Option<V>,
    pub query: WorldQuery<V>,
}

pub(crate) const QUERY_ARITIES: &[(&str, usize)] = &[
    ("trace", 1), ("material", 1), ("sky", 1), ("medium", 1),
    ("visible", 2), ("pick_light", 2), ("light", 3),
];

impl<V: Clone> WorldCall<V> {
    pub fn resolve(name: &str, operands: &[V], span: Span) -> Res<Option<Self>> {
        let Some(arity) = QUERY_ARITIES.iter().find_map(|&(n, arity)| (name == n).then_some(arity)) else {
            return Ok(None);
        };
        let (world, args) = match operands.len() {
            n if n == arity => (None, operands),
            n if n == arity + 1 => (Some(operands[0].clone()), &operands[1..]),
            _ => return Err(Diag::new(Code::E003, span, format!("{name} takes {arity} operands after the World"),
                "supply the query operands and optionally a leading World")),
        };
        use WorldQuery::*;
        let query = match (name, args) {
            ("trace", [r]) => Trace { ray: r.clone() },
            ("visible", [a, b]) => Visible { a: a.clone(), b: b.clone() },
            ("material", [h]) => Material { hit: h.clone() },
            ("light", [i, p, r]) => Light { selection: LightSelection::Indexed(i.clone()), point: p.clone(), rng: r.clone() },
            ("pick_light", [p, r]) => Light { selection: LightSelection::Pick, point: p.clone(), rng: r.clone() },
            ("sky", [d]) => Sky { direction: d.clone() },
            ("medium", [p]) => Medium { point: p.clone() },
            _ => unreachable!("matched query arity"),
        };
        Ok(Some(Self { world, query }))
    }
}
