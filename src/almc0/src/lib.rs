//! Stage-0 Alúma compiler (ADR-002): `.alm` source to canonical IR.
//!
//! Bracketed section numbers point into `documentation/language_reference.md`.

pub mod ast;
pub mod consts;
pub mod lex;
pub mod library;
pub mod load;
pub mod parse;
pub mod resolve;
pub mod token;
