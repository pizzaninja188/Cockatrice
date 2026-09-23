//! Registry mapping for Caught in the Crossfire, Batch 48 of the issue #359 Spree cohort.

use std::collections::BTreeSet;

use tricerules_cards::primitives::{
    Amount, CastCostGroupDef, CastCostOptionDef, ManaCostChoiceKind, RelativePlayerSet,
    SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, ChoiceId, Color, ModeId};

const CAUGHT_IN_THE_CROSSFIRE_FINGERPRINT: &str =
    "41c908e02a9144ff044c062a7f9b60c4a4f1066ffb9e25e5fbb978cde61b6805";

fn mana_cost(group: &CastCostGroupDef, index: usize) -> String {
    let CastCostOptionDef::Mana { kind, cost, .. } = &group.options[index] else {
        panic!("expected a mana option at index {index}");
    };
    assert_eq!(*kind, ManaCostChoiceKind::AdditionalPayment);
    cost.to_string()
}

fn option_id(group: &CastCostGroupDef, index: usize) -> ChoiceId {
    let CastCostOptionDef::Mana { option_id, .. } = &group.options[index] else {
        panic!("expected a mana option at index {index}");
    };
    option_id.clone()
}

#[test]
fn issue_359_caught_in_the_crossfire_maps_both_printed_clauses() {
    let card = CardRegistry::global()
        .get("caught_in_the_crossfire")
        .expect("registered");
    assert_eq!(card.name, "Caught in the Crossfire");
    assert_eq!(
        CardRegistry::global().id_for_name("Caught in the Crossfire"),
        Some("caught_in_the_crossfire")
    );
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "caught_in_the_crossfire");
    assert_eq!(face.mana_cost.to_string(), "{R}{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Red]);

    let [group] = face.cast_cost_groups.as_slice() else {
        panic!("one Spree payment group");
    };
    assert_eq!(group.group_id, ChoiceId::new("spree").unwrap());
    assert_eq!(
        group.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!((group.min, group.max), (1, 2));
    assert_eq!(group.options.len(), 2);
    assert_eq!(option_id(group, 0), ChoiceId::new("outlaws_cost").unwrap());
    assert_eq!(
        option_id(group, 1),
        ChoiceId::new("non_outlaws_cost").unwrap()
    );
    assert_eq!(mana_cost(group, 0), "{1}");
    assert_eq!(mana_cost(group, 1), "{1}");

    let modal = face.modal_spell.as_ref().expect("Spree modes");
    assert_eq!((modal.min_modes, modal.max_modes), (1, 2));
    let [outlaws, non_outlaws] = modal.modes.as_slice() else {
        panic!("two printed Spree modes");
    };
    assert_eq!(outlaws.mode_id, ModeId::new("outlaws").unwrap());
    assert_eq!(
        outlaws.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        outlaws.linked_cast_cost,
        Some(tricerules_cards::primitives::CastCostOptionRef {
            group_id: ChoiceId::new("spree").unwrap(),
            option_id: ChoiceId::new("outlaws_cost").unwrap(),
        })
    );
    assert!(outlaws.targeting.is_none());
    let [SpellEffectKind::DamageAll {
        amount,
        players,
        kind,
    }] = outlaws.effects.as_slice()
    else {
        panic!("outlaw mode is one untargeted DamageAll effect");
    };
    assert_eq!(*amount, Amount::Fixed(2));
    assert_eq!(*players, RelativePlayerSet::All);
    let branches = kind.any_of.as_ref().expect("outlaw subtype disjunction");
    assert_eq!(branches.len(), 5);
    let subtype_union: BTreeSet<&str> = branches
        .iter()
        .map(|branch| {
            assert_eq!(branch.kind, TargetKind::Creature);
            assert_eq!(branch.required_subtypes.len(), 1);
            branch.required_subtypes[0].as_str()
        })
        .collect();
    assert_eq!(
        subtype_union,
        BTreeSet::from(["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"])
    );

    assert_eq!(non_outlaws.mode_id, ModeId::new("non_outlaws").unwrap());
    assert_eq!(
        non_outlaws.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        non_outlaws.linked_cast_cost,
        Some(tricerules_cards::primitives::CastCostOptionRef {
            group_id: ChoiceId::new("spree").unwrap(),
            option_id: ChoiceId::new("non_outlaws_cost").unwrap(),
        })
    );
    assert!(non_outlaws.targeting.is_none());
    let [SpellEffectKind::DamageAll {
        amount,
        players,
        kind,
    }] = non_outlaws.effects.as_slice()
    else {
        panic!("non-outlaw mode is one untargeted DamageAll effect");
    };
    assert_eq!(*amount, Amount::Fixed(2));
    assert_eq!(*players, RelativePlayerSet::All);
    assert_eq!(kind.kind, TargetKind::Creature);
    assert_eq!(
        kind.excluded_subtypes
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"])
    );
}

#[test]
fn issue_359_caught_in_the_crossfire_fingerprint_matches_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let row = fingerprints
        .lines()
        .find(|line| line.starts_with("caught_in_the_crossfire\t"))
        .expect("fingerprint row");
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
    assert_eq!(fields[0], "caught_in_the_crossfire");
    assert_eq!(fields[1], "Caught in the Crossfire");
    assert_eq!(fields[2], "caught_in_the_crossfire");
    assert_eq!(fields[3], "Caught in the Crossfire");
    assert_eq!(fields[4], CAUGHT_IN_THE_CROSSFIRE_FINGERPRINT);
}
