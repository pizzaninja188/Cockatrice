use super::ManaSpendingOrigin;
use tricerules_card_model::primitives::ManaSpendingRestriction;
use tricerules_proto::ruled::v1::PresentationRef;

/// One session-stable restriction identity, its display metadata and optional source receipt.
#[derive(serde::Serialize, Debug, Clone)]
pub struct ManaRestrictionGroup {
    pub restriction: ManaSpendingRestriction,
    pub presentation: Option<PresentationRef>,
    pub origin: Option<ManaSpendingOrigin>,
}

/// Append-only registry: public group IDs are one-based and survive payment rewinds.
#[derive(serde::Serialize, Debug, Clone, Default)]
pub struct ManaRestrictionGroups {
    groups: Vec<ManaRestrictionGroup>,
}

impl ManaRestrictionGroups {
    pub fn get(&self, group_id: u32) -> Option<&ManaRestrictionGroup> {
        self.groups.get(group_id.checked_sub(1)? as usize)
    }

    /// Used by scenario fixtures to vary permissions without changing a group's stable ID.
    pub fn get_mut(&mut self, group_id: u32) -> Option<&mut ManaRestrictionGroup> {
        self.groups.get_mut(group_id.checked_sub(1)? as usize)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &ManaRestrictionGroup)> {
        self.groups
            .iter()
            .enumerate()
            .map(|(index, group)| ((index + 1) as u32, group))
    }

    pub fn push(&mut self, group: ManaRestrictionGroup) -> u32 {
        self.groups.push(group);
        self.groups.len() as u32
    }
}
