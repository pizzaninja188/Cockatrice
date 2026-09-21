//! Registry conformance for the reviewed direct-RON activated-ability batch.
//!
//! Patriot, Shield Wielder, Brave-Kin Duo, Raccoon Rallier, Vampiric Rites, Deserter's Disciple,
//! Stark Industries Executive, Treasure Dredger and Flame-Chain Mauler were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-21). Governance: CR 115 (targets), CR 118.12/119 (life
//! payments), CR 301.5 (Treasure), CR 602 (activated abilities), CR 608.2b (target revalidation),
//! and CR 611.2c (until end of turn).

use tricerules_cards::primitives::{
    AbilityCost, ActivationTiming, Amount, CombatRestriction, CombatRestrictionScope,
    EffectSubject, PowerComparison, SpellEffectKind, TargetController, TargetKind,
    TargetObjectExclusion,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn activated<'a>(
    registry: &'a CardRegistry,
    id: &str,
) -> &'a tricerules_cards::primitives::ActivatedAbilityDef {
    let [ability] = primary(registry, id).activated_abilities.as_slice() else {
        panic!("{id} one activated ability");
    };
    ability
}

#[test]
fn issue_misc6_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "patriot,_shield_wielder",
            "Patriot, Shield Wielder",
            "{1}{W}",
            &["Creature", "Human", "Hero"][..],
            Some(2),
            Some(2),
        ),
        (
            "brave-kin_duo",
            "Brave-Kin Duo",
            "{W}",
            &["Creature", "Rabbit", "Mouse"][..],
            Some(1),
            Some(1),
        ),
        (
            "raccoon_rallier",
            "Raccoon Rallier",
            "{1}{R}",
            &["Creature", "Raccoon", "Bard"][..],
            Some(2),
            Some(2),
        ),
        (
            "vampiric_rites",
            "Vampiric Rites",
            "{B}",
            &["Enchantment"][..],
            None,
            None,
        ),
        (
            "deserters_disciple",
            "Deserter's Disciple",
            "{1}{R}",
            &["Creature", "Human", "Rebel", "Ally"][..],
            Some(2),
            Some(2),
        ),
        (
            "stark_industries_executive",
            "Stark Industries Executive",
            "{R}",
            &["Creature", "Human", "Advisor"][..],
            Some(1),
            Some(2),
        ),
        (
            "treasure_dredger",
            "Treasure Dredger",
            "{1}{B}",
            &["Creature", "Human", "Rogue"][..],
            Some(2),
            Some(2),
        ),
        (
            "flame-chain_mauler",
            "Flame-Chain Mauler",
            "{1}{R}",
            &["Creature", "Elemental", "Warrior"][..],
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
        primary(registry, "patriot,_shield_wielder").supertypes,
        ["Legendary"],
        "Patriot is legendary"
    );

    // Patriot, Shield Wielder: {2}, {T}: another target creature you control gets +2/+0 and hexproof.
    let patriot = activated(registry, "patriot,_shield_wielder");
    assert!(
        matches!(patriot.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::Tap] if c.to_string() == "{2}"),
        "{:?}",
        patriot.costs
    );
    let SpellEffectKind::PumpTarget {
        power,
        toughness,
        subject: EffectSubject::Chosen(pump_target),
        ..
    } = &patriot.effect[0]
    else {
        panic!("pump, got {:?}", patriot.effect[0]);
    };
    assert_eq!((*power, *toughness), (2, 0));
    assert_eq!(pump_target.kind, TargetKind::Creature);
    assert_eq!(pump_target.controller, TargetController::You);
    assert!(pump_target
        .excluded_objects
        .contains(&TargetObjectExclusion::Source));
    let SpellEffectKind::GrantKeywords {
        subject: EffectSubject::Chosen(grant_target),
        keywords,
    } = &patriot.effect[1]
    else {
        panic!("grant, got {:?}", patriot.effect[1]);
    };
    assert_eq!(grant_target.controller, TargetController::You);
    assert_eq!(keywords, &vec![Keyword::Hexproof]);
    assert_eq!(
        patriot.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0, 1]
    );

    // Brave-Kin Duo: {1}, {T}: target creature gets +1/+1; sorcery speed.
    let brave_kin = activated(registry, "brave-kin_duo");
    assert_eq!(brave_kin.timing, ActivationTiming::SorcerySpeed);
    assert!(
        matches!(
            brave_kin.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                ..
            }]
        ),
        "{:?}",
        brave_kin.effect
    );

    // Raccoon Rallier: {T}: target creature you control gains haste; sorcery speed.
    let raccoon = activated(registry, "raccoon_rallier");
    assert_eq!(raccoon.costs, [AbilityCost::Tap]);
    assert_eq!(raccoon.timing, ActivationTiming::SorcerySpeed);
    let SpellEffectKind::GrantKeywords {
        subject: EffectSubject::Chosen(target),
        keywords,
    } = &raccoon.effect[0]
    else {
        panic!("grant, got {:?}", raccoon.effect[0]);
    };
    assert_eq!(target.controller, TargetController::You);
    assert_eq!(keywords, &vec![Keyword::Haste]);

    // Vampiric Rites: {1}{B}, sacrifice a creature: you gain 1 life and draw a card.
    let rites = activated(registry, "vampiric_rites");
    let [mana, sacrifice] = rites.costs.as_slice() else {
        panic!("two costs, got {:?}", rites.costs);
    };
    assert!(matches!(mana, AbilityCost::Mana(c) if c.to_string() == "{1}{B}"));
    let AbilityCost::SacrificePermanent { filter } = sacrifice else {
        panic!("sacrifice cost, got {sacrifice:?}");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(
        rites.effect,
        [
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1)
            },
            SpellEffectKind::Draw {
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }
        ]
    );

    // Deserter's Disciple: {T}: another target creature you control with power <= 2 can't be blocked.
    let disciple = activated(registry, "deserters_disciple");
    assert_eq!(disciple.costs, [AbilityCost::Tap]);
    let SpellEffectKind::ApplyCombatRestriction {
        scope: CombatRestrictionScope::Chosen(target),
        restriction,
    } = &disciple.effect[0]
    else {
        panic!("combat restriction, got {:?}", disciple.effect[0]);
    };
    assert_eq!(
        *restriction,
        CombatRestriction {
            cant_be_blocked: true,
            ..CombatRestriction::default()
        }
    );
    assert_eq!(target.controller, TargetController::You);
    assert_eq!(target.power, Some(PowerComparison::AtMost(2)));
    assert!(target
        .excluded_objects
        .contains(&TargetObjectExclusion::Source));

    // Stark Industries Executive: {2}, {T}: create a Treasure token.
    let executive = activated(registry, "stark_industries_executive");
    assert!(
        matches!(executive.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::Tap] if c.to_string() == "{2}"),
        "{:?}",
        executive.costs
    );
    assert!(
        matches!(executive.effect.as_slice(), [SpellEffectKind::CreateTokens { token, count, .. }]
            if token == "treasure" && *count == Amount::Fixed(1)),
        "{:?}",
        executive.effect
    );

    // Treasure Dredger: {1}, {T}, pay 1 life: create a Treasure token.
    let dredger = activated(registry, "treasure_dredger");
    let [mana, tap, pay_life] = dredger.costs.as_slice() else {
        panic!("three costs, got {:?}", dredger.costs);
    };
    assert!(matches!(mana, AbilityCost::Mana(c) if c.to_string() == "{1}"));
    assert_eq!(*tap, AbilityCost::Tap);
    assert_eq!(*pay_life, AbilityCost::PayLife { amount: 1 });
    assert!(
        matches!(dredger.effect.as_slice(), [SpellEffectKind::CreateTokens { token, count, .. }]
            if token == "treasure" && *count == Amount::Fixed(1)),
        "{:?}",
        dredger.effect
    );

    // Flame-Chain Mauler: {1}{R}: this creature gets +1/+0 and gains menace.
    let mauler = activated(registry, "flame-chain_mauler");
    assert!(
        matches!(mauler.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{1}{R}"),
        "{:?}",
        mauler.costs
    );
    assert!(
        matches!(
            &mauler.effect[0],
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                subject: EffectSubject::Source,
                ..
            }
        ),
        "{:?}",
        mauler.effect[0]
    );
    assert!(
        matches!(&mauler.effect[1], SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Source, keywords } if keywords == &vec![Keyword::Menace]),
        "{:?}",
        mauler.effect[1]
    );
}

