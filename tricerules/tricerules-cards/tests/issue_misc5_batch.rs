//! Registry conformance for the reviewed direct-RON batch of simple Standard effects.
//!
//! Acolyte of Aclazotz, Bartolome del Presidio, Captain Storm, Cosmium Raider, Ashiok's Reaper,
//! Battlesong Berserker, Clammy Prowler and Bake into a Pie were promoted
//! after complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-21). Governance: CR 115 (targets), 118.12 (additional costs),
//! 119/120 (life), 122.1 (counters), 301.5 (Food), 508 (attack triggers), 602 (activated
//! abilities), 603.6 (entry/leaves triggers), 608.2b (target revalidation), 609.3 (intervening
//! if), and 611.2c (until end of turn).

use tricerules_cards::primitives::{
    AbilityCost, Amount, CastTriggerPlayer, CombatRestriction, CombatRestrictionScope, CombatRole,
    CounterKind, EffectSubject, EventZone, LifeAmount, PermanentTypeFilter, PlayerRecipient,
    SpellEffectKind, TargetController, TargetKind, TargetObjectExclusion, TriggerCondition,
    ZoneEventDestination,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_misc5_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "acolyte_of_aclazotz",
            "Acolyte of Aclazotz",
            "{2}{B}",
            &["Creature", "Vampire", "Cleric"][..],
            Some(1),
            Some(4),
        ),
        (
            "bartolome_del_presidio",
            "Bartolomé del Presidio",
            "{W}{B}",
            &["Creature", "Vampire", "Knight"][..],
            Some(2),
            Some(1),
        ),
        (
            "captain_storm,_cosmium_raider",
            "Captain Storm, Cosmium Raider",
            "{U}{R}",
            &["Creature", "Human", "Pirate"][..],
            Some(2),
            Some(2),
        ),
        (
            "ashioks_reaper",
            "Ashiok's Reaper",
            "{3}{B}",
            &["Creature", "Nightmare"][..],
            Some(3),
            Some(3),
        ),
        (
            "battlesong_berserker",
            "Battlesong Berserker",
            "{3}{R}",
            &["Creature", "Human", "Berserker"][..],
            Some(3),
            Some(4),
        ),
        (
            "clammy_prowler",
            "Clammy Prowler",
            "{3}{U}",
            &["Enchantment", "Creature", "Horror"][..],
            Some(2),
            Some(5),
        ),
        (
            "bake_into_a_pie",
            "Bake into a Pie",
            "{2}{B}{B}",
            &["Instant"][..],
            None,
            None,
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

    for (id, supertypes) in [
        ("bartolome_del_presidio", &["Legendary"][..]),
        ("captain_storm,_cosmium_raider", &["Legendary"][..]),
    ] {
        assert_eq!(
            primary(registry, id).supertypes,
            supertypes,
            "{id} supertypes"
        );
    }

    // Acolyte of Aclazotz: {T}, sacrifice another creature or artifact: each opponent loses 1, you gain 1.
    let acolyte = primary(registry, "acolyte_of_aclazotz");
    let [acolyte_ability] = acolyte.activated_abilities.as_slice() else {
        panic!("one ability");
    };
    let [AbilityCost::Tap, AbilityCost::SacrificePermanent { filter }] =
        acolyte_ability.costs.as_slice()
    else {
        panic!("{:?}", acolyte_ability.costs);
    };
    let any_of = filter.any_of.as_ref().expect("creature or artifact");
    assert!(any_of.iter().any(|f| f.kind == TargetKind::Creature
        && f.controller == TargetController::You
        && f.excluded_objects.contains(&TargetObjectExclusion::Source)));
    assert!(any_of.iter().any(
        |f| f.permanent_types.contains(&PermanentTypeFilter::Artifact)
            && f.excluded_objects.contains(&TargetObjectExclusion::Source)
    ));
    assert!(
        matches!(&acolyte_ability.effect[0], SpellEffectKind::LoseLife { amount, who }
            if *amount == LifeAmount::Fixed(1) && *who == PlayerRecipient::EachOpponent),
        "{:?}",
        acolyte_ability.effect[0]
    );
    assert_eq!(
        acolyte_ability.effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(1)
        }
    );

    // Bartolome del Presidio: sacrifice another creature or artifact: +1/+1 counter on itself.
    let bartolome = primary(registry, "bartolome_del_presidio");
    let [bartolome_ability] = bartolome.activated_abilities.as_slice() else {
        panic!("one ability");
    };
    let [AbilityCost::SacrificePermanent { filter }] = bartolome_ability.costs.as_slice() else {
        panic!("{:?}", bartolome_ability.costs);
    };
    assert!(filter.any_of.is_some(), "creature or artifact");
    assert!(
        matches!(&bartolome_ability.effect[0], SpellEffectKind::PutCounters {
            counter, count, subject: EffectSubject::Source }
            if *counter == CounterKind::PlusOnePlusOne && *count == Amount::Fixed(1)),
        "{:?}",
        bartolome_ability.effect[0]
    );

    // Captain Storm: artifact enters -> +1/+1 counter on a target Pirate you control.
    let storm = primary(registry, "captain_storm,_cosmium_raider");
    let [storm_trigger] = storm.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller, filter, ..
    } = &storm_trigger.trigger
    else {
        panic!("enters trigger, got {:?}", storm_trigger.trigger);
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Artifact));
    let SpellEffectKind::PutCounters {
        counter,
        count,
        subject: EffectSubject::Chosen(target),
    } = &storm_trigger.effect[0]
    else {
        panic!("put counters, got {:?}", storm_trigger.effect[0]);
    };
    assert_eq!(*counter, CounterKind::PlusOnePlusOne);
    assert_eq!(*count, Amount::Fixed(1));
    assert_eq!(target.kind, TargetKind::Creature);
    assert_eq!(target.controller, TargetController::You);
    assert_eq!(target.required_subtypes, ["Pirate"]);
    assert_eq!(
        storm_trigger.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Ashiok's Reaper: an enchantment you control leaves the battlefield for a graveyard -> draw.
    let reaper = primary(registry, "ashioks_reaper");
    let [reaper_trigger] = reaper.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    let TriggerCondition::WheneverPermanentLeavesBattlefield {
        controller,
        filter,
        destination,
        ..
    } = &reaper_trigger.trigger
    else {
        panic!("leaves trigger, got {:?}", reaper_trigger.trigger);
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(
        filter.permanent_type,
        Some(PermanentTypeFilter::Enchantment)
    );
    assert_eq!(
        *destination,
        ZoneEventDestination::OneOf(vec![EventZone::Graveyard])
    );
    assert_eq!(
        reaper_trigger.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );

    // Battlesong Berserker: whenever you attack, target creature you control gets +1/+0 and menace.
    let berserker = primary(registry, "battlesong_berserker");
    let [berserker_trigger] = berserker.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(
        matches!(
            berserker_trigger.trigger,
            TriggerCondition::WheneverControllerAttacks { .. }
        ),
        "{:?}",
        berserker_trigger.trigger
    );
    let SpellEffectKind::PumpTarget {
        power,
        toughness,
        subject: EffectSubject::Chosen(pump_target),
        ..
    } = &berserker_trigger.effect[0]
    else {
        panic!("pump, got {:?}", berserker_trigger.effect[0]);
    };
    assert_eq!((*power, *toughness), (1, 0));
    assert_eq!(pump_target.kind, TargetKind::Creature);
    assert_eq!(pump_target.controller, TargetController::You);
    let SpellEffectKind::GrantKeywords {
        subject: EffectSubject::Chosen(grant_target),
        keywords,
    } = &berserker_trigger.effect[1]
    else {
        panic!("grant, got {:?}", berserker_trigger.effect[1]);
    };
    assert_eq!(grant_target.kind, TargetKind::Creature);
    assert_eq!(keywords, &vec![Keyword::Menace]);
    assert_eq!(
        berserker_trigger.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0, 1]
    );

    // Clammy Prowler: attacks -> another target attacking creature can't be blocked this turn.
    let prowler = primary(registry, "clammy_prowler");
    let [prowler_trigger] = prowler.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(
        matches!(
            prowler_trigger.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0
            }
        ),
        "{:?}",
        prowler_trigger.trigger
    );
    let SpellEffectKind::ApplyCombatRestriction {
        scope: CombatRestrictionScope::Chosen(target),
        restriction,
    } = &prowler_trigger.effect[0]
    else {
        panic!("combat restriction, got {:?}", prowler_trigger.effect[0]);
    };
    assert_eq!(
        *restriction,
        CombatRestriction {
            cant_be_blocked: true,
            ..CombatRestriction::default()
        }
    );
    assert_eq!(target.kind, TargetKind::Creature);
    assert_eq!(target.combat_role, Some(CombatRole::Attacking));
    assert_eq!(
        target.controller,
        TargetController::Any,
        "any attacking creature, not just the controller's"
    );
    assert!(target
        .excluded_objects
        .contains(&TargetObjectExclusion::Source));
    assert_eq!(
        prowler_trigger.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Bake into a Pie: destroy target creature, then create a Food token.
    let pie = primary(registry, "bake_into_a_pie");
    let SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(target),
    } = &pie.spell_effect[0]
    else {
        panic!("destroy, got {:?}", pie.spell_effect[0]);
    };
    assert_eq!(target.kind, TargetKind::Creature);
    assert!(
        matches!(&pie.spell_effect[1], SpellEffectKind::CreateTokens { token, count, .. }
            if token == "food" && *count == Amount::Fixed(1)),
        "{:?}",
        pie.spell_effect[1]
    );
    assert_eq!(
        pie.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );
}

