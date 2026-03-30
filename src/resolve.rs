//! Resolver engine: drives bucket state transitions and produces decision records.

use crate::bucket::Bucket;
use crate::contracts::canon_entry::CanonEntry;
use crate::contracts::escalation::Escalation;
use crate::contracts::policy::Policy;

/// The decision produced for a single bucket after resolution.
#[derive(Debug)]
pub enum Decision {
    /// Bucket resolved to a canonical entry.
    Resolved(CanonEntry),
    /// Bucket escalated for human review.
    Escalated(Escalation),
}

/// Resolve a single bucket against the policy. Returns a decision record.
pub fn resolve_bucket(_bucket: &Bucket, _policy: &Policy) -> Decision {
    todo!("bucket resolution and state machine transitions")
}