#[test]
fn issue_misc6_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "patriot,_shield_wielder",
            "Patriot, Shield Wielder",
            "0884381cd170a34e49b32967da0cb4fdd7d4207583d0c8d5bb8612ac621aa8e0",
        ),
        (
            "brave-kin_duo",
            "Brave-Kin Duo",
            "3210ae6f04e2b11b77bb21fa0f63431f409081ae0b054aab4c2fd71b109c0d54",
        ),
        (
            "raccoon_rallier",
            "Raccoon Rallier",
            "cd5e2fc2af3a908b6f40dbfffdc91d5c60abd55e371d8b5a7fafbda8aa4b67cb",
        ),
        (
            "vampiric_rites",
            "Vampiric Rites",
            "db25e8662d79bbe54aa4ef19f8f47ea29ed9d2232cc7c8fc352448eafc58b7b8",
        ),
        (
            "deserters_disciple",
            "Deserter's Disciple",
            "84a970b6022e407b96d7b39c3dba8e48b3df2f55861939923e2710de915d4395",
        ),
        (
            "stark_industries_executive",
            "Stark Industries Executive",
            "08689f40ec60edd64a6e19eb38b2841f3dba8d8975d10a7ceecb7bd4ec7fd1b5",
        ),
        (
            "treasure_dredger",
            "Treasure Dredger",
            "5275758e9980eeae27be9a2db2144a95d3a819d68837b9426250c6815c5a3511",
        ),
        (
            "flame-chain_mauler",
            "Flame-Chain Mauler",
            "82c25e7d8fe0a46ce76bbe95173dfd6ccd695dc01bcc7664e793073706ff06e0",
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
