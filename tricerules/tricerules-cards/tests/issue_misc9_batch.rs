//! Registry conformance for the reviewed direct-RON creature-trigger batch.
//!
//! Mary Jane Watson, Disruptor Wanderglyph, Seasoned Consultant, Noggle Robber, Meteor Golem,
//! Vinereap Mentor, Jumbo Cactuar and Undercity Dire Rat were promoted after complete-definition
//! review against the pinned Scryfall snapshot (exact records and `rulings_uri` fetched
//! 2026-09-21). Governance: CR 400.7 (zone changes), CR 508.1/508.3 (attack declaration),
//! CR 603.2c (one trigger per event), CR 603.6 (entry triggers), CR 701.13 (exile), and
//! CR 111.10a-b (Treasure/Food).

use tricerules_cards::primitives::{
    Amount, EffectSubject, GraveyardDestination, GraveyardOwner, PermanentTypeFilter,
    PlayerRecipient, SpellEffectKind, TargetController, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_misc9_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "mary_jane_watson",
            "Mary Jane Watson",
            "{1}{G/W}",
            &["Creature", "Human", "Performer"][..],
            Some(2),
            Some(2),
        ),
        (
            "disruptor_wanderglyph",
            "Disruptor Wanderglyph",
            "{4}",
            &["Artifact", "Creature", "Golem"][..],
            Some(3),
            Some(4),
        ),
        (
            "seasoned_consultant",
            "Seasoned Consultant",
            "{1}{W}",
            &["Creature", "Human", "Detective"][..],
            Some(1),
            Some(3),
        ),
        (
            "noggle_robber",
            "Noggle Robber",
            "{1}{R/G}{R/G}",
            &["Creature", "Noggle", "Rogue"][..],
            Some(3),
            Some(3),
        ),
        (
            "meteor_golem",
            "Meteor Golem",
            "{7}",
            &["Artifact", "Creature", "Golem"][..],
            Some(3),
            Some(3),
        ),
        (
            "vinereap_mentor",
            "Vinereap Mentor",
            "{B}{G}",
            &["Creature", "Squirrel", "Druid"][..],
            Some(3),
            Some(2),
        ),
        (
            "jumbo_cactuar",
            "Jumbo Cactuar",
            "{5}{G}{G}",
            &["Creature", "Plant"][..],
            Some(1),
            Some(7),
        ),
        (
            "undercity_dire_rat",
            "Undercity Dire Rat",
            "{1}{B}",
            &["Creature", "Rat"][..],
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
    assert_eq!(
        primary(registry, "mary_jane_watson").supertypes,
        ["Legendary"],
        "Mary Jane is legendary"
    );

    // Mary Jane Watson: a Spider you control enters -> draw, once each turn.
    let mary = primary(registry, "mary_jane_watson");
    let [mary_trigger] = mary.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller, filter, ..
    } = &mary_trigger.trigger
    else {
        panic!("enters trigger, got {:?}", mary_trigger.trigger);
    };
    assert_eq!(
        *controller,
        tricerules_cards::primitives::CastTriggerPlayer::Controller
    );
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Creature));
    assert_eq!(filter.required_subtypes, ["Spider"]);
    assert!(!filter.exclude_source, "Mary Jane is not a Spider");
    assert_eq!(
        mary_trigger.max_triggers_per_turn,
        Some(1),
        "once each turn is a per-turn cap, not a lifetime cap"
    );
    assert!(!mary_trigger.triggers_only_once);
    assert_eq!(
        mary_trigger.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );

    // Disruptor Wanderglyph: attacks -> exile target card from an opponent's graveyard.
    let glyph = primary(registry, "disruptor_wanderglyph");
    let [glyph_trigger] = glyph.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        glyph_trigger.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    ));
    let SpellEffectKind::MoveGraveyardCards {
        filter,
        destination,
        ..
    } = &glyph_trigger.effect[0]
    else {
        panic!("move graveyard, got {:?}", glyph_trigger.effect[0]);
    };
    assert_eq!(filter.owner, GraveyardOwner::Opponent);
    assert_eq!(*destination, GraveyardDestination::Exile);
    let group = &glyph_trigger.targeting.as_ref().unwrap().groups[0];
    assert_eq!((group.min, group.max), (1, 1));

    // Seasoned Consultant: attacks with three or more creatures -> +2/+0.
    let consultant = primary(registry, "seasoned_consultant");
    let [consultant_trigger] = consultant.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(
        matches!(
            consultant_trigger.trigger,
            TriggerCondition::WheneverControllerAttacks {
                min_attackers: Some(3),
                ..
            }
        ),
        "{:?}",
        consultant_trigger.trigger
    );
    assert!(
        matches!(
            &consultant_trigger.effect[0],
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                subject: EffectSubject::Source,
                ..
            }
        ),
        "{:?}",
        consultant_trigger.effect[0]
    );

    // Noggle Robber / Vinereap Mentor: two independent triggers (enter and dies) sharing a line.
    for (id, token) in [("noggle_robber", "treasure"), ("vinereap_mentor", "food")] {
        let face = primary(registry, id);
        let [enter, dies] = face.triggered_abilities.as_slice() else {
            panic!("{id} two triggers");
        };
        assert!(matches!(
            enter.trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        ));
        assert!(matches!(dies.trigger, TriggerCondition::WhenSelfDies));
        for ability in [enter, dies] {
            assert!(
                matches!(&ability.effect[0], SpellEffectKind::CreateTokens { token: t, count, .. }
                    if t == token && *count == Amount::Fixed(1)),
                "{id} {:?}",
                ability.effect[0]
            );
        }
    }

    // Meteor Golem: enters -> destroy target nonland permanent an opponent controls.
    let golem = primary(registry, "meteor_golem");
    let [golem_trigger] = golem.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        golem_trigger.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    ));
    let SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(target),
    } = &golem_trigger.effect[0]
    else {
        panic!("destroy, got {:?}", golem_trigger.effect[0]);
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    assert_eq!(target.controller, TargetController::Opponent);
    assert!(target
        .excluded_permanent_types
        .contains(&PermanentTypeFilter::Land));
    assert_eq!(
        golem_trigger.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Jumbo Cactuar: attacks -> +9999/+0.
    let cactuar = primary(registry, "jumbo_cactuar");
    let [cactuar_trigger] = cactuar.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        cactuar_trigger.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    ));
    assert!(
        matches!(
            &cactuar_trigger.effect[0],
            SpellEffectKind::PumpTarget {
                power: 9999,
                toughness: 0,
                subject: EffectSubject::Source,
                ..
            }
        ),
        "{:?}",
        cactuar_trigger.effect[0]
    );

    // Undercity Dire Rat: dies -> create a Treasure.
    let rat = primary(registry, "undercity_dire_rat");
    let [rat_trigger] = rat.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        rat_trigger.trigger,
        TriggerCondition::WhenSelfDies
    ));
    assert!(
        matches!(&rat_trigger.effect[0], SpellEffectKind::CreateTokens { token, count, .. }
            if token == "treasure" && *count == Amount::Fixed(1)),
        "{:?}",
        rat_trigger.effect[0]
    );
}

