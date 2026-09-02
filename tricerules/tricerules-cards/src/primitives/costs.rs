//! Costs paid to activate abilities.

use super::{BattlefieldPermanentFilter, GameCondition, TargetFilter, TargetKind, ZoneCardFilter};
use crate::mana::ManaCost;
use crate::{choice_fallback, AbilityPresentation, ChoiceId};
use serde::{Deserialize, Serialize};

/// Engine-computed quantity contributed by one public-zone object payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectContributionKind {
    ManaValue,
    CurrentPower,
}

/// Typed cardinality for a selected-object cost. This deliberately models only the two
/// payment shapes used by object costs rather than a general constraint language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectPaymentConstraint {
    ExactCount(u32),
    AggregateMinimum {
        minimum: u32,
        contribution: ObjectContributionKind,
    },
}

impl ObjectPaymentConstraint {
    pub fn exact_count(self) -> Option<u32> {
        match self {
            Self::ExactCount(count) => Some(count),
            Self::AggregateMinimum { .. } => None,
        }
    }

    pub fn aggregate_minimum(self) -> Option<(u32, ObjectContributionKind)> {
        match self {
            Self::AggregateMinimum {
                minimum,
                contribution,
            } => Some((minimum, contribution)),
            Self::ExactCount(_) => None,
        }
    }

    pub(crate) fn validate_for(
        self,
        expected: ObjectContributionKind,
        label: &str,
    ) -> Result<(), String> {
        match self {
            Self::ExactCount(0) | Self::AggregateMinimum { minimum: 0, .. } => {
                Err(format!("{label} cost requires a positive constraint"))
            }
            Self::AggregateMinimum { contribution, .. } if contribution != expected => Err(
                format!("{label} cost uses an incompatible aggregate contribution"),
            ),
            _ => Ok(()),
        }
    }
}

/// Which battlefield object supplies counters for an activated-ability cost. Source preserves
/// the historical Walking Ballista/Brambleback Brute shape; selected permanents are an
/// engine-authored, non-targeting choice such as Ray Fillet or Sage of Fables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CounterRemovalPaymentSource {
    #[default]
    Source,
    SelectedPermanent(Box<TargetFilter>),
}

fn counter_payment_filter_is_permanent(filter: &TargetFilter) -> bool {
    filter.any_of.as_ref().map_or_else(
        || matches!(filter.kind, TargetKind::Creature | TargetKind::AnyPermanent),
        |branches| branches.iter().all(counter_payment_filter_is_permanent),
    )
}

fn counter_payment_filter_has_context_free_controller(filter: &TargetFilter) -> bool {
    filter.any_of.as_ref().map_or_else(
        || !matches!(filter.controller, super::TargetController::DefendingPlayer),
        |branches| {
            branches
                .iter()
                .all(counter_payment_filter_has_context_free_controller)
        },
    )
}

/// Cost to activate an activated ability (CR 602). Shared by every activated ability,
/// including mana abilities: an ability is classified as a mana ability (CR 605.1a) by its
/// *effect* being [`SpellEffectKind::ProduceMana`], not by its cost — so a `{T}` land, a
/// `{1}, {T}` filter land, and a sacrifice-for-mana rock all use these same cost kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityCost {
    /// Brambleback Brute removes any one kind; Walking Ballista removes fixed +1/+1 counters.
    /// None permits exactly one counter of any present kind and is valid only for source payment.
    RemoveCounters {
        counter: Option<super::CounterKind>,
        count: u32,
        #[serde(default)]
        payment_source: CounterRemovalPaymentSource,
    },
    /// CR 701.68: Gristle Glutton and Spiral into Solitude put all counters on one creature.
    Blight { count: u32 },
    /// CR 606.4: add (positive), remove (negative), or leave unchanged (zero) loyalty counters
    /// as the cost of activating a planeswalker's loyalty ability.
    Loyalty(i32),
    /// {T}: tap the source permanent.
    Tap,
    /// Tap exactly `count` untapped permanents the activating player controls that match
    /// `filter`. This is a selection cost, not targeting, and summoning sickness does not apply
    /// unless the selected object is also paying a separate `{T}` source cost.
    TapPermanents {
        constraint: ObjectPaymentConstraint,
        filter: TargetFilter,
        #[serde(default)]
        exclude_source: bool,
    },
    /// Pay mana (e.g. `"{4}"`, `"{2}{R}"`). Same brace syntax as `CardDefinition.mana_cost`.
    Mana(ManaCost),
    /// CR 701.67: Foggy Swamp Vinebender and Watery Grasp permit untapped artifacts or
    /// creatures to pay the generic component. Shared with Waterbending Lesson at resolution.
    Waterbend(ManaCost),
    /// Discard one card chosen from the activating player's hand.
    Discard,
    /// Discard the source object itself from its owner's hand (cycling and typecycling).
    DiscardSelf,
    /// Exile the source object itself from its owner's graveyard (renew).
    ExileSelf,
    /// Sacrifice the source permanent as cost (e.g. Bottle Gnomes).
    SacrificeSelf,
    /// Sacrifice another or the source permanent when it matches the filter (e.g. Portcullis
    /// Vine's "Sacrifice a creature with defender"). This is selection, not targeting: shroud
    /// and hexproof do not apply.
    SacrificePermanent { filter: TargetFilter },
    /// Exile exactly `count` matching cards from the activating player's graveyard. This is a
    /// selection cost, not targeting. Say Its Name excludes its own source object; Bearscape and
    /// Grim Lavamancer reuse the same bounded graveyard-cohort payment without source exclusion.
    ExileGraveyardCards {
        constraint: ObjectPaymentConstraint,
        filter: ZoneCardFilter,
        #[serde(default)]
        exclude_source: bool,
    },
}

