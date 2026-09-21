//! Registry conformance for the #465 token-definition batch: the Hero, Fungus, and Dinosaur
//! predefined tokens plus the three Standard cards they unblock.
//!
//! Dwarven Castle Guard, Synapse Necromage and Scalestorm Summoner were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-21). Governance: CR 111.1 (tokens), 603.6c (dies triggers),
//! 508.1/603.2c (attack triggers), and 608.2 (a conditional instruction is evaluated as it
//! resolves).

use tricerules_cards::primitives::{
    Amount, BattlefieldAggregate, CardTypeFilter, GameCondition, PlayerRecipient,
    RelativePlayerSet, ResolutionBranchRequirement, ResolutionBranchSelection, SpellEffectKind,
    StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Color, Layout};

#[test]
fn issue_465_token_blockers_maps_definitions() {
    let registry = CardRegistry::global();

    // The three predefined tokens carry their printed name, color, types and P/T.
    for (id, name, types, colors, power, toughness) in [
        (
            "hero_c_1_1",
            "Hero",
            &["Creature", "Hero"][..],
            Some(Vec::new()),
            1u32,
            1u32,
        ),
        (
            "fungus_b_1_1_cant_block",
            "Fungus",
            &["Creature", "Fungus"][..],
            Some(vec![Color::Black]),
            1,
            1,
        ),
        (
            "dinosaur_r_3_1",
            "Dinosaur",
            &["Creature", "Dinosaur"][..],
            Some(vec![Color::Red]),
            3,
            1,
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing token {id}"));
        assert!(registry.is_token(id), "{id} is a token");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        assert_eq!(f.colors_override, colors, "{id} colors");
        assert!(
            f.triggered_abilities.is_empty() && f.activated_abilities.is_empty(),
            "{id} has only printed characteristics"
        );
    }

    // The Fungus token cannot block.
    let fungus = registry
        .get("fungus_b_1_1_cant_block")
        .expect("token")
        .primary_face();
    assert!(fungus.static_abilities.iter().any(|ability| matches!(
        &ability.definition,
        StaticAbilityDef::SelfCombatRestriction {
            restriction,
            condition: None,
        } if restriction.cant_block
    )));

    // The Hero token is colorless.
    assert_eq!(
        registry
            .get("hero_c_1_1")
            .expect("token")
            .primary_face()
            .colors_override,
        Some(Vec::new())
    );

    // Two dies triggers that create the new tokens.
    for (id, name, mana, types, power, toughness, token, count) in [
        (
            "dwarven_castle_guard",
            "Dwarven Castle Guard",
            "{1}{W}",
            &["Creature", "Dwarf", "Soldier"][..],
            2,
            1,
            "hero_c_1_1",
            1,
        ),
        (
            "synapse_necromage",
            "Synapse Necromage",
            "{2}{B}",
            &["Creature", "Fungus", "Wizard"][..],
            3,
            1,
            "fungus_b_1_1_cant_block",
            2,
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        let f = definition.primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id} has one trigger");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
        let [effect] = trigger.effect.as_slice() else {
            panic!("{id} has one effect");
        };
        assert!(
            matches!(
                effect,
                SpellEffectKind::CreateTokens { token: got, count: got_count, who: PlayerRecipient::Controller, .. }
                    if got == token && *got_count == Amount::Fixed(count)
            ),
            "{id} creates {count} {token}, got {effect:?}"
        );
    }

    // Scalestorm Summoner: an attack trigger whose Dinosaur is created only while the controller
    // has a creature with power 4 or greater, rechecked as the trigger resolves.
    let scalestorm = registry.get("scalestorm_summoner").expect("registered");
    let f = scalestorm.primary_face();
    assert_eq!(f.name, "Scalestorm Summoner");
    assert_eq!(f.mana_cost.to_string(), "{2}{R}");
    assert_eq!(f.types, ["Creature", "Human", "Warlock"]);
    assert_eq!((f.power, f.toughness), (Some(3), Some(3)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    let [effect] = trigger.effect.as_slice() else {
        panic!("one effect");
    };
    let SpellEffectKind::ChooseResolutionBranch {
        selection,
        branches,
        ..
    } = effect
    else {
        panic!("conditional branch, got {effect:?}");
    };
    assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
    let [dinosaur, fallback] = branches.as_slice() else {
        panic!("two branches");
    };
    assert!(
        matches!(
            &dinosaur.requirement,
            ResolutionBranchRequirement::GameCondition(GameCondition::BattlefieldAggregate {
                filter,
                aggregate: BattlefieldAggregate::MaximumPower,
                min: Some(4),
                ..
            }) if filter.controllers == RelativePlayerSet::Controller
                && filter.card_type == Some(CardTypeFilter::Creature)
        ),
        "power-4 condition, got {:?}",
        dinosaur.requirement
    );
    assert_eq!(
        dinosaur.effects,
        [SpellEffectKind::CreateTokens {
            token: "dinosaur_r_3_1".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert_eq!(fallback.requirement, ResolutionBranchRequirement::Always);
    assert!(fallback.effects.is_empty());
}

#[test]
fn issue_465_token_blockers_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "dwarven_castle_guard",
            "Dwarven Castle Guard",
            "723e672d91d358f4c42f8002ec93fd94decbe8f34abc01bdf437c7957825ccd4",
        ),
        (
            "synapse_necromage",
            "Synapse Necromage",
            "0c3d2340b62b0555d542df9b182a2bf71cdae716fbb79a428741bc41cfd794a7",
        ),
        (
            "scalestorm_summoner",
            "Scalestorm Summoner",
            "1cdca9b779c8629ee9f110dbdc2298a50438b70972786cda6f510a19f3b45aa7",
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
