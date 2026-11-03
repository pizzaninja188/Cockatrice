use super::{AttachedObjectLki, ObjectId, PlayerId, TokenCopySnapshot};
use std::collections::{BTreeMap, HashMap};
use tricerules_card_model::{Color, CounterKind, Keyword};

/// Facts may be recorded independently. In particular, a known empty collection or unknown
/// power/toughness is different from a fact that has never been captured.
#[derive(serde::Serialize, Debug, Clone, Default)]
struct LastKnownObject {
    tapped: Option<bool>,
    keywords: Option<Vec<Keyword>>,
    colors: Option<Vec<Color>>,
    types: Option<Vec<String>>,
    names: Option<Vec<String>>,
    controller: Option<PlayerId>,
    counters: Option<BTreeMap<CounterKind, u32>>,
    power_toughness: Option<(Option<i64>, Option<i64>)>,
    mana_value: Option<u32>,
    copy_snapshot: Option<TokenCopySnapshot>,
    attached_object: Option<AttachedObjectLki>,
}

/// Facts for exact object incarnations. Narrow updates preserve facts
/// captured at different points in departure, phasing and simultaneous-event processing.
#[derive(serde::Serialize, Debug, Clone, Default)]
pub struct LastKnownInformation {
    objects: HashMap<(ObjectId, u64), LastKnownObject>,
    // Existing source-tapped fallback uses the most recent recorded departure, not the
    // largest generation. Other fact lookups must remain strictly generation-scoped.
    latest_tapped: HashMap<ObjectId, bool>,
}

impl LastKnownInformation {
    pub fn latest_tapped(&self, object_id: &ObjectId) -> Option<&bool> {
        self.latest_tapped.get(object_id)
    }

    pub(crate) fn record_latest_tapped(&mut self, object_id: ObjectId, tapped: bool) {
        self.latest_tapped.insert(object_id, tapped);
    }

    pub fn tapped(&self, key: &(ObjectId, u64)) -> Option<&bool> {
        self.objects.get(key)?.tapped.as_ref()
    }

    pub(crate) fn record_tapped(&mut self, key: (ObjectId, u64), value: bool) {
        self.objects.entry(key).or_default().tapped = Some(value);
    }

    pub fn keywords(&self, key: &(ObjectId, u64)) -> Option<&Vec<Keyword>> {
        self.objects.get(key)?.keywords.as_ref()
    }

    pub(crate) fn record_keywords(&mut self, key: (ObjectId, u64), value: Vec<Keyword>) {
        self.objects.entry(key).or_default().keywords = Some(value);
    }

    pub fn colors(&self, key: &(ObjectId, u64)) -> Option<&Vec<Color>> {
        self.objects.get(key)?.colors.as_ref()
    }

    pub(crate) fn record_colors(&mut self, key: (ObjectId, u64), value: Vec<Color>) {
        self.objects.entry(key).or_default().colors = Some(value);
    }

    pub fn types(&self, key: &(ObjectId, u64)) -> Option<&Vec<String>> {
        self.objects.get(key)?.types.as_ref()
    }

    pub(crate) fn record_types(&mut self, key: (ObjectId, u64), value: Vec<String>) {
        self.objects.entry(key).or_default().types = Some(value);
    }

    pub fn names(&self, key: &(ObjectId, u64)) -> Option<&Vec<String>> {
        self.objects.get(key)?.names.as_ref()
    }

    pub(crate) fn record_names(&mut self, key: (ObjectId, u64), value: Vec<String>) {
        self.objects.entry(key).or_default().names = Some(value);
    }

    pub fn controller(&self, key: &(ObjectId, u64)) -> Option<&PlayerId> {
        self.objects.get(key)?.controller.as_ref()
    }

    pub(crate) fn record_controller(&mut self, key: (ObjectId, u64), value: PlayerId) {
        self.objects.entry(key).or_default().controller = Some(value);
    }

    pub fn counters(&self, key: &(ObjectId, u64)) -> Option<&BTreeMap<CounterKind, u32>> {
        self.objects.get(key)?.counters.as_ref()
    }

