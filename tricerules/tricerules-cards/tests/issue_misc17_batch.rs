//! Registry conformance for the reviewed direct-RON entry/value batch.
//!
//! Runaway Boulder, Hulldrifter, Meteor Sword, Holy Cow, Terror of Mount Velus, Plumecreed Escort,
//! Youthful Valkyrie, Obyra, Dreaming Duelist and Long-Bodied Grey Dog were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and `rulings_uri`
//! fetched 2026-09-22). Governance: CR 115.1a/120.3 (targeted damage), CR 301.5/702.6 (Equipment
//! and equip), CR 603.6a (entry trigger), CR 603.6d (self-excluding permanent-entry watchers),
//! CR 701.22 (scry), CR 702.4 (double strike), CR 702.9 (flying), CR 702.11 (hexproof), CR 702.17
//! (reach), CR 702.29 (cycling), and CR 111.10 (Treasure token).

use tricerules_cards::primitives::{
    AbilityCost, Amount, CastTriggerPlayer, LifeAmount, ObjectContributionKind,
    ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
    StaticAbilityDef, TargetController, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, CounterKind, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn activated<'a>(
    registry: &'a CardRegistry,
    id: &str,
    index: usize,
) -> &'a tricerules_cards::primitives::ActivatedAbilityDef {
    &primary(registry, id).activated_abilities[index]
}

fn crew_minimum(costs: &[AbilityCost]) -> u32 {
    let [AbilityCost::TapPermanents {
        constraint:
            ObjectPaymentConstraint::AggregateMinimum {
                minimum,
                contribution: ObjectContributionKind::CurrentPower,
            },
        exclude_source: true,
        ..
    }] = costs
    else {
        panic!("crew cost shape: {costs:?}");
    };
    *minimum
}

#[test]
fn issue_misc17_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "runaway_boulder",
            "Runaway Boulder",
            "{6}",
            &["Artifact"][..],
            None,
            None,
        ),
        (
            "hulldrifter",
            "Hulldrifter",
            "{3}{U}{U}",
            &["Artifact", "Vehicle"][..],
            Some(3),
            Some(2),
        ),
        (
            "meteor_sword",
            "Meteor Sword",
            "{7}",
            &["Artifact", "Equipment"][..],
            None,
            None,
        ),
        (
            "holy_cow",
            "Holy Cow",
            "{2}{W}",
            &["Creature", "Ox", "Angel"][..],
            Some(2),
            Some(2),
        ),
        (
            "terror_of_mount_velus",
            "Terror of Mount Velus",
            "{5}{R}{R}",
            &["Creature", "Dragon"][..],
            Some(5),
            Some(5),
        ),
        (
            "plumecreed_escort",
            "Plumecreed Escort",
            "{1}{U}",
            &["Creature", "Bird", "Scout"][..],
            Some(2),
            Some(1),
        ),
        (
            "youthful_valkyrie",
            "Youthful Valkyrie",
            "{1}{W}",
            &["Creature", "Angel"][..],
            Some(1),
            Some(3),
        ),
        (
            "obyra,_dreaming_duelist",
            "Obyra, Dreaming Duelist",
            "{U}{B}",
            &["Creature", "Faerie", "Warrior"][..],
            Some(2),
            Some(2),
        ),
        (
            "long-bodied_grey_dog",
            "Long-Bodied Grey Dog",
            "{3}",
            &["Creature", "Dog"][..],
            Some(2),
            Some(2),
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} p/t");
    }

    // Runaway Boulder: Flash, an entry damage trigger, and hand-zone cycling.
    let boulder = primary(registry, "runaway_boulder");
    assert!(boulder.keywords.contains(&Keyword::Flash));
    assert!(
        matches!(&boulder.triggered_abilities[0].effect[0],
            SpellEffectKind::DamageTarget { amount: Amount::Fixed(6), target }
            if target.kind == TargetKind::Creature && target.controller == TargetController::Opponent),
        "{:?}",
        boulder.triggered_abilities[0].effect
    );
    assert!(matches!(
        activated(registry, "runaway_boulder", 0).costs.as_slice(),
        [AbilityCost::Mana(c), AbilityCost::DiscardSelf] if c.to_string() == "{2}"
    ));
    assert!(matches!(
        activated(registry, "runaway_boulder", 0).effect.as_slice(),
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }]
    ));

    // Hulldrifter: flying, entry draw two, Crew 3.
    let hulldrifter = primary(registry, "hulldrifter");
    assert!(hulldrifter.keywords.contains(&Keyword::Flying));
    assert!(matches!(
        &hulldrifter.triggered_abilities[0].effect[0],
        SpellEffectKind::Draw {
            count: Amount::Fixed(2),
            ..
        }
    ));
    assert_eq!(
        crew_minimum(&activated(registry, "hulldrifter", 0).costs),
        3
    );

    // Meteor Sword: entry destroy, +3/+3 attached modifier, equip {3}.
    let sword = primary(registry, "meteor_sword");
    assert!(
        matches!(&sword.triggered_abilities[0].effect[0],
            SpellEffectKind::Destroy { subject }
            if matches!(subject, tricerules_cards::primitives::EffectSubject::Chosen(f)
                if f.kind == TargetKind::AnyPermanent)),
        "{:?}",
        sword.triggered_abilities[0].effect
    );
    assert!(
        matches!(
            &sword.static_abilities[0].definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 3,
                delta_toughness: 3,
                ..
            }
        ),
        "{:?}",
        sword.static_abilities[0].definition
    );

    // Holy Cow: gain 2 life then scry 1.
    let cow = primary(registry, "holy_cow");
    assert!(cow.keywords.contains(&Keyword::Flash));
    assert!(cow.keywords.contains(&Keyword::Flying));
    assert!(matches!(
        &cow.triggered_abilities[0].effect[0],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(2)
        }
    ));
    assert!(matches!(
        &cow.triggered_abilities[0].effect[1],
        SpellEffectKind::Scry {
            count: Amount::Fixed(1)
        }
    ));

    // Terror of Mount Velus: double strike keyword and a mass grant.
    let terror = primary(registry, "terror_of_mount_velus");
    assert!(terror.keywords.contains(&Keyword::DoubleStrike));
    assert!(matches!(
        &terror.triggered_abilities[0].effect[0],
        SpellEffectKind::GrantKeywordsAll { keywords, .. }
            if keywords == &vec![Keyword::DoubleStrike]
    ));

    // Plumecreed Escort: targeted hexproof grant.
    let escort = primary(registry, "plumecreed_escort");
    assert!(matches!(
        &escort.triggered_abilities[0].effect[0],
        SpellEffectKind::GrantKeywords { keywords, .. } if keywords == &vec![Keyword::Hexproof]
    ));

    // Youthful Valkyrie: another Angel you control enters.
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller,
        filter,
        creature_filter,
    } = &primary(registry, "youthful_valkyrie").triggered_abilities[0].trigger
    else {
        panic!("valkyrie entry watcher");
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Creature));
    assert!(filter.exclude_source);
    assert_eq!(
        creature_filter.as_ref().unwrap().required_subtypes,
        vec!["Angel".to_string()]
    );
    assert!(matches!(
        &primary(registry, "youthful_valkyrie").triggered_abilities[0].effect[0],
        SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            ..
        }
    ));

    // Obyra: another Faerie you control enters and each opponent loses 1 life.
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        filter,
        creature_filter,
        ..
    } = &primary(registry, "obyra,_dreaming_duelist").triggered_abilities[0].trigger
    else {
        panic!("obyra entry watcher");
    };
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Creature));
    assert_eq!(
        creature_filter.as_ref().unwrap().required_subtypes,
        vec!["Faerie".to_string()]
    );
    assert_eq!(
        primary(registry, "obyra,_dreaming_duelist").triggered_abilities[0].effect,
        [SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(1),
            who: PlayerRecipient::EachOpponent,
        }]
    );

    // Long-Bodied Grey Dog: a tapped Treasure token on entry.
    let dog = primary(registry, "long-bodied_grey_dog");
    assert!(matches!(
        &dog.triggered_abilities[0].effect[0],
        SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), tapped: true, .. }
            if token == "treasure"
    ));
}

