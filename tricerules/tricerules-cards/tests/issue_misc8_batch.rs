//! Registry conformance for the reviewed direct-RON targeted-spell batch.
//!
//! Hour of Defeat, Unsubtle Mockery, Reckless Ransacking, Savor, Masterful Flourish, Kin-Tree
//! Severance, Joust Through and Spider Food were promoted after complete-definition review
//! against the pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-21).
//! Governance: CR 115 (targets), CR 118/119 (life), CR 120 (damage), CR 202.3 (mana value),
//! CR 301.5 (Treasure/Food), CR 608.2b (target revalidation), CR 611.2c (until end of turn),
//! CR 701.25 (surveil), and CR 702.12b (indestructible).

use tricerules_cards::primitives::{
    Amount, CombatRole, EffectSubject, LibraryPartitionKind, SpellEffectKind, TargetController,
    TargetKind,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn surviel_partition() -> SpellEffectKind {
    SpellEffectKind::LibraryPartition {
        count: 1,
        top_min: 0,
        top_max: None,
        kind: LibraryPartitionKind::Surveil,
    }
}

#[test]
fn issue_misc8_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types) in [
        (
            "hour_of_defeat",
            "Hour of Defeat",
            "{3}{B}",
            &["Instant"][..],
        ),
        (
            "unsubtle_mockery",
            "Unsubtle Mockery",
            "{2}{R}",
            &["Instant"][..],
        ),
        (
            "reckless_ransacking",
            "Reckless Ransacking",
            "{1}{R}",
            &["Instant"][..],
        ),
        ("savor", "Savor", "{1}{B}", &["Instant"][..]),
        (
            "masterful_flourish",
            "Masterful Flourish",
            "{B}",
            &["Instant"][..],
        ),
        (
            "kin-tree_severance",
            "Kin-Tree Severance",
            "{2/W}{2/B}{2/G}",
            &["Instant"][..],
        ),
        ("joust_through", "Joust Through", "{W}", &["Instant"][..]),
        ("spider_food", "Spider Food", "{2}{G}", &["Sorcery"][..]),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
    }

    // Hour of Defeat: destroy target creature, then surveil 1.
    let hour = primary(registry, "hour_of_defeat");
    assert!(
        matches!(&hour.spell_effect[0], SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter) } if filter.kind == TargetKind::Creature),
        "{:?}",
        hour.spell_effect[0]
    );
    assert_eq!(hour.spell_effect[1], surviel_partition());
    assert_eq!(
        hour.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Unsubtle Mockery: 4 damage to target creature, then surveil 1.
    let mockery = primary(registry, "unsubtle_mockery");
    assert!(
        matches!(&mockery.spell_effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(4) && target.kind == TargetKind::Creature),
        "{:?}",
        mockery.spell_effect[0]
    );
    assert_eq!(mockery.spell_effect[1], surviel_partition());

    // Reckless Ransacking: target creature gets +3/+2, then create a Treasure.
    let ransacking = primary(registry, "reckless_ransacking");
    assert!(
        matches!(
            &ransacking.spell_effect[0],
            SpellEffectKind::PumpTarget {
                power: 3,
                toughness: 2,
                ..
            }
        ),
        "{:?}",
        ransacking.spell_effect[0]
    );
    assert!(
        matches!(&ransacking.spell_effect[1], SpellEffectKind::CreateTokens { token, count, .. }
            if token == "treasure" && *count == Amount::Fixed(1)),
        "{:?}",
        ransacking.spell_effect[1]
    );

    // Savor: target creature gets -2/-2, then create a Food.
    let savor = primary(registry, "savor");
    assert!(
        matches!(
            &savor.spell_effect[0],
            SpellEffectKind::PumpTarget {
                power: -2,
                toughness: -2,
                ..
            }
        ),
        "{:?}",
        savor.spell_effect[0]
    );
    assert!(
        matches!(&savor.spell_effect[1], SpellEffectKind::CreateTokens { token, count, .. }
            if token == "food" && *count == Amount::Fixed(1)),
        "{:?}",
        savor.spell_effect[1]
    );

    // Masterful Flourish: target creature you control gets +1/+0 and gains indestructible.
    let flourish = primary(registry, "masterful_flourish");
    let SpellEffectKind::PumpTarget {
        power,
        toughness,
        subject: EffectSubject::Chosen(pump_target),
        ..
    } = &flourish.spell_effect[0]
    else {
        panic!("pump, got {:?}", flourish.spell_effect[0]);
    };
    assert_eq!((*power, *toughness), (1, 0));
    assert_eq!(pump_target.kind, TargetKind::Creature);
    assert_eq!(pump_target.controller, TargetController::You);
    assert!(
        matches!(&flourish.spell_effect[1], SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(target), keywords }
            if target.controller == TargetController::You && keywords == &vec![Keyword::Indestructible]),
        "{:?}",
        flourish.spell_effect[1]
    );
    assert_eq!(
        flourish.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0, 1]
    );

    // Kin-Tree Severance: exile target permanent with mana value 3 or greater.
    let severance = primary(registry, "kin-tree_severance");
    assert!(
        matches!(&severance.spell_effect[0], SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter) }
            if filter.kind == TargetKind::AnyPermanent
                && filter.min_mana_value == Some(3)),
        "{:?}",
        severance.spell_effect[0]
    );

    // Joust Through: 3 damage to target attacking or blocking creature, then gain 1 life.
    let joust = primary(registry, "joust_through");
    assert!(
        matches!(&joust.spell_effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(3) && target.combat_role == Some(CombatRole::AttackingOrBlocking)),
        "{:?}",
        joust.spell_effect[0]
    );
    assert_eq!(
        joust.spell_effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(1)
        }
    );

    // Spider Food: destroy up to one target artifact, enchantment, or creature with flying; create Food.
    let spider = primary(registry, "spider_food");
    let SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(target),
    } = &spider.spell_effect[0]
    else {
        panic!("destroy, got {:?}", spider.spell_effect[0]);
    };
    let any_of = target.any_of.as_ref().expect("three branches");
    assert!(any_of.iter().any(
        |f| f.permanent_types == [tricerules_cards::primitives::PermanentTypeFilter::Artifact]
    ));
    assert!(any_of
        .iter()
        .any(|f| f.permanent_types
            == [tricerules_cards::primitives::PermanentTypeFilter::Enchantment]));
    assert!(any_of
        .iter()
        .any(|f| f.required_keywords == [Keyword::Flying]));
    assert!(
        matches!(&spider.spell_effect[1], SpellEffectKind::CreateTokens { token, count, .. }
            if token == "food" && *count == Amount::Fixed(1)),
        "{:?}",
        spider.spell_effect[1]
    );
    let group = &spider.targeting.as_ref().unwrap().groups[0];
    assert_eq!((group.min, group.max), (0, 1), "up to one target");
}