impl AbilityCost {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Waterbend(cost)
                if cost.pips.iter().any(|p| matches!(p, crate::ManaSymbol::X)) =>
            {
                Err("Waterbend activation cost cannot contain unbound X".into())
            }
            Self::RemoveCounters {
                counter,
                count,
                payment_source,
            } => {
                if *count == 0 || (counter.is_none() && *count != 1) {
                    return Err(
                        "counter cost requires a positive fixed count or any one counter".into(),
                    );
                }
                if let CounterRemovalPaymentSource::SelectedPermanent(filter) = payment_source {
                    if counter.is_none() {
                        return Err(
                            "selected-permanent counter removal requires a fixed counter kind"
                                .into(),
                        );
                    }
                    if !counter_payment_filter_is_permanent(filter) {
                        return Err(
                            "selected counter payment requires Creature or AnyPermanent filters"
                                .into(),
                        );
                    }
                    if !counter_payment_filter_has_context_free_controller(filter) {
                        return Err(
                            "selected counter payment cannot use defending-player context".into(),
                        );
                    }
                    filter.validate_characteristic_constraints()?;
                }
                if let Some(counter) = counter {
                    counter.validate()?;
                }
                Ok(())
            }
            Self::Blight { count: 0 } => Err("blight cost requires a positive count".into()),
            Self::TapPermanents {
                constraint, filter, ..
            } => {
                constraint.validate_for(ObjectContributionKind::CurrentPower, "permanent tap")?;
                filter.validate_characteristic_constraints()
            }
            Self::ExileGraveyardCards {
                constraint, filter, ..
            } => {
                constraint
                    .validate_for(ObjectContributionKind::ManaValue, "graveyard-card exile")?;
                if constraint.aggregate_minimum().is_some() && filter == &ZoneCardFilter::default()
                {
                    Ok(())
                } else {
                    filter.validate()
                }
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod waterbend_tests {
    #[test]
    fn waterbend_resolution_cost_is_data_authored() {
        let cost = ron::from_str::<super::super::ResolutionCost>(r#"Waterbend("{2}")"#);
        assert!(
            cost.is_ok(),
            "Waterbending Lesson needs a typed resolution cost: {cost:?}"
        );
    }
}

/// Mandatory additional costs paid while casting a spell (CR 118.8, 601.2f-h).
///
/// These components deliberately exclude mana: the face's normal or alternate mana cost remains
/// the single mana component, while this ordered list supplies authored nonmana choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdditionalCost {
    /// Mandatory Blight payment, sharing the cast-option and activated-cost operation.
    Blight { count: u32 },
    /// Discard one other card chosen from the caster's hand.
    DiscardCard,
    /// Sacrifice one permanent the caster controls that matches `filter`.
    SacrificePermanent { filter: TargetFilter },
    /// Tap exactly `count` matching untapped permanents the caster controls.
    TapPermanents {
        constraint: ObjectPaymentConstraint,
        filter: TargetFilter,
        #[serde(default)]
        exclude_source: bool,
    },
    /// Exile selected cards from the caster's graveyard, either by exact count or by an
    /// engine-computed mana-value minimum (Collect Evidence).
    ExileGraveyardCards {
        constraint: ObjectPaymentConstraint,
        filter: ZoneCardFilter,
        #[serde(default)]
        exclude_source: bool,
    },
}

/// One announced cast-time cost choice group (CR 601.2b). Kicker and behold share this
/// vocabulary: both record an option before targets are chosen, then let later rules text query
/// the stable receipt instead of re-examining the paid object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastCostGroupDef {
    pub group_id: ChoiceId,
    pub presentation: AbilityPresentation,
    #[serde(default)]
    pub min: u32,
    #[serde(default = "default_one")]
    pub max: u32,
    pub options: Vec<CastCostOptionDef>,
}

fn default_one() -> u32 {
    1
}

/// One mutually distinguishable option in a cast-cost group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManaCostChoiceKind {
    AdditionalPayment,
    Kicker,
}

