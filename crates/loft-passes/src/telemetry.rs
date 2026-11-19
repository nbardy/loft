//! Static counts over Naga IR. Each reachable function is visited once per module.
use crate::program::{Program, ShaderModule};
use naga::{AddressSpace, Block, Expression, Function, MathFunction, Statement, TypeInner};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StorageEntry {
    pub entry: String,
    pub stage: naga::ShaderStage,
    pub bindings: Vec<(u32, u32)>,
}

pub fn storage_entries(module: &naga::Module) -> Result<Vec<StorageEntry>, String> {
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(module).map_err(|e| e.to_string())?;
    Ok(module.entry_points.iter().enumerate().map(|(i, entry)| {
        let uses = info.get_entry_point(i);
        let mut bindings = module.global_variables.iter().filter_map(|(handle, var)| {
            let binding = var.binding.as_ref()?;
            (matches!(var.space, AddressSpace::Storage { .. }) && !uses[handle].is_empty())
                .then_some((binding.group, binding.binding))
        }).collect::<Vec<_>>();
        bindings.sort_unstable();
        StorageEntry { entry: entry.name.clone(), stage: entry.stage, bindings }
    }).collect())
}

#[derive(Debug, Default, Serialize)]
pub struct ModuleTelemetry {
    pub module_id: String,
    pub wgsl_lines: u32,
    pub alu_ops: u32,
    pub transcendentals: u32,
    pub storage_loads: u32,
    pub storage_stores: u32,
    pub texture_samples: u32,
    pub workgroup_bytes: u32,
    pub max_loop_depth: u32,
    pub branch_count: u32,
    pub call_sites: u32,
}

pub fn collect(program: &Program) -> Vec<ModuleTelemetry> {
    program
        .modules
        .iter()
        .map(ModuleTelemetry::from_shader)
        .collect()
}

impl ModuleTelemetry {
    pub fn from_shader(shader: &ShaderModule) -> Self {
        let module = &shader.naga;
        let ctx = module.to_ctx();
        let mut count = Self {
            module_id: shader.name.clone(),
            wgsl_lines: shader.wgsl.lines().count() as u32,
            workgroup_bytes: module
                .global_variables
                .iter()
                .filter(|(_, var)| var.space == AddressSpace::WorkGroup)
                .map(|(_, var)| module.types[var.ty].inner.size(ctx))
                .sum(),
            ..Self::default()
        };
        let mut visited = vec![false; module.functions.len()];
        for entry in &module.entry_points {
            count.function(module, &entry.function, &mut visited);
        }
        count
    }

    fn function(&mut self, module: &naga::Module, function: &Function, visited: &mut [bool]) {
        use MathFunction::*;
        for (_, expr) in function.expressions.iter() {
            match *expr {
                Expression::Unary { .. } | Expression::Binary { .. } => self.alu_ops += 1,
                Expression::Math { fun, .. } => match fun {
                    Sin | Cos | Tan | Asin | Acos | Atan | Atan2 | Sinh | Cosh | Tanh | Asinh
                    | Acosh | Atanh | Exp | Exp2 | Log | Log2 | Pow | Sqrt | InverseSqrt => {
                        self.transcendentals += 1
                    }
                    _ => self.alu_ops += 1,
                },
                Expression::Load { pointer } if storage(module, function, pointer) => {
                    self.storage_loads += 1
                }
                Expression::ImageSample { .. } | Expression::ImageLoad { .. } => {
                    self.texture_samples += 1
                }
                _ => {}
            }
        }
        self.block(module, function, &function.body, 0, visited);
    }

    fn block(
        &mut self,
        module: &naga::Module,
        function: &Function,
        block: &Block,
        depth: u32,
        visited: &mut [bool],
    ) {
        for stmt in block {
            match stmt {
                Statement::Block(body) => self.block(module, function, body, depth, visited),
                Statement::If { accept, reject, .. } => {
                    self.branch_count += 1;
                    self.block(module, function, accept, depth, visited);
                    self.block(module, function, reject, depth, visited);
                }
                Statement::Switch { cases, .. } => {
                    self.branch_count += 1;
                    for case in cases {
                        self.block(module, function, &case.body, depth, visited);
                    }
                }
                Statement::Loop {
                    body, continuing, ..
                } => {
                    self.max_loop_depth = self.max_loop_depth.max(depth + 1);
                    self.block(module, function, body, depth + 1, visited);
                    self.block(module, function, continuing, depth + 1, visited);
                }
                Statement::Store { pointer, .. } if storage(module, function, *pointer) => {
                    self.storage_stores += 1
                }
                Statement::Call {
                    function: target, ..
                } => {
                    self.call_sites += 1;
                    if !std::mem::replace(&mut visited[target.index()], true) {
                        // Loop depth is lexical and resets at a function boundary.
                        self.function(module, &module.functions[*target], visited);
                    }
                }
                _ => {}
            }
        }
    }
}

fn storage(
    module: &naga::Module,
    function: &Function,
    mut pointer: naga::Handle<Expression>,
) -> bool {
    loop {
        match function.expressions[pointer] {
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => {
                pointer = base
            }
            Expression::GlobalVariable(g) => {
                return matches!(
                    module.global_variables[g].space,
                    AddressSpace::Storage { .. }
                );
            }
            Expression::FunctionArgument(i) => {
                return matches!(
                    module.types[function.arguments[i as usize].ty].inner,
                    TypeInner::Pointer {
                        space: AddressSpace::Storage { .. },
                        ..
                    }
                );
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(wgsl: &str) -> ModuleTelemetry {
        let naga = naga::front::wgsl::parse_str(wgsl).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&naga)
        .unwrap();
        ModuleTelemetry::from_shader(&ShaderModule {
            name: "fixture".into(),
            wgsl: wgsl.into(),
            naga,
            span: None,
            origins: vec![],
        })
    }

    #[test]
    fn l23a_exact_naga_counts_and_json() {
        let text = include_str!("../../../tests/fixtures/telemetry.wgsl");
        assert_eq!(
            serde_json::to_value(counts(text)).unwrap(),
            serde_json::json!({
                "module_id": "fixture", "wgsl_lines": 10, "alu_ops": 2, "transcendentals": 1,
                "storage_loads": 1, "storage_stores": 1, "texture_samples": 0, "workgroup_bytes": 64,
                "max_loop_depth": 1, "branch_count": 1, "call_sites": 1
            })
        );
    }

    #[test]
    fn l23a_detects_five_inline_bsdf_copies() {
        let entry = "@compute @workgroup_size(32) fn main(@builtin(local_invocation_index) i: u32) { var x = f32(i);";
        let lobe = "x = (x * x + 0.2) / (x + 0.4);";
        let original = counts(&format!(
            "fn lobe(v: f32) -> f32 {{ var x = v; {lobe} return x; }} {entry} x = lobe(x); }}"
        ));
        let mutant = counts(&format!("{entry} {} }}", lobe.repeat(5)));
        assert_eq!((original.alu_ops, original.call_sites), (4, 1));
        assert_eq!((mutant.alu_ops, mutant.call_sites), (20, 0));
        assert!(mutant.alu_ops > original.alu_ops);
    }
}
