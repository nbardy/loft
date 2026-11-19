//! World geometry acceleration. The frozen wire stores topology and row addresses;
//! the caller supplies per-kind intersections and owns the row data.
use crate::program::Instance;
use loft_abi::{BvhNode, LeafRef};
use std::collections::HashMap;


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BvhError {
    TooManyInstances,
    InvalidBounds(LeafRef),
    DuplicateInstance(LeafRef),
    ChangedMembership,
    InvalidRay,
}

/// A ray with a unit direction and an inclusive, nonnegative interval in metres.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    origin: [f32; 3],
    direction: [f32; 3],
    near: f32,
    far: f32,
}

impl Ray {
    pub fn new(
        origin: [f32; 3],
        direction: [f32; 3],
        near: f32,
        far: f32,
    ) -> Result<Self, BvhError> {
        let length = direction.into_iter().map(|x| x * x).sum::<f32>().sqrt();
        if !origin.into_iter().all(f32::is_finite)
            || !length.is_finite()
            || (length - 1.0).abs() > 1e-5
            || !near.is_finite()
            || near < 0.0
            || far.is_nan()
            || far < near
        {
            return Err(BvhError::InvalidRay);
        }
        Ok(Self {
            origin,
            direction,
            near,
            far,
        })
    }

    pub fn direction(self) -> [f32; 3] {
        self.direction
    }

    pub fn interval(self) -> std::ops::RangeInclusive<f32> {
        self.near..=self.far
    }

    pub fn at(self, t: f32) -> [f32; 3] {
        std::array::from_fn(|i| self.origin[i] + t * self.direction[i])
    }

    fn clip(self, lo: [f32; 3], hi: [f32; 3]) -> Option<Self> {
        let (mut near, mut far) = (self.near, self.far);
        for i in 0..3 {
            let (o, d) = (self.origin[i], self.direction[i]);
            if d == 0.0 {
                if o < lo[i] || o > hi[i] {
                    return None;
                }
                continue;
            }
            // Direct division also handles an origin on the slab with a subnormal direction.
            let a = (lo[i] - o) / d;
            let b = (hi[i] - o) / d;
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
        Some(Self { near, far, ..self })
    }

    fn accepts(self, t: f32) -> bool {
        t.is_finite() && (self.near..=self.far).contains(&t)
    }
}

#[derive(Default)]
pub struct Bvh2 {
    nodes: Vec<BvhNode>,
    leaves: Vec<LeafRef>,
}

impl Bvh2 {
    pub fn build(instances: &[Instance]) -> Result<Self, BvhError> {
        if instances.len() > u32::MAX as usize {
            return Err(BvhError::TooManyInstances);
        }
        index_instances(instances)?;
        let mut tree = Self::default();
        if !instances.is_empty() {
            tree.nodes.push(BvhNode::default());
            tree.split(0, &mut instances.to_vec());
        }
        Ok(tree)
    }

    pub fn nodes(&self) -> &[BvhNode] {
        &self.nodes
    }

    pub fn leaves(&self) -> &[LeafRef] {
        &self.leaves
    }

    fn split(&mut self, i: usize, instances: &mut [Instance]) {
        let (lo, hi) = bounds(instances);
        if instances.len() <= 4 {
            let left_or_first = self.leaves.len() as u32;
            instances.sort_unstable_by_key(|item| (item.kind, item.row));
            self.leaves.extend(instances.iter().map(leaf_ref));
            self.nodes[i] = BvhNode {
                lo,
                hi,
                left_or_first,
                count: instances.len() as u32,
            };
            return;
        }
        let axis = (0..3)
            .max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
            .unwrap();
        let mid = instances.len() / 2;
        instances.select_nth_unstable_by(mid, |a, b| {
            let key = |x: &Instance| f64::from(x.lo[axis]) + f64::from(x.hi[axis]);
            key(a)
                .total_cmp(&key(b))
                .then_with(|| (a.kind, a.row).cmp(&(b.kind, b.row)))
        });
        let (left, right) = instances.split_at_mut(mid);
        let child = self.nodes.len();
        self.nodes.resize(child + 2, BvhNode::default());
        self.nodes[i] = BvhNode {
            lo,
            hi,
            left_or_first: child as u32,
            count: 0,
        };
        self.split(child, left);
        self.split(child + 1, right);
    }

