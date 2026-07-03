use crate::config::{SortKey, SortRule};
use serde::{Deserialize, Serialize};
use std::sync::Once;

static SORT_VALUE_WARNING: Once = Once::new();

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct LegacySortValue {
    pub priority_none: Option<u8>,
    pub priority_low: Option<u8>,
    pub priority_medium: Option<u8>,
    pub priority_high: Option<u8>,
    pub no_due_date: Option<u8>,
    pub not_recurring: Option<u8>,
    pub today: Option<u8>,
    pub overdue: Option<u8>,
    pub now: Option<u8>,
    pub deadline_value: Option<u8>,
    pub deadline_days: Option<u8>,
}

pub(crate) fn detect_and_migrate_sort_value(
    sort_value: Option<&LegacySortValue>,
) -> Option<Vec<SortRule>> {
    let sort_value = sort_value?;
    SORT_VALUE_WARNING.call_once(|| {
        eprintln!(
            "Legacy sort_value config detected; migrating to sort_order. \
             Please update your config because sort_value will be removed in a future version."
        );
    });

    let priority = [
        sort_value.priority_none,
        sort_value.priority_low,
        sort_value.priority_medium,
        sort_value.priority_high,
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or_default();

    let mut weighted_keys = vec![
        (SortKey::Priority, priority),
        (SortKey::Overdue, sort_value.overdue.unwrap_or_default()),
        (SortKey::Today, sort_value.today.unwrap_or_default()),
        (SortKey::Now, sort_value.now.unwrap_or_default()),
        (
            SortKey::NoDueDate,
            sort_value.no_due_date.unwrap_or_default(),
        ),
        (
            SortKey::NotRecurring,
            sort_value.not_recurring.unwrap_or_default(),
        ),
        (
            SortKey::Deadline,
            sort_value.deadline_value.unwrap_or_default(),
        ),
    ];

    weighted_keys.sort_by(|(left_key, left_weight), (right_key, right_weight)| {
        right_weight
            .cmp(left_weight)
            .then_with(|| sort_key_default_index(left_key).cmp(&sort_key_default_index(right_key)))
    });

    let mut keys: Vec<SortKey> = weighted_keys.into_iter().map(|(key, _)| key).collect();
    for default_key in SortKey::default_order() {
        if !keys.contains(&default_key) {
            keys.push(default_key);
        }
    }

    Some(
        keys.into_iter()
            .map(SortRule::with_default_direction)
            .collect(),
    )
}

fn sort_key_default_index(key: &SortKey) -> usize {
    SortKey::default_order()
        .iter()
        .position(|default_key| default_key == key)
        .unwrap_or(usize::MAX)
}