/// Rules identity of an object paid as an announced optional or additional cast cost. The
/// distinction is observable by cards such as Agent Maria Hill and by kicker-conditioned text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectCastCostKind {
    AdditionalPayment,
    Kicker,
    Teamwork,
}

/// One mutually distinguishable option in a cast-cost group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CastCostOptionDef {
    /// Cinder Strike's optional payment and Wild Unraveling's Blight-or-mana group.
    Blight {
        option_id: ChoiceId,
        presentation: AbilityPresentation,
        count: u32,
    },
    /// Pay this additional mana as part of the spell's single total cost. Grow from the Ashes and
    /// Gnarlid Colony use this for kicker.
    Mana {
        option_id: ChoiceId,
        presentation: AbilityPresentation,
        kind: ManaCostChoiceKind,
        cost: ManaCost,
    },
    /// CR 701.4: reveal one matching hand card or choose one matching permanent you control.
    /// Caustic Exhale and Osseous Exhale use the same typed selection.
    Behold {
        option_id: ChoiceId,
        presentation: AbilityPresentation,
        hand_filter: ZoneCardFilter,
        permanent_filter: Box<TargetFilter>,
    },
    /// Tap a generation-bound cohort of untapped permanents as an announced cast cost. Teamwork
    /// uses an aggregate CurrentPower constraint; exact-count variants support future mechanics.
    TapPermanents {
        option_id: ChoiceId,
        presentation: AbilityPresentation,
        kind: ObjectCastCostKind,
        constraint: ObjectPaymentConstraint,
        filter: Box<TargetFilter>,
    },
    /// Sacrifice one matching permanent as an announced cost, including kicker costs.
    SacrificePermanent {
        option_id: ChoiceId,
        presentation: AbilityPresentation,
        kind: ObjectCastCostKind,
        filter: Box<TargetFilter>,
    },
}

impl CastCostGroupDef {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.group_id.validate()?;
        self.presentation.validate()?;
        if self.options.is_empty()
            || self.min > self.max
            || self.max == 0
            || self.max as usize > self.options.len()
        {
            return Err(
                "cast cost group requires min <= max <= distinct option count and max > 0".into(),
            );
        }
        let mut option_ids = std::collections::HashSet::new();
        for option in &self.options {
            let option_id = option.option_id();
            option_id.validate()?;
            option.presentation().validate()?;
            if !option_ids.insert(option_id.as_str()) {
                return Err(format!("duplicate cast cost option id '{option_id}'"));
            }
            match option {
                CastCostOptionDef::Blight { count, .. } => {
                    if *count == 0 {
                        return Err("blight option requires a positive count".into());
                    }
                }
                CastCostOptionDef::Mana { cost, .. } => {
                    if cost.is_empty() {
                        return Err("cast mana option requires a nonempty cost".into());
                    }
                }
                CastCostOptionDef::Behold {
                    hand_filter,
                    permanent_filter,
                    ..
                } => {
                    hand_filter.validate()?;
                    permanent_filter.validate_target_constraints()?;
                }
                CastCostOptionDef::TapPermanents {
                    constraint, filter, ..
                } => {
                    constraint.validate_for(ObjectContributionKind::CurrentPower, "cast tap")?;
                    filter.validate_target_constraints()?;
                }
                CastCostOptionDef::SacrificePermanent { filter, .. } => {
                    filter.validate_target_constraints()?;
                }
            }
        }
        Ok(())
    }

    pub fn fallback_prompt(&self) -> String {
        choice_fallback("Choose cast cost", &self.group_id)
    }
}

impl CastCostOptionDef {
    pub fn option_id(&self) -> &ChoiceId {
        match self {
            Self::Blight { option_id, .. }
            | Self::Mana { option_id, .. }
            | Self::Behold { option_id, .. }
            | Self::TapPermanents { option_id, .. }
            | Self::SacrificePermanent { option_id, .. } => option_id,
        }
    }

    pub fn presentation(&self) -> &AbilityPresentation {
        match self {
            Self::Blight { presentation, .. }
            | Self::Mana { presentation, .. }
            | Self::Behold { presentation, .. }
            | Self::TapPermanents { presentation, .. }
            | Self::SacrificePermanent { presentation, .. } => presentation,
        }
    }

