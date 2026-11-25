//! Registry evidence for issue #434: Adept Watershaper, The Wandering Rescuer and The Seriema.
//!
//! Oracle checked 2026-11-25 against the pinned Scryfall `oracle_cards` snapshot; Scryfall rulings
//! could not be fetched (HTTP 403). Each card's last clause is a layer-6 keyword grant (CR 613.1f)
//! over "other tapped [legendary] creatures you control": tapped status (CR 110.5) and the
//! legendary supertype (CR 205.4) live in the shared `matching` predicate. The Seriema's anthem is
//! in its 7+ striation (CR 722.2), so it carries the same charge-counter condition as its Flying
//! animation.

use crate::common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CreatureScopeController, CreatureScopeFilter, GameCondition, ObjectFilter, SpellEffectKind,
    StaticAbilityDef, TargetKind, TriggerCondition,
};
use tricerules_cards::{CounterKind, Keyword};

fn other_tapped_creatures_you_control(supertypes: &[&str]) -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(CreatureScopeController::YouControl),
        exclude_self: true,
        matching: Box::new(ObjectFilter {
            kind: TargetKind::Creature,
            tapped: Some(true),
            required_supertypes: supertypes.iter().map(|value| (*value).to_owned()).collect(),
            ..ObjectFilter::default()
        }),
    }
}

fn seven_charge() -> GameCondition {
    GameCondition::SourceCounterCount {
        counter: CounterKind::Charge,
        min: Some(7),
        max: None,
    }
}

#[test]
fn issue_434_registers_the_three_complete_identities() {
    let registry = tricerules_cards::registry::global();
    for (id, name, mana_cost, types, keywords, power_toughness, legendary) in [
        (
            "adept_watershaper",
            "Adept Watershaper",
            "{2}{W}",
            &["Creature", "Merfolk", "Cleric"][..],
            &[][..],
            Some((3, 4)),
            false,
        ),
        (
            "the_wandering_rescuer",
            "The Wandering Rescuer",
            "{3}{W}{W}",
            &["Creature", "Human", "Samurai", "Noble"][..],
            &[Keyword::Flash, Keyword::Convoke, Keyword::DoubleStrike][..],
            Some((3, 4)),
            true,
        ),
        (
            "the_seriema",
            "The Seriema",
            "{1}{W}{W}",
            &["Artifact", "Spacecraft"][..],
            &[][..],
            Some((5, 5)),
            true,
        ),
    ] {
        assert_eq!(registry.id_for_name(name), Some(id), "{name}");
        let face = FaceExpectation {
            id,
            name,
            face_id: id,
            mana_cost,
            types,
            keywords,
            power_toughness,
        }
        .check();
        assert_eq!(
            face.supertypes.iter().any(|value| value == "Legendary"),
            legendary,
            "{id}"
        );
    }
}

#[test]
fn issue_434_tapped_anthems_use_the_shared_tapped_predicate() {
    for (id, keyword) in [
        ("adept_watershaper", Keyword::Indestructible),
        ("the_wandering_rescuer", Keyword::Hexproof),
    ] {
        let face = tricerules_cards::registry::global()
            .get(id)
            .unwrap()
            .primary_face();
        let [ability] = face.static_abilities.as_slice() else {
            panic!("{id} has exactly one static ability");
        };
        assert_eq!(
            ability.definition,
            StaticAbilityDef::AnthemKeyword {
                filter: other_tapped_creatures_you_control(&[]),
                condition: None,
                keyword,
            },
            "{id}"
        );
        assert!(face.triggered_abilities.is_empty() && face.activated_abilities.is_empty());
    }
}

#[test]
fn issue_434_the_seriema_gates_flying_and_its_legendary_anthem_at_seven() {
    let face = tricerules_cards::registry::global()
        .get("the_seriema")
        .unwrap()
        .primary_face();

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("The Seriema has exactly one entry trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::SearchLibrary { filter: Some(filter), reveal: true, .. }]
            if filter.required_supertypes == ["Legendary"]
    ));
    assert_eq!(face.activated_abilities.len(), 1, "Station");

    let [animation, anthem] = face.static_abilities.as_slice() else {
        panic!("The Seriema has the 7+ animation and anthem");
    };
    assert!(matches!(
        &animation.definition,
        StaticAbilityDef::ConditionalSelfModifier {
            condition,
            base_power: Some(5),
            base_toughness: Some(5),
            keywords,
            ..
        } if *condition == seven_charge() && keywords == &[Keyword::Flying]
    ));
    assert_eq!(
        anthem.definition,
        StaticAbilityDef::AnthemKeyword {
            filter: other_tapped_creatures_you_control(&["Legendary"]),
            condition: Some(seven_charge()),
            keyword: Keyword::Indestructible,
        }
    );
}