#[test]
fn issue_misc5_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "acolyte_of_aclazotz",
            "Acolyte of Aclazotz",
            "28636449cdd726955c3ffd68bcb625c1f2dcf7ea5c1c6e1935589cc32a867624",
        ),
        (
            "bartolome_del_presidio",
            "Bartolomé del Presidio",
            "9fff0ecc2e4048dc2b0bbe115b63f05fedf0d38a877abcbf48d587802fb4eedf",
        ),
        (
            "captain_storm,_cosmium_raider",
            "Captain Storm, Cosmium Raider",
            "8c5291e9132681e5db42d2a8dc92dfe958759cdc6fa49a2570fa66367608eb3d",
        ),
        (
            "ashioks_reaper",
            "Ashiok's Reaper",
            "cb53c97830591a9e44571e470bd5d58e85f6c5466e2a54cf63d5eecbd6beb14e",
        ),
        (
            "battlesong_berserker",
            "Battlesong Berserker",
            "68b0ceb6b3c382bac24ca1dfc5f1ae2deb5a5f34eb76c64255d689d881a69dca",
        ),
        (
            "clammy_prowler",
            "Clammy Prowler",
            "1e5e9e68394e52ff92ff4a0e2c5d165c908aa01a0968a9833cbee9b53708aa96",
        ),
        (
            "bake_into_a_pie",
            "Bake into a Pie",
            "d6f71fdade69478f444fb5130ccdbb8a2b93eeefef4d7bd3dbe715ca51aaf864",
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
