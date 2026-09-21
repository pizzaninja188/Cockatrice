//! Registry conformance for the reviewed direct-RON activated-ability and token-entry batch.
//!
//! Umbral Collar Zealot, Bold Biochemist, Hardened Tactician, Wildheart Invoker, Sting-Slinger,
//! Tunnel Surveyor, Fire Nation Raider and Sami's Curiosity were promoted after complete-definition
//! review against the pinned Scryfall snapshot (exact records and `rulings_uri` fetched
//! 2026-09-21). Governance: CR 115 (targets), CR 118.12 (additional costs), CR 119 (life),
//! CR 120.3 (damage), CR 301.5 (Clue/Lander), CR 602 (activated abilities), CR 603.4
//! (intervening-if), CR 603.6 (entry triggers), CR 611.2c (until end of turn), CR 701.25
//! (surveil), CR 701.68 (blight), and CR 702.177 (power-up/exhaust activation limit).

use tricerules_cards::primitives::{
    AbilityCost, ActivatedCostModifier, ActivationLimit, Amount, CounterKind, EffectSubject,
    GameCondition, LibraryPartitionKind, PlayerRecipient, RelativePlayerSet, SpellEffectKind,
    TargetController, TargetKind, TriggerCondition,
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
fn issue_misc10_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "umbral_collar_zealot",
            "Umbral Collar Zealot",
            "{1}{B}",
            &["Creature", "Human", "Cleric"][..],
            Some(3),
            Some(2),
        ),
        (
            "bold_biochemist",
            "Bold Biochemist",
            "{1}{U}",
            &["Creature", "Human", "Scientist"][..],
            Some(1),
            Some(3),
        ),
        (
            "hardened_tactician",
            "Hardened Tactician",
            "{1}{W}{B}",
            &["Creature", "Human", "Warrior"][..],
            Some(2),
            Some(4),
        ),
        (
            "wildheart_invoker",
            "Wildheart Invoker",
            "{2}{G}{G}",
            &["Creature", "Elf", "Shaman"][..],
            Some(4),
            Some(3),
        ),
        (
            "sting-slinger",
            "Sting-Slinger",
            "{2}{R}",
            &["Creature", "Goblin", "Warrior"][..],
            Some(3),
            Some(3),
        ),
        (
            "tunnel_surveyor",
            "Tunnel Surveyor",
            "{2}{U}",
            &["Creature", "Human", "Detective"][..],
            Some(2),
            Some(2),
        ),
        (
            "fire_nation_raider",
            "Fire Nation Raider",
            "{3}{R}",
            &["Creature", "Human", "Soldier"][..],
            Some(4),
            Some(2),
        ),
        (
            "samis_curiosity",
            "Sami's Curiosity",
            "{G}",
            &["Sorcery"][..],
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

    // Umbral Collar Zealot: sacrifice another creature or artifact -> surveil 1.
    let zealot = activated(registry, "umbral_collar_zealot");
    let [AbilityCost::SacrificePermanent { filter }] = zealot.costs.as_slice() else {
        panic!("{:?}", zealot.costs);
    };
    assert!(filter.any_of.is_some(), "creature or artifact");
    assert_eq!(
        zealot.effect,
        [SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );

    // Bold Biochemist: power-up {5}{U}, once per object, reduced when it entered this turn.
    let biochemist = activated(registry, "bold_biochemist");
    assert!(
        matches!(biochemist.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{5}{U}"),
        "{:?}",
        biochemist.costs
    );
    assert_eq!(
        biochemist.activation_limit,
        Some(ActivationLimit::PerObject { max_activations: 1 })
    );
    let [ActivatedCostModifier::ConditionalSourceManaCostReduction { condition }] =
        biochemist.cost_modifiers.as_slice()
    else {
        panic!("{:?}", biochemist.cost_modifiers);
    };
    let GameCondition::PermanentsEnteredThisTurn {
        controllers,
        filter,
        min,
        ..
    } = condition
    else {
        panic!("entered-this-turn condition, got {condition:?}");
    };
    assert_eq!(*controllers, RelativePlayerSet::All);
    assert!(filter.source_only, "only the source's own entry counts");
    assert_eq!(*min, Some(1));
    assert_eq!(
        biochemist.effect,
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            }
        ]
    );

    // Hardened Tactician: {1}, sacrifice a token -> draw a card.
    let tactician = activated(registry, "hardened_tactician");
    let [mana, sacrifice] = tactician.costs.as_slice() else {
        panic!("{:?}", tactician.costs);
    };
    assert!(matches!(mana, AbilityCost::Mana(c) if c.to_string() == "{1}"));
    let AbilityCost::SacrificePermanent { filter } = sacrifice else {
        panic!("sacrifice cost, got {sacrifice:?}");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.token, Some(true));
    assert_eq!(
        tactician.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );

    // Wildheart Invoker: {8}: target creature gets +5/+5 and gains trample.
    let invoker = activated(registry, "wildheart_invoker");
    assert!(
        matches!(invoker.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{8}"),
        "{:?}",
        invoker.costs
    );
    assert!(
        matches!(
            &invoker.effect[0],
            SpellEffectKind::PumpTarget {
                power: 5,
                toughness: 5,
                ..
            }
        ),
        "{:?}",
        invoker.effect[0]
    );
    assert!(
        matches!(&invoker.effect[1], SpellEffectKind::GrantKeywords { keywords, .. }
            if keywords == &vec![Keyword::Trample]),
        "{:?}",
        invoker.effect[1]
    );
    assert_eq!(
        invoker.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0, 1]
    );

    // Sting-Slinger: {1}{R}, {T}, Blight 1: 2 damage to each opponent.
    let slinger = activated(registry, "sting-slinger");
    let [mana, tap, blight] = slinger.costs.as_slice() else {
        panic!("{:?}", slinger.costs);
    };
    assert!(matches!(mana, AbilityCost::Mana(c) if c.to_string() == "{1}{R}"));
    assert_eq!(*tap, AbilityCost::Tap);
    assert_eq!(*blight, AbilityCost::Blight { count: 1 });
    assert_eq!(
        slinger.effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(2),
            who: PlayerRecipient::EachOpponent,
        }]
    );

    // Tunnel Surveyor: enters -> create a Glimmer.
    let surveyor = primary(registry, "tunnel_surveyor");
    let [surveyor_trigger] = surveyor.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        surveyor_trigger.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    ));
    assert!(
        matches!(&surveyor_trigger.effect[0], SpellEffectKind::CreateTokens { token, count, .. }
            if token == "glimmer_w_1_1" && *count == Amount::Fixed(1)),
        "{:?}",
        surveyor_trigger.effect[0]
    );

    // Fire Nation Raider: enters, if you attacked this turn -> create a Clue.
    let raider = primary(registry, "fire_nation_raider");
    let [raider_trigger] = raider.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        raider_trigger.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    ));
    assert_eq!(
        raider_trigger.intervening_if,
        Some(GameCondition::AttackedThisTurn {
            players: RelativePlayerSet::Controller
        })
    );
    assert!(
        matches!(&raider_trigger.effect[0], SpellEffectKind::CreateTokens { token, count, .. }
            if token == "clue" && *count == Amount::Fixed(1)),
        "{:?}",
        raider_trigger.effect[0]
    );

    // Sami's Curiosity: gain 2 life, create a Lander.
    let sami = primary(registry, "samis_curiosity");
    assert_eq!(
        sami.spell_effect,
        [
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2)
            },
            SpellEffectKind::CreateTokens {
                token: "lander".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }
        ]
    );
}

