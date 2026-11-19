//! Native E+ front end.
pub mod cst;
mod parser;
pub use parser::parse;
pub mod ast;
