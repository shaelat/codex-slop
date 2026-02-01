//! Traceroute collection and parsing.

pub mod parser;
pub mod runner;

pub use parser::{parse_traceroute_n, ParsedTraceRun};