#[test]
fn issue_misc17_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "holy_cow",
            "Holy Cow",
            "62ef9e6c034033b2e7574703062e1d01f80fa2d42007394f8c26039b2a4cf67f",
        ),
        (
            "hulldrifter",
            "Hulldrifter",
            "a0182b60074f929b002948f8ddbba2d4f3591c75000651331c1f33144cc5f2e2",
        ),
        (
            "long-bodied_grey_dog",
            "Long-Bodied Grey Dog",
            "672bab4f177e86b434fb6f78b87238fd12a8f301b88a30e7679ed588bb8ed3ae",
        ),
        (
            "meteor_sword",
            "Meteor Sword",
            "e2ef86a39aff4a943a7572fb636646ae8b2f853361cfa34ee215521b00f3821f",
        ),
        (
            "obyra,_dreaming_duelist",
            "Obyra, Dreaming Duelist",
            "b27b64db85e895a582b7ebb915c4c9235e8d83d44fb9952421cf02136679b207",
        ),
        (
            "plumecreed_escort",
            "Plumecreed Escort",
            "0d5d89de2eb73744fd1667790b07bf6fac066fddf3d2bb2bf61a63366e01bd13",
        ),
        (
            "runaway_boulder",
            "Runaway Boulder",
            "5ad71e477740e2eb652be04556e2010e4c63ab7b1cffde423c8ade0ed65fd1a6",
        ),
        (
            "terror_of_mount_velus",
            "Terror of Mount Velus",
            "7300b9cb61efdffc38da3451169436ed1c57953d6daae07ca5dd987f49aa197b",
        ),
        (
            "youthful_valkyrie",
            "Youthful Valkyrie",
            "8f81f2592fbcaef0c894f826a59fefd9e782d94e9c8dbc4b9ae04e7aa0bfb1a7",
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
