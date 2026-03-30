//! Render canon_entry.v0 and escalation.v0 records to JSONL output.

use std::io::Write;

use crate::contracts::canon_entry::CanonEntry;
use crate::contracts::escalation::Escalation;

/// Write a canon entry as a single JSONL line.
pub fn write_canon_entry(
    _writer: &mut dyn Write,
    _entry: &CanonEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("render canon_entry.v0 JSONL")
}

/// Write an escalation as a single JSONL line.
pub fn write_escalation(
    _writer: &mut dyn Write,
    _escalation: &Escalation,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("render escalation.v0 JSONL")
}
