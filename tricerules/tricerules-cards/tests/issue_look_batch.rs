//! Registry conformance for the reviewed look-selection and combat-trigger direct-RON batch.
//!
//! Frontier Seeker, Eclipsed Elf, Eclipsed Boggart, Eclipsed Merrow, Staunch Crewmate,
//! Wild Pack Squad, Might of the Ancestors and Tatyova, Benthic Druid were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and `rulings_uri`
//! fetched 2026-09-21). Governance: CR 603.6a (entry triggers), 701.18/701.23 (look and reveal),
//! 121.1 (draw), 119.3 (life gain), 611.2c and 613 layer 6 (until-end-of-turn keyword grants),
//! 508.1/603.2 (beginning-of-combat triggers), and 603.6a/305 (landfall).

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, EffectSubject, LibraryBottomOrder, PermanentTypeFilter,
    SpellEffectKind, TargetController, TargetFilter, TargetKind, TriggerCondition, ZoneCardFilter,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn creature_you() -> EffectSubject {
    EffectSubject::Chosen(Box::new(TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        ..TargetFilter::default()
    }))
}

fn look_filter(card_id: &str) -> ZoneCardFilter {
    match card_id {
        "frontier_seeker" => ZoneCardFilter {
            any_of: Some(vec![
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    required_subtypes: vec!["Mount".into()],
                    ..ZoneCardFilter::default()
                },
                ZoneCardFilter {
                    required_subtypes: vec!["Plains".into()],
                    ..ZoneCardFilter::default()
                },
            ]),
            ..ZoneCardFilter::default()
        },
        "eclipsed_elf" => subtype_any(&["Elf", "Swamp", "Forest"]),
        "eclipsed_boggart" => subtype_any(&["Goblin", "Swamp", "Mountain"]),
        "eclipsed_merrow" => subtype_any(&["Merfolk", "Plains", "Island"]),
        "staunch_crewmate" => ZoneCardFilter {
            any_of: Some(vec![
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Artifact),
                    ..ZoneCardFilter::default()
                },
                ZoneCardFilter {
                    required_subtypes: vec!["Pirate".into()],
                    ..ZoneCardFilter::default()
                },
            ]),
            ..ZoneCardFilter::default()
        },
        other => panic!("unexpected look card {other}"),
    }
}

fn subtype_any(subtypes: &[&str]) -> ZoneCardFilter {
    ZoneCardFilter {
        any_of: Some(
            subtypes
                .iter()
                .map(|subtype| ZoneCardFilter {
                    required_subtypes: vec![(*subtype).into()],
                    ..ZoneCardFilter::default()
                })
                .collect(),
        ),
        ..ZoneCardFilter::default()
    }
}

