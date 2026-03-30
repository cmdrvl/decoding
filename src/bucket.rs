//! Logical bucket keys, edge/base bucket construction, bucket grouping, and bucket store.

use crate::contracts::claim::{Claim, SubjectRef};
use crate::contracts::vocabulary::PropertyType;

/// The logical key for a bucket. Base buckets use subject + property_type.
/// Edge buckets additionally include the value ref.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BucketKey {
    pub subject: SubjectRef,
    pub property_type: PropertyType,
    pub value_ref: Option<SubjectRef>,
}

/// A bucket collecting claims for a single logical key.
#[derive(Debug)]
pub struct Bucket {
    pub key: BucketKey,
    pub bucket_id: String,
    pub state: BucketState,
    pub claims: Vec<Claim>,
}

/// Bucket state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketState {
    Empty,
    SingleSource,
    Converging,
    Converged,
    Conflicted,
    Escalated,
}

/// Compute the logical bucket key for a claim.
pub fn bucket_key_for(_claim: &Claim) -> BucketKey {
    todo!("bucket key construction from claim")
}

/// Compute the bucket_id hash from a bucket key.
pub fn bucket_id(_key: &BucketKey) -> String {
    todo!("bucket_id from canonical JSON of bucket key")
}

/// The bucket store: groups claims by logical bucket key.
#[derive(Debug, Default)]
pub struct BucketStore {
    pub buckets: indexmap::IndexMap<String, Bucket>,
}

impl BucketStore {
    /// Insert a claim into the appropriate bucket, handling duplicate collapse.
    pub fn insert(&mut self, _claim: Claim) {
        todo!("insert claim into bucket store with duplicate collapse")
    }
}
