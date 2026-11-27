//! Native E+ front end.
#[path = "lex/legacy.rs"]
pub mod cst;
mod lex;
mod parser;
pub use lex::UNIT_SYMBOLS;
pub use parser::{parse, TYPE_ALIASES};
pub mod ast;