#[test]
fn issue_look_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, expected_count, mana, types, power, toughness) in [
        (
            "frontier_seeker",
            5u32,
            "{1}{W}",
            &["Creature", "Human", "Scout"][..],
            2,
            1,
        ),
        (
            "eclipsed_elf",
            4,
            "{B/G}{B/G}{B/G}",
            &["Creature", "Elf", "Scout"][..],
            3,
            2,
        ),
        (
            "eclipsed_boggart",
            4,
            "{B/R}{B/R}{B/R}",
            &["Creature", "Goblin", "Scout"][..],
            2,
            3,
        ),
        (
            "eclipsed_merrow",
            4,
            "{W/U}{W/U}{W/U}",
            &["Creature", "Merfolk", "Scout"][..],
            2,
            3,
        ),
        (
            "staunch_crewmate",
            4,
            "{1}{U}",
            &["Creature", "Human", "Pirate"][..],
            2,
            1,
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal);
        let f = definition.primary_face();
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id} has one trigger");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        let [effect] = trigger.effect.as_slice() else {
            panic!("{id} has one effect");
        };
        let SpellEffectKind::LookChooseToHand {
            count,
            filter: Some(filter),
            min,
            max,
            reveal,
            bottom_order,
        } = effect
        else {
            panic!("{id} looks and chooses, got {effect:?}");
        };
        assert_eq!(*count, expected_count);
        assert_eq!((*min, *max), (0, 1));
        assert!(*reveal);
        assert_eq!(*bottom_order, LibraryBottomOrder::Random);
        assert_eq!(*filter, look_filter(id));
    }

    // Might of the Ancestors: combat pump and vigilance on a controlled creature.
    let might = registry.get("might_of_the_ancestors").expect("registered");
    let f = might.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{2}{W}");
    assert_eq!(f.types, ["Enchantment"]);
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.trigger,
        TriggerCondition::AtBeginningOfCombat {
            player: tricerules_cards::CastTriggerPlayer::Controller
        }
    );
    assert_eq!(
        trigger.effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                scale: None,
                subject: creature_you(),
            },
            SpellEffectKind::GrantKeywords {
                subject: creature_you(),
                keywords: vec![Keyword::Vigilance],
            },
        ]
    );
    assert_eq!(
        trigger.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0, 1]
    );

    // Wild Pack Squad: optional up-to-one combat keyword grant.
    let squad = registry.get("wild_pack_squad").expect("registered");
    let f = squad.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{2}{W}");
    assert_eq!((f.power, f.toughness), (Some(2), Some(3)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            })),
            keywords: vec![Keyword::FirstStrike, Keyword::Vigilance],
        }]
    );
    let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("one group");
    };
    assert_eq!((group.min, group.max), (0, 1));

    // Tatyova: landfall gain one life and draw a card.
    let tatyova = registry.get("tatyova,_benthic_druid").expect("registered");
    assert_eq!(tatyova.name, "Tatyova, Benthic Druid");
    let f = tatyova.primary_face();
    assert_eq!(f.face_id.as_str(), "tatyova_benthic_druid");
    assert_eq!(f.mana_cost.to_string(), "{3}{G}{U}");
    assert!(f.supertypes.iter().any(|s| s == "Legendary"));
    assert_eq!(f.types, ["Creature", "Merfolk", "Druid"]);
    assert_eq!((f.power, f.toughness), (Some(3), Some(3)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller, filter, ..
    } = &trigger.trigger
    else {
        panic!("landfall trigger, got {:?}", trigger.trigger);
    };
    assert_eq!(*controller, tricerules_cards::CastTriggerPlayer::Controller);
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Land));
    assert_eq!(
        trigger.effect,
        [
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1)
            },
            SpellEffectKind::Draw {
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ]
    );
}

#[test]
fn issue_look_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "frontier_seeker",
            "Frontier Seeker",
            "9e92faaa809cbe019890e430edeae5aa2ec75e81cce812b0e14416759d0ca088",
        ),
        (
            "eclipsed_elf",
            "Eclipsed Elf",
            "d36ca94a0ff8feb43d265aa50b5407fa68eaf42048cb553ba2e8d8e0bc053fc3",
        ),
        (
            "eclipsed_boggart",
            "Eclipsed Boggart",
            "86e5808002f8eb9661f52e07ce41d589ea3619e2f75c407434c7392892764243",
        ),
        (
            "eclipsed_merrow",
            "Eclipsed Merrow",
            "bf6736e6aa306ece7734d66d4aaebf43924411aeb770d2aec45dbe4d4a0c4663",
        ),
        (
            "staunch_crewmate",
            "Staunch Crewmate",
            "83b6155a1975bbee67e9427de5045c4671176547bd06b05fef143057976bae41",
        ),
        (
            "wild_pack_squad",
            "Wild Pack Squad",
            "b028249fbbc66d46669805758769feb68bc429b283324c07d292f255a5e8b853",
        ),
        (
            "might_of_the_ancestors",
            "Might of the Ancestors",
            "0c57d841a2a56535139761f0d9b9f5139262926271984a18381c9b29b1f236cb",
        ),
        (
            "tatyova,_benthic_druid",
            "Tatyova, Benthic Druid",
            "c923e79654c8808072d9cd85cf4cceb84024847c8c64f2a5928281caa4d9c057",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
