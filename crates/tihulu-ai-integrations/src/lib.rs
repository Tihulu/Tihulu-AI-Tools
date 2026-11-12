//! Inert, Rust-first contracts for future Jan Desktop MCP and TIHULU adapters.
//!
//! No MCP listener, Jan configuration edits, external process invocation, model
//! loading, filesystem scan or hardware access is implemented by this crate.
//! The host application must create trusted adapters and obtain real consent.

#![forbid(unsafe_code)]

pub mod bridge;
pub mod lifecycle;
pub mod references;
pub mod typed;
pub mod wizard;