    /// Refit in reverse node order, accepting any input order. Failure leaves the
    /// previous tree usable. Membership changes require a rebuild.
    pub fn refit(&mut self, instances: &[Instance]) -> Result<(), BvhError> {
        let index = index_instances(instances)?;
        if index.len() != self.leaves.len()
            || self
                .leaves
                .iter()
                .any(|r| !index.contains_key(&(r.kind_id, r.row_id)))
        {
            return Err(BvhError::ChangedMembership);
        }
        for i in (0..self.nodes.len()).rev() {
            let node = self.nodes[i];
            let first = node.left_or_first as usize;
            let (lo, hi) = if node.count == 0 {
                let [a, b] = [self.nodes[first], self.nodes[first + 1]];
                union((a.lo, a.hi), (b.lo, b.hi))
            } else {
                self.leaves[first..first + node.count as usize]
                    .iter()
                    .fold(EMPTY, |acc, r| {
                        let item = index[&(r.kind_id, r.row_id)];
                        union(acc, (item.lo, item.hi))
                    })
            };
            self.nodes[i].lo = lo;
            self.nodes[i].hi = hi;
        }
        Ok(())
    }

    fn intersect<E>(
        &self,
        mut ray: Ray,
        any_hit: bool,
        mut intersect: impl FnMut(LeafRef, Ray) -> Result<Option<f32>, E>,
    ) -> Result<Option<(LeafRef, f32)>, E> {
        let Some(root) = self.nodes.first() else {
            return Ok(None);
        };
        let Some(span) = ray.clip(root.lo, root.hi) else {
            return Ok(None);
        };
        // With four-row leaves, median splits of up to u32::MAX rows fit in 32 slots.
        let mut stack = [(0, 0.); 32];
        stack[0] = (0, span.near);
        let mut depth = 1;
        let mut best = None;
        while depth > 0 {
            depth -= 1;
            let (i, enter) = stack[depth];
            if enter > ray.far {
                continue;
            }
            let node = self.nodes[i];
            let first = node.left_or_first as usize;
            if node.count == 0 {
                let children = [first, first + 1].map(|j| {
                    ray.clip(self.nodes[j].lo, self.nodes[j].hi)
                        .map(|span| (j, span.near))
                });
                match children {
                    [Some(a), Some(b)] => {
                        let (near, far) = if a.1 <= b.1 { (a, b) } else { (b, a) };
                        stack[depth] = far;
                        stack[depth + 1] = near;
                        depth += 2;
                    }
                    [Some(child), None] | [None, Some(child)] => {
                        stack[depth] = child;
                        depth += 1;
                    }
                    [None, None] => {}
                }
                continue;
            }
            for &leaf in &self.leaves[first..first + node.count as usize] {
                if let Some(t) = intersect(leaf, ray)?
                    && ray.accepts(t)
                {
                    if any_hit {
                        return Ok(Some((leaf, t)));
                    }
                    let id = |r: LeafRef| (r.kind_id, r.row_id);
                    if best.is_none_or(|(prev, at)| t < at || t == at && id(leaf) < id(prev)) {
                        best = Some((leaf, t));
                        ray.far = t;
                    }
                }
            }
        }
        Ok(best)
    }

    pub fn trace<E>(
        &self,
        ray: Ray,
        intersect: impl FnMut(LeafRef, Ray) -> Result<Option<f32>, E>,
    ) -> Result<Option<(LeafRef, f32)>, E> {
        self.intersect(ray, false, intersect)
    }

    pub fn occluded<E>(
        &self,
        ray: Ray,
        intersect: impl FnMut(LeafRef, Ray) -> Result<Option<f32>, E>,
    ) -> Result<bool, E> {
        self.intersect(ray, true, intersect)
            .map(|hit| hit.is_some())
    }
}

fn leaf_ref(item: &Instance) -> LeafRef {
    LeafRef {
        kind_id: item.kind,
        row_id: item.row,
    }
}

fn index_instances(instances: &[Instance]) -> Result<HashMap<(u32, u32), &Instance>, BvhError> {
    let mut index = HashMap::with_capacity(instances.len());
    for item in instances {
        if (0..3)
            .any(|i| !item.lo[i].is_finite() || !item.hi[i].is_finite() || item.lo[i] > item.hi[i])
        {
            return Err(BvhError::InvalidBounds(leaf_ref(item)));
        }
        if index.insert((item.kind, item.row), item).is_some() {
            return Err(BvhError::DuplicateInstance(leaf_ref(item)));
        }
    }
    Ok(index)
}

type Bounds = ([f32; 3], [f32; 3]);
const EMPTY: Bounds = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);

fn union((lo, hi): Bounds, (other_lo, other_hi): Bounds) -> Bounds {
    (
        std::array::from_fn(|i| lo[i].min(other_lo[i])),
        std::array::from_fn(|i| hi[i].max(other_hi[i])),
    )
}

fn bounds(instances: &[Instance]) -> Bounds {
    instances
        .iter()
        .fold(EMPTY, |acc, item| union(acc, (item.lo, item.hi)))
}
