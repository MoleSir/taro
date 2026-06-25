mod base;
pub use base::*;

mod object;
pub use object::*;

pub mod compile;
pub mod vm;
mod std;
mod builtin;

#[cfg(test)]
mod tests;