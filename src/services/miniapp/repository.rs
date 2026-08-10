use std::collections::BTreeMap;

use super::types::{LeadRecord, ListingSummary};

pub trait ListingRepository {
    fn list(&self) -> Vec<ListingSummary>;
}

#[derive(Clone, Debug)]
pub struct FixtureListingRepository {
    listings: Vec<ListingSummary>,
}

impl Default for FixtureListingRepository {
    fn default() -> Self {
        let listings = serde_json::from_str(include_str!("fixtures/listings.json"))
            .expect("embedded miniapp fixture must be valid");
        Self { listings }
    }
}

impl FixtureListingRepository {
    #[cfg(test)]
    pub fn from_listings(listings: Vec<ListingSummary>) -> Self {
        Self { listings }
    }
}

impl ListingRepository for FixtureListingRepository {
    fn list(&self) -> Vec<ListingSummary> {
        self.listings.clone()
    }
}

/// Future production adapter. Deliberately unavailable in Phase 1.
pub struct SpacetimeListingRepository;

impl SpacetimeListingRepository {
    pub fn unavailable_reason() -> &'static str {
        "SpacetimeListingRepository is reserved and not configured in Phase 1"
    }
}

pub trait LeadRepository {
    fn find_by_idempotency_key(&self, key: &str) -> Option<LeadRecord>;
    fn insert(&mut self, key: String, lead: LeadRecord) -> LeadRecord;
}

#[derive(Debug, Default)]
pub struct InMemoryLeadRepository {
    by_idempotency_key: BTreeMap<String, LeadRecord>,
}

impl LeadRepository for InMemoryLeadRepository {
    fn find_by_idempotency_key(&self, key: &str) -> Option<LeadRecord> {
        self.by_idempotency_key.get(key).cloned()
    }

    fn insert(&mut self, key: String, lead: LeadRecord) -> LeadRecord {
        self.by_idempotency_key.insert(key, lead.clone());
        lead
    }
}

/// Future production adapter. No persistence or external notification is attempted in Phase 1.
pub struct SpacetimeLeadRepository;

impl SpacetimeLeadRepository {
    pub fn unavailable_reason() -> &'static str {
        "SpacetimeLeadRepository is reserved and not configured in Phase 1"
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvisorWorkload {
    pub advisor_id: String,
    pub pending_count: u32,
}

pub fn balanced_advisor_candidate(advisors: &[AdvisorWorkload]) -> Option<AdvisorWorkload> {
    let mut candidates = advisors.to_vec();
    candidates.sort_by(|left, right| {
        left.pending_count
            .cmp(&right.pending_count)
            .then_with(|| left.advisor_id.cmp(&right.advisor_id))
    });
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture不少于十五套且完全内嵌() {
        assert!(FixtureListingRepository::default().list().len() >= 15);
    }

    #[test]
    fn 顾问候选按待跟进量和稳定id排序() {
        let candidate = balanced_advisor_candidate(&[
            AdvisorWorkload {
                advisor_id: "demo-b".into(),
                pending_count: 2,
            },
            AdvisorWorkload {
                advisor_id: "demo-a".into(),
                pending_count: 2,
            },
            AdvisorWorkload {
                advisor_id: "demo-c".into(),
                pending_count: 4,
            },
        ])
        .expect("candidate");
        assert_eq!(candidate.advisor_id, "demo-a");
    }
}