    pub(crate) fn record_counters(
        &mut self,
        key: (ObjectId, u64),
        value: BTreeMap<CounterKind, u32>,
    ) {
        self.objects.entry(key).or_default().counters = Some(value);
    }

    pub fn power_toughness(&self, key: &(ObjectId, u64)) -> Option<&(Option<i64>, Option<i64>)> {
        self.objects.get(key)?.power_toughness.as_ref()
    }

    pub(crate) fn record_power_toughness(
        &mut self,
        key: (ObjectId, u64),
        value: (Option<i64>, Option<i64>),
    ) {
        self.objects.entry(key).or_default().power_toughness = Some(value);
    }

    pub fn mana_value(&self, key: &(ObjectId, u64)) -> Option<&u32> {
        self.objects.get(key)?.mana_value.as_ref()
    }

    pub(crate) fn record_mana_value(&mut self, key: (ObjectId, u64), value: u32) {
        self.objects.entry(key).or_default().mana_value = Some(value);
    }

    /// A departed-owner replacement cohort supplies a fallback only when no derived value exists.
    pub(crate) fn record_mana_value_if_absent(&mut self, key: (ObjectId, u64), value: u32) {
        self.objects
            .entry(key)
            .or_default()
            .mana_value
            .get_or_insert(value);
    }

    pub(crate) fn copy_snapshot(&self, key: &(ObjectId, u64)) -> Option<&TokenCopySnapshot> {
        self.objects.get(key)?.copy_snapshot.as_ref()
    }

    pub(crate) fn record_copy_snapshot(&mut self, key: (ObjectId, u64), value: TokenCopySnapshot) {
        self.objects.entry(key).or_default().copy_snapshot = Some(value);
    }

    pub(crate) fn attached_object(&self, key: &(ObjectId, u64)) -> Option<&AttachedObjectLki> {
        self.objects.get(key)?.attached_object.as_ref()
    }

    pub(crate) fn record_attached_object(
        &mut self,
        key: (ObjectId, u64),
        value: AttachedObjectLki,
    ) {
        self.objects.entry(key).or_default().attached_object = Some(value);
    }

    // Missing-LKI fixtures deliberately remove one fact without erasing other qualities.
    #[cfg(test)]
    pub(crate) fn forget_power_toughness(&mut self, key: &(ObjectId, u64)) {
        if let Some(object) = self.objects.get_mut(key) {
            object.power_toughness = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn forget_copy_snapshot(&mut self, key: &(ObjectId, u64)) {
        if let Some(object) = self.objects.get_mut(key) {
            object.copy_snapshot = None;
        }
    }

    /// Keep capture format_version 1 stable while raw state serialization retains the full
    /// private store for command-rejection atomicity checks.
    pub(crate) fn extend_legacy_diagnostic(
        &self,
        fields: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), serde_json::Error> {
        macro_rules! project {
            ($fact:ident, $legacy:literal) => {
                let values: HashMap<_, _> = self
                    .objects
                    .iter()
                    .filter_map(|(key, object)| object.$fact.as_ref().map(|value| (*key, value)))
                    .collect();
                fields.insert($legacy.into(), crate::diagnostic_json::to_value(&values)?);
            };
        }
        fields.insert(
            "last_known_tapped".into(),
            crate::diagnostic_json::to_value(&self.latest_tapped)?,
        );
        project!(tapped, "last_known_tapped_by_generation");
        project!(keywords, "last_known_keywords_by_generation");
        project!(colors, "last_known_colors_by_generation");
        project!(types, "last_known_types_by_generation");
        project!(names, "last_known_names_by_generation");
        project!(controller, "last_known_controller_by_generation");
        project!(counters, "last_known_counters_by_generation");
        project!(power_toughness, "last_known_pt_by_generation");
        project!(mana_value, "last_known_mana_value_by_generation");
        project!(copy_snapshot, "last_known_copy_by_generation");
        project!(attached_object, "last_known_attached_object_by_generation");
        Ok(())
    }
}