#[test]
fn issue_misc8_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "hour_of_defeat",
            "Hour of Defeat",
            "34fe42b59872632bd105daf4bc0dcb3280a5743591a52c361daf3133bad38578",
        ),
        (
            "unsubtle_mockery",
            "Unsubtle Mockery",
            "12531132533eb398a957f7b4200962216db7024f0136de05acfc08cd80ce8b23",
        ),
        (
            "reckless_ransacking",
            "Reckless Ransacking",
            "f88110a4d7682b5a6c899fb696fcf5fe3fffa8aaf0aac4112aa75515794ee2a5",
        ),
        (
            "savor",
            "Savor",
            "28e489560d2e7f3c115d509b287e1191f54bcf37fcd41cfce29adc924840f659",
        ),
        (
            "masterful_flourish",
            "Masterful Flourish",
            "5af84dba12f0111dde5c420eb80266098626932f1739920dbd1164553302d26b",
        ),
        (
            "kin-tree_severance",
            "Kin-Tree Severance",
            "329d3181d82a6561c3e7914e2792b94cbc8c5b399e50adbe07fbebb10f6349de",
        ),
        (
            "joust_through",
            "Joust Through",
            "d92748e1c1fb0f0b0302a202a822ad33666b2dc70b2d6f83a4114b91168152a8",
        ),
        (
            "spider_food",
            "Spider Food",
            "d06e91b6b912d93225c711255d9591dbe195f10c3df87aceecf291ced7c432d8",
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
