//! Registry conformance for the reviewed direct-RON dies-value batch.
//!
//! Driver of the Dead, Harried Spearguard, Callous Inspector, Edge Rover, Sizzling Changeling and
//! Miner's Guidewing were promoted after complete-definition review against the pinned Scryfall
//! snapshot (exact records and `rulings_uri` fetched 2026-09-22). Governance: CR 603.6c (dies
//! triggers), CR 111.10 (Rat, Clue, and Lander tokens), CR 115.3 (targets), CR 120.3 (the damage
//! the source deals to its controller), CR 400.7/608.2b (a returned card is a new object), CR 701.8
//! (the graveyard return), CR 701.44 (Explore), and CR 702.73 (Changeling).

use tricerules_cards::primitives::{
    Amount, EffectSubject, GraveyardDestination, PlayerRecipient, SpellEffectKind, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{CardRegistry, CharacteristicDefiningAbility, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_misc22_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "driver_of_the_dead",
            "Driver of the Dead",
            "{3}{B}",
            &["Creature", "Vampire"][..],
            Some(3),
            Some(2),
        ),
        (
            "harried_spearguard",
            "Harried Spearguard",
            "{R}",
            &["Creature", "Human", "Soldier"][..],
            Some(1),
            Some(1),
        ),
        (
            "callous_inspector",
            "Callous Inspector",
            "{B}",
            &["Creature", "Human", "Soldier"][..],
            Some(1),
            Some(1),
        ),
        (
            "edge_rover",
            "Edge Rover",
            "{G}",
            &["Artifact", "Creature", "Robot", "Scout"][..],
            Some(2),
            Some(2),
        ),
        (
            "sizzling_changeling",
            "Sizzling Changeling",
            "{2}{R}",
            &["Creature", "Shapeshifter"][..],
            Some(3),
            Some(2),
        ),
        (
            "miners_guidewing",
            "Miner's Guidewing",
            "{W}",
            &["Creature", "Bird"][..],
            Some(1),
            Some(1),
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} p/t");
        assert_eq!(
            f.triggered_abilities[0].trigger,
            TriggerCondition::WhenSelfDies,
            "{id}"
        );
    }

    // Driver of the Dead: a dies trigger returns a mana-value-2-or-less creature card.
    let driver = primary(registry, "driver_of_the_dead");
    let [SpellEffectKind::MoveGraveyardCards {
        filter,
        destination: GraveyardDestination::Battlefield { .. },
        ..
    }] = driver.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", driver.triggered_abilities[0].effect);
    };
    assert_eq!(filter.card.as_ref().and_then(|c| c.max_mana_value), Some(2));

    // Harried Spearguard: haste and a banned-blocker Rat token.
    let guard = primary(registry, "harried_spearguard");
    assert!(guard.keywords.contains(&Keyword::Haste));
    assert!(matches!(
        &guard.triggered_abilities[0].effect[0],
        SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), .. }
            if token == "rat_b_1_1_cant_block"
    ));

    // Callous Inspector: menace, 1 damage to the controller, and a Clue token.
    let inspector = primary(registry, "callous_inspector");
    assert!(inspector.keywords.contains(&Keyword::Menace));
    let inspector_effect = &inspector.triggered_abilities[0].effect;
    assert!(matches!(
        &inspector_effect[0],
        SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(1),
            who: PlayerRecipient::Controller
        }
    ));
    assert!(matches!(
        &inspector_effect[1],
        SpellEffectKind::CreateTokens { token, .. } if token == "clue"
    ));

    // Edge Rover: reach and a Lander token for each player.
    let rover = primary(registry, "edge_rover");
    assert!(rover.keywords.contains(&Keyword::Reach));
    assert!(matches!(
        &rover.triggered_abilities[0].effect[0],
        SpellEffectKind::CreateTokens { token, who: PlayerRecipient::EachPlayer, .. }
            if token == "lander"
    ));

    // Sizzling Changeling: changeling CDA and the exile-top play permission.
    let changeling = primary(registry, "sizzling_changeling");
    assert!(matches!(
        &changeling.characteristic_defining_abilities[0].definition,
        CharacteristicDefiningAbility::Changeling
    ));
    assert!(matches!(
        &changeling.triggered_abilities[0].effect[0],
        SpellEffectKind::ExileTopWithPlayPermission {
            player: PlayerRecipient::Controller,
            ..
        }
    ));

    // Miner's Guidewing: flying, vigilance, and a targeted Explore.
    let guidewing = primary(registry, "miners_guidewing");
    assert!(guidewing.keywords.contains(&Keyword::Flying));
    assert!(guidewing.keywords.contains(&Keyword::Vigilance));
    let [SpellEffectKind::Explore { subject }] = guidewing.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", guidewing.triggered_abilities[0].effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(
        filter.controller,
        tricerules_cards::primitives::TargetController::You
    );
}

#[test]
fn issue_misc22_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "driver_of_the_dead",
            "Driver of the Dead",
            "b2a2aaff9ed23fdcd311170c94421786d81f9bb2b53f314e611b2491129db892",
        ),
        (
            "harried_spearguard",
            "Harried Spearguard",
            "5764cc26f084761fd0eb67912d1c8f4b1df565c0669f0dc509e5f6eb2ebb71a0",
        ),
        (
            "callous_inspector",
            "Callous Inspector",
            "2aa603574666c7590a58d7c48099d3f94a906111f1623e7fb28036b605e9b123",
        ),
        (
            "edge_rover",
            "Edge Rover",
            "8caeca50e8038bc3f1942d4dc5584c600187f67ea9128d0e397044cd03794cb2",
        ),
        (
            "sizzling_changeling",
            "Sizzling Changeling",
            "8a69081074d7cecd09657d9f7d4ebeee7963e142f75d310b0dbe73713b5dbc69",
        ),
        (
            "miners_guidewing",
            "Miner's Guidewing",
            "4e92e9e28a69b4067e983c881e06c4e25fcbc1fc50f3864f494b09f485400ac3",
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
