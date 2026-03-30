//! Logical bucket keys, edge/base bucket construction, bucket grouping, and bucket store.

use std::collections::BTreeSet;

use crate::contracts::claim::Claim;
use crate::contracts::vocabulary::{PropertyType, SubjectRef, ValueRef};
use crate::normalize::{canonical_json, sha256_hex};
use indexmap::map::Entry;
use serde_json::json;

/// The logical key for a bucket. Base buckets use subject + property_type.
/// Edge buckets additionally include the value ref.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BucketKey {
    Base(BaseBucketKey),
    Edge(EdgeBucketKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BaseBucketKey {
    pub subject: SubjectRef,
    pub property_type: PropertyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeBucketKey {
    pub subject: SubjectRef,
    pub property_type: PropertyType,
    pub value_ref: ValueRef,
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

impl Bucket {
    pub fn claim_count(&self) -> usize {
        self.claims.len()
    }

    pub fn source_artifact_count(&self) -> usize {
        self.claims
            .iter()
            .map(|claim| claim.source.artifact_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    }

    fn refresh_after_insert(&mut self) {
        self.claims
            .sort_unstable_by(|left, right| left.claim_id.cmp(&right.claim_id));
        self.state = match self.claims.len() {
            0 => BucketState::Empty,
            1 => BucketState::SingleSource,
            _ => BucketState::Converging,
        };
    }
}

impl BucketKey {
    pub fn subject(&self) -> &SubjectRef {
        match self {
            BucketKey::Base(key) => &key.subject,
            BucketKey::Edge(key) => &key.subject,
        }
    }

    pub fn property_type(&self) -> PropertyType {
        match self {
            BucketKey::Base(key) => key.property_type,
            BucketKey::Edge(key) => key.property_type,
        }
    }

    pub fn value_ref(&self) -> Option<&ValueRef> {
        match self {
            BucketKey::Base(_) => None,
            BucketKey::Edge(key) => Some(&key.value_ref),
        }
    }

    fn canonical_value(&self) -> serde_json::Value {
        match self {
            BucketKey::Base(key) => json!({
                "subject": key.subject,
                "property_type": key.property_type,
            }),
            BucketKey::Edge(key) => json!({
                "subject": key.subject,
                "property_type": key.property_type,
                "value": key.value_ref,
            }),
        }
    }
}

/// Compute the logical bucket key for a claim.
pub fn bucket_key_for(claim: &Claim) -> BucketKey {
    if claim.property_type.is_edge() {
        let value_ref = claim
            .value_ref()
            .expect("edge claims are validated before bucketing")
            .expect("edge property must contain a value ref");

        BucketKey::Edge(EdgeBucketKey {
            subject: claim.subject.clone(),
            property_type: claim.property_type,
            value_ref,
        })
    } else {
        BucketKey::Base(BaseBucketKey {
            subject: claim.subject.clone(),
            property_type: claim.property_type,
        })
    }
}

/// Compute the bucket_id hash from a bucket key.
pub fn bucket_id(key: &BucketKey) -> String {
    sha256_hex(canonical_json(&key.canonical_value()).as_bytes())
}

/// The bucket store: groups claims by logical bucket key.
#[derive(Debug, Default)]
pub struct BucketStore {
    pub buckets: indexmap::IndexMap<String, Bucket>,
    seen_claim_ids: BTreeSet<String>,
}

impl BucketStore {
    /// Insert a claim into the appropriate bucket, handling duplicate collapse.
    pub fn insert(&mut self, claim: Claim) {
        if !self.seen_claim_ids.insert(claim.claim_id.clone()) {
            return;
        }

        let key = bucket_key_for(&claim);
        let id = bucket_id(&key);

        match self.buckets.entry(id.clone()) {
            Entry::Occupied(mut entry) => {
                let bucket = entry.get_mut();
                bucket.claims.push(claim);
                bucket.refresh_after_insert();
            }
            Entry::Vacant(entry) => {
                let mut bucket = Bucket {
                    key,
                    bucket_id: id,
                    state: BucketState::Empty,
                    claims: vec![claim],
                };
                bucket.refresh_after_insert();
                entry.insert(bucket);
            }
        }

        self.buckets
            .sort_by(|left_key, _, right_key, _| left_key.cmp(right_key));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BaseBucketKey, BucketKey, BucketState, BucketStore, EdgeBucketKey, bucket_id,
        bucket_key_for,
    };
    use crate::contracts::claim::parse_claim;
    use crate::contracts::vocabulary::{PropertyType, SubjectKind};
    use crate::normalize::{canonical_json, sha256_hex};
    use serde_json::json;

    fn mixed_source_fixture() -> &'static str {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/claims/mixed_source.jsonl"
        ))
    }

    #[test]
    fn uses_edge_bucket_keys_for_edge_properties() {
        let claim = parse_claim(mixed_source_fixture().lines().next().unwrap()).unwrap();
        let key = bucket_key_for(&claim);

        assert_eq!(
            key,
            BucketKey::Edge(EdgeBucketKey {
                subject: claim.subject.clone(),
                property_type: PropertyType::DependsOn,
                value_ref: claim.value_ref().unwrap().unwrap(),
            })
        );
    }

    #[test]
    fn uses_base_bucket_keys_for_non_edge_properties() {
        let claim = parse_claim(mixed_source_fixture().lines().nth(3).unwrap()).unwrap();
        let key = bucket_key_for(&claim);

        assert_eq!(
            key,
            BucketKey::Base(BaseBucketKey {
                subject: claim.subject.clone(),
                property_type: PropertyType::Liveness,
            })
        );
    }

    #[test]
    fn bucket_id_matches_phase_one_base_object_shape() {
        let key = BucketKey::Base(BaseBucketKey {
            subject: crate::contracts::vocabulary::SubjectRef {
                kind: SubjectKind::Feed,
                id: "fdmee.actuals_load".to_string(),
            },
            property_type: PropertyType::Liveness,
        });

        let expected = sha256_hex(
            canonical_json(&json!({
                "subject": {"kind": "feed", "id": "fdmee.actuals_load"},
                "property_type": "liveness",
            }))
            .as_bytes(),
        );

        assert_eq!(bucket_id(&key), expected);
    }

    #[test]
    fn edge_bucket_ids_do_not_collapse_distinct_targets() {
        let left = parse_claim(mixed_source_fixture().lines().next().unwrap()).unwrap();
        let right = parse_claim(r#"{"event":"claim.v0","claim_id":"sha256:efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef","source":{"kind":"repo_scan","scanner":"crucible.scan.repo@0.1.0","artifact_id":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","locator":{"kind":"file_range","value":"sql/close_pack/report_dependencies.sql#L29-L40"}},"subject":{"kind":"report","id":"hyperion.close_pack_ebitda"},"property_type":"depends_on","value":{"kind":"feed","id":"fdmee.budget_load"},"confidence":0.89}"#).unwrap();

        assert_ne!(bucket_key_for(&left), bucket_key_for(&right));
        assert_ne!(
            bucket_id(&bucket_key_for(&left)),
            bucket_id(&bucket_key_for(&right))
        );
    }

    #[test]
    fn bucket_id_is_stable_for_identical_keys() {
        let claim = parse_claim(mixed_source_fixture().lines().next().unwrap()).unwrap();
        let key = bucket_key_for(&claim);

        assert_eq!(bucket_id(&key), bucket_id(&key));
    }

    #[test]
    fn bucket_store_groups_claims_by_logical_key_and_sorts_by_claim_id() {
        let mut store = BucketStore::default();

        for line in mixed_source_fixture()
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            store.insert(parse_claim(line).unwrap());
        }

        assert_eq!(store.buckets.len(), 1);

        let bucket = store.buckets.values().next().unwrap();
        assert_eq!(bucket.state, BucketState::Converging);
        assert_eq!(bucket.claim_count(), 3);
        assert_eq!(bucket.source_artifact_count(), 3);
        assert_eq!(
            bucket
                .claims
                .iter()
                .map(|claim| claim.claim_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            ]
        );
    }

    #[test]
    fn bucket_store_collapses_duplicate_claim_ids_before_counting() {
        let mut store = BucketStore::default();
        let claim = parse_claim(mixed_source_fixture().lines().next().unwrap()).unwrap();

        store.insert(claim.clone());
        store.insert(claim);

        let bucket = store.buckets.values().next().unwrap();
        assert_eq!(bucket.state, BucketState::SingleSource);
        assert_eq!(bucket.claim_count(), 1);
        assert_eq!(bucket.source_artifact_count(), 1);
    }

    #[test]
    fn bucket_store_counts_distinct_source_artifacts_after_duplicate_collapse() {
        let mut store = BucketStore::default();
        let first = parse_claim(mixed_source_fixture().lines().next().unwrap()).unwrap();
        let second = parse_claim(r#"{"event":"claim.v0","claim_id":"sha256:9999888877776666555544443333222211110000aaaabbbbccccddddeeeeffff","source":{"kind":"repo_scan","scanner":"crucible.scan.repo@0.1.0","artifact_id":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","locator":{"kind":"file_range","value":"sql/close_pack/report_dependencies.sql#L41-L59"}},"subject":{"kind":"report","id":"hyperion.close_pack_ebitda"},"property_type":"depends_on","value":{"kind":"feed","id":"fdmee.actuals_load"},"confidence":0.84}"#).unwrap();

        store.insert(first);
        store.insert(second);

        let bucket = store.buckets.values().next().unwrap();
        assert_eq!(bucket.claim_count(), 2);
        assert_eq!(bucket.source_artifact_count(), 1);
    }

    #[test]
    fn bucket_store_collapses_duplicate_claim_ids_before_cross_bucket_insert() {
        let mut store = BucketStore::default();
        let first = parse_claim(mixed_source_fixture().lines().next().unwrap()).unwrap();
        let duplicate_with_different_bucket = parse_claim(
            r#"{"event":"claim.v0","claim_id":"sha256:1111111111111111111111111111111111111111111111111111111111111111","source":{"kind":"db_scan","scanner":"crucible.scan.db@0.1.0","artifact_id":"sha256:1616161616161616161616161616161616161616161616161616161616161616","locator":{"kind":"table_row","value":"ops.feed_registry#fdmee.actuals_load"}},"subject":{"kind":"feed","id":"fdmee.actuals_load"},"property_type":"liveness","value":{"kind":"scalar","value":"alive"},"confidence":0.94}"#,
        )
        .unwrap();

        store.insert(first);
        store.insert(duplicate_with_different_bucket);

        assert_eq!(store.buckets.len(), 1);

        let bucket = store.buckets.values().next().unwrap();
        assert_eq!(bucket.state, BucketState::SingleSource);
        assert_eq!(bucket.claim_count(), 1);
        assert_eq!(bucket.key.property_type(), PropertyType::DependsOn);
    }
}
