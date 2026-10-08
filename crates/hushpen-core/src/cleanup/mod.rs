//! Rule-based cleanup of the raw transcript. Pure text in, text out.

pub mod rules;
mod token;

pub use rules::{Options, clean};

#[cfg(test)]
mod tests;