#[test]
fn issue_misc10_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "umbral_collar_zealot",
            "Umbral Collar Zealot",
            "367bd5406fed67f00ea81b245ea894d707e820c741869768be4dcd6249a63ffb",
        ),
        (
            "bold_biochemist",
            "Bold Biochemist",
            "54c20fe59644dc98d433077e7d9fd28df9f5a9344b633714d51844b7a8ad7e7f",
        ),
        (
            "hardened_tactician",
            "Hardened Tactician",
            "da7ee2b1fee0292c8c551b3e7a86511ddab22000d4af1dd9e7bdd74c275cb16a",
        ),
        (
            "wildheart_invoker",
            "Wildheart Invoker",
            "35b68d304b98604088745245bb02f20e1ae262e2c243939450aea189dff568c2",
        ),
        (
            "sting-slinger",
            "Sting-Slinger",
            "7735d7fb944d842244a61567cead2b0c614e12feef82d31c02486a4872b09e99",
        ),
        (
            "tunnel_surveyor",
            "Tunnel Surveyor",
            "5472cc023b939522c42f7c88cf01a375ac48dffe4e47d9de3b4596b068d500cb",
        ),
        (
            "fire_nation_raider",
            "Fire Nation Raider",
            "e8d90e8525bc2947add5018d6a0bc0cd5d1ce8d06a53d56d6efe99aa17a7b018",
        ),
        (
            "samis_curiosity",
            "Sami's Curiosity",
            "1dca20a45463ee83e652f459f62a553222db01d835127731f5b5b94e33813e90",
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