    pub fn fallback_label(&self) -> String {
        match self {
            Self::Blight { count, .. } => format!("Blight {count}"),
            Self::Mana { kind, cost, .. } => match kind {
                ManaCostChoiceKind::AdditionalPayment => format!("Pay {cost}"),
                ManaCostChoiceKind::Kicker => format!("Kicker {cost}"),
            },
            Self::Behold { option_id, .. } => choice_fallback("Behold", option_id),
            Self::TapPermanents { kind, .. } => match kind {
                ObjectCastCostKind::Teamwork => "Pay teamwork cost".into(),
                ObjectCastCostKind::Kicker => "Pay tap kicker cost".into(),
                ObjectCastCostKind::AdditionalPayment => "Tap permanents".into(),
            },
            Self::SacrificePermanent { kind, .. } => match kind {
                ObjectCastCostKind::Kicker => "Kicker - sacrifice a permanent".into(),
                ObjectCastCostKind::Teamwork => "Pay teamwork cost".into(),
                ObjectCastCostKind::AdditionalPayment => "Sacrifice a permanent".into(),
            },
        }
    }
}

/// Stable authored identity of one option in one cast-cost group. Wire commands continue to use
/// positional indices as batch coordinates, while card definitions and durable receipts use this
/// identity so reordering unrelated options cannot change rules behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastCostOptionRef {
    pub group_id: ChoiceId,
    pub option_id: ChoiceId,
}

impl CastCostOptionRef {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.group_id.validate()?;
        self.option_id.validate()
    }
}

/// A typed linked condition over an announced cast-cost option. `expected_selected = false`
/// supports the inverse branch without requiring clients or effects to infer a default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastCostReceiptCondition {
    pub group_id: ChoiceId,
    pub option_id: ChoiceId,
    #[serde(default = "default_true")]
    pub expected_selected: bool,
}

fn default_true() -> bool {
    true
}

/// A face-authored modifier applied while determining that spell's total cost (CR 601.2f).
///
/// The condition vocabulary is shared with triggers, activated abilities, and conditional
/// effects so card data never reimplements public game-state queries. Winged Words uses a
/// controller-owned flying-creature condition; Purple Worm and Bone Picker can reuse the same
/// modifier with the existing creature-deaths-this-turn condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellCostModifier {
    /// Automatic public quantities, e.g. Witchstalker Frenzy's attackers and affinity cohorts.
    GenericReduction { amount: super::Amount },
    /// Reduce only the generic component of the selected normal or alternative mana cost.
    ConditionalGenericReduction {
        amount: u32,
        condition: GameCondition,
    },
    /// Reduce this spell's generic cost once when an announced target matches `filter`.
    /// Luminous Rebuke inspects battlefield creatures; No One Left Behind inspects graveyard cards.
    TargetMatchGenericReduction {
        amount: u32,
        filter: super::TargetMatchFilter,
    },
    /// Reduce this spell's generic cost for each matching battlefield permanent. Affinity for
    /// creatures and future affinity-style cohorts share this counted form.
    BattlefieldCountGenericReduction {
        amount_per_match: u32,
        filter: BattlefieldPermanentFilter,
        #[serde(default)]
        aggregate: super::BattlefieldAggregate,
    },
}

/// One modifier applied while determining an activated ability's mana cost (CR 602.2b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedCostModifier {
    ConditionalGenericReduction {
        amount: u32,
        condition: GameCondition,
    },
    /// Power-up (Ninja of the Hand, Ultron Drone): while the condition holds, reduce the
    /// ability's mana cost by the source permanent's fixed generic, colored, and colorless
    /// mana symbols. Flexible source symbols remain unsupported until the payment contract can
    /// publish the reduction choices required by CR 118.7e-f.
    ConditionalSourceManaCostReduction { condition: GameCondition },
}

impl ActivatedCostModifier {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::ConditionalGenericReduction { amount, condition } => {
                if *amount == 0 {
                    return Err("activated generic cost reduction must be nonzero".into());
                }
                condition.validate_live()
            }
            Self::ConditionalSourceManaCostReduction { condition } => condition.validate_live(),
        }
    }
}

impl SpellCostModifier {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::GenericReduction { amount } => amount.validate_cost(false),
            Self::ConditionalGenericReduction { amount, condition } => {
                if *amount == 0 {
                    return Err("conditional generic cost reduction must be nonzero".into());
                }
                condition.validate_live()
            }
            Self::TargetMatchGenericReduction { amount, filter } => {
                if *amount == 0 {
                    return Err("target-matching generic cost reduction must be nonzero".into());
                }
                filter.validate()
            }
            Self::BattlefieldCountGenericReduction {
                amount_per_match,
                filter,
                ..
            } => {
                if *amount_per_match == 0 {
                    return Err("battlefield-count generic cost reduction must be nonzero".into());
                }
                filter.validate()
            }
        }
    }
}