#[test]
fn issue_misc9_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "mary_jane_watson",
            "Mary Jane Watson",
            "e4d8b1942199fcac5c3db11d3ae2dccf58646f5e8847274743e6807d89293527",
        ),
        (
            "disruptor_wanderglyph",
            "Disruptor Wanderglyph",
            "cffd9b90294b8c93a150a9cfb910be91eefe2a71a1b80e74ad1f5309244598d8",
        ),
        (
            "seasoned_consultant",
            "Seasoned Consultant",
            "b9787a04cba75c75ca56632a636a195b5eb169923c80f31bf72112f0e7a7889a",
        ),
        (
            "noggle_robber",
            "Noggle Robber",
            "eba6e5668ccc37ee396ca95b838cb7ace6a890c2fdc34413f7461ba8ac0c5d33",
        ),
        (
            "meteor_golem",
            "Meteor Golem",
            "7e39dc4c57e1f09dcdbea1b2bec472d64ccf4b6dce6bf6f929a342f2afd00d34",
        ),
        (
            "vinereap_mentor",
            "Vinereap Mentor",
            "eaafb15449cdcee8620a2db3df8601a1b419014e1939f437219b5705a48773a8",
        ),
        (
            "jumbo_cactuar",
            "Jumbo Cactuar",
            "4c73e88f5b7d8285b331a1592e6ec2bbd70ac513ed5744a10c3ce349e1dd06e3",
        ),
        (
            "undercity_dire_rat",
            "Undercity Dire Rat",
            "9cdd9bcfd03724e1155da1b287cff90abfb719f98d6b17f6717171a377e4b0e5",
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
