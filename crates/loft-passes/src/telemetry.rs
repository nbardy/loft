//! Static counts over Naga IR. Each reachable function is visited once per module.
use crate::program::{Program, ShaderModule};
use naga::{AddressSpace, Block, Expression, Function, MathFunction, Statement, TypeInner};
use serde::Serialize;

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
