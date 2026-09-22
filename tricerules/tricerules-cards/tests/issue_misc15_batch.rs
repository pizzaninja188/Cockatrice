//! Registry conformance for the reviewed direct-RON pump/combat batch.
//!
//! Dauntless Veteran, Remnant Elemental, Skystinger, Friendly Ghost, Nebula Dragon, Cavern Stomper,
//! Hermitic Nautilus and Lion Heart were promoted after complete-definition review against the
//! pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-22). Governance:
//! CR 115.1a/120.3 (targeted damage), CR 301.5/702.6 (Equipment and equip), CR 508.3 (attack
//! trigger), CR 509.1b/509.3b (block trigger and evasion), CR 603.6a (entry trigger), CR 611.2c/
//! 613.4c (until-end-of-turn P/T), CR 701.22 (scry), CR 702.9 (flying), CR 702.17 (reach), and
//! CR 702.20 (vigilance).

use tricerules_cards::primitives::{
    AbilityCost, Amount, PowerComparison, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
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
    index: usize,
) -> &'a tricerules_cards::primitives::ActivatedAbilityDef {
    &primary(registry, id).activated_abilities[index]
}

#[test]
fn issue_misc15_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "dauntless_veteran",
            "Dauntless Veteran",
            "{1}{W}{W}",
            &["Creature", "Human", "Soldier"][..],
            Some(2),
            Some(2),
        ),
        (
            "remnant_elemental",
            "Remnant Elemental",
            "{1}{R}",
            &["Creature", "Elemental"][..],
            Some(0),
            Some(4),
        ),
        (
            "skystinger",
            "Skystinger",
            "{2}{G}",
            &["Creature", "Insect", "Warrior"][..],
            Some(3),
            Some(3),
        ),
        (
            "friendly_ghost",
            "Friendly Ghost",
            "{3}{W}",
            &["Creature", "Spirit"][..],
            Some(2),
            Some(4),
        ),
        (
            "nebula_dragon",
            "Nebula Dragon",
            "{6}{R}",
            &["Creature", "Dragon"][..],
            Some(4),
            Some(4),
        ),
        (
            "cavern_stomper",
            "Cavern Stomper",
            "{4}{G}{G}",
            &["Creature", "Dinosaur"][..],
            Some(7),
            Some(7),
        ),
        (
            "hermitic_nautilus",
            "Hermitic Nautilus",
            "{1}{U}",
            &["Artifact", "Creature", "Nautilus"][..],
            Some(1),
            Some(4),
        ),
        (
            "lion_heart",
            "Lion Heart",
            "{4}",
            &["Artifact", "Equipment"][..],
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

    // Dauntless Veteran: attacking pumps each creature you control +1/+1.
    let veteran = primary(registry, "dauntless_veteran");
    assert!(matches!(
        veteran.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    ));
    assert!(
        matches!(
            &veteran.triggered_abilities[0].effect[0],
            SpellEffectKind::PumpAll {
                power: 1,
                toughness: 1,
                ..
            }
        ),
        "{:?}",
        veteran.triggered_abilities[0].effect
    );

    // Remnant Elemental: reach plus a landfall self-pump.
    let remnant = primary(registry, "remnant_elemental");
    assert!(remnant.keywords.contains(&Keyword::Reach));
    assert!(matches!(
        &remnant.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield { .. }
    ));
    assert!(
        matches!(
            &remnant.triggered_abilities[0].effect[0],
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                ..
            }
        ),
        "{:?}",
        remnant.triggered_abilities[0].effect
    );

    // Skystinger: reach plus a block-a-flyer self-pump.
    let skystinger = primary(registry, "skystinger");
    assert!(skystinger.keywords.contains(&Keyword::Reach));
    let TriggerCondition::WheneverSelfBlocksCreature { attacker } =
        &skystinger.triggered_abilities[0].trigger
    else {
        panic!("{:?}", skystinger.triggered_abilities[0].trigger);
    };
    assert!(attacker.required_keywords.contains(&Keyword::Flying));
    assert!(
        matches!(
            &skystinger.triggered_abilities[0].effect[0],
            SpellEffectKind::PumpTarget {
                power: 5,
                toughness: 0,
                ..
            }
        ),
        "{:?}",
        skystinger.triggered_abilities[0].effect
    );

    // Friendly Ghost: flying plus a targeted entry pump.
    let ghost = primary(registry, "friendly_ghost");
    assert!(ghost.keywords.contains(&Keyword::Flying));
    assert!(
        matches!(
            &ghost.triggered_abilities[0].effect[0],
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 4,
                ..
            }
        ),
        "{:?}",
        ghost.triggered_abilities[0].effect
    );

    // Nebula Dragon: flying plus entry damage to any target.
    let dragon = primary(registry, "nebula_dragon");
    assert!(dragon.keywords.contains(&Keyword::Flying));
    assert!(
        matches!(&dragon.triggered_abilities[0].effect[0], SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(3),
            target,
        } if target.kind == TargetKind::AnyTarget),
        "{:?}",
        dragon.triggered_abilities[0].effect
    );
    let dragon_group = &dragon.triggered_abilities[0]
        .targeting
        .as_ref()
        .expect("targeting")
        .groups[0];
    assert_eq!((dragon_group.min, dragon_group.max), (1, 1));

    // Cavern Stomper: entry scry 2 plus {3}{G} unblockable-by-power-2-or-less.
    let stomper = primary(registry, "cavern_stomper");
    assert!(
        matches!(
            &stomper.triggered_abilities[0].effect[0],
            SpellEffectKind::Scry {
                count: Amount::Fixed(2)
            }
        ),
        "{:?}",
        stomper.triggered_abilities[0].effect
    );
    let stomper_ability = activated(registry, "cavern_stomper", 0);
    assert!(
        matches!(stomper_ability.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{3}{G}"),
        "{:?}",
        stomper_ability.costs
    );
    let SpellEffectKind::ApplyCombatRestriction { restriction, .. } = &stomper_ability.effect[0]
    else {
        panic!("{:?}", stomper_ability.effect);
    };
    assert!(
        matches!(restriction.cant_be_blocked_by.as_slice(), [filter]
            if filter.kind == TargetKind::Creature
                && filter.power == Some(PowerComparison::AtMost(2))),
        "{:?}",
        restriction.cant_be_blocked_by
    );

    // Hermitic Nautilus: vigilance plus a +3/-3 self-pump.
    let nautilus = primary(registry, "hermitic_nautilus");
    assert!(nautilus.keywords.contains(&Keyword::Vigilance));
    let nautilus_ability = activated(registry, "hermitic_nautilus", 0);
    assert!(
        matches!(nautilus_ability.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{1}{U}"),
        "{:?}",
        nautilus_ability.costs
    );
    assert!(
        matches!(
            &nautilus_ability.effect[0],
            SpellEffectKind::PumpTarget {
                power: 3,
                toughness: -3,
                ..
            }
        ),
        "{:?}",
        nautilus_ability.effect
    );

    // Lion Heart: entry damage, attached +2/+1, equip {2}.
    let lion = primary(registry, "lion_heart");
    assert!(
        matches!(&lion.triggered_abilities[0].effect[0], SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(2),
            target,
        } if target.kind == TargetKind::AnyTarget),
        "{:?}",
        lion.triggered_abilities[0].effect
    );
    assert!(
        matches!(
            &lion.static_abilities[0].definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 2,
                delta_toughness: 1,
                ..
            }
        ),
        "{:?}",
        lion.static_abilities[0].definition
    );
    let lion_equip = activated(registry, "lion_heart", 0);
    assert!(
        matches!(lion_equip.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{2}"),
        "{:?}",
        lion_equip.costs
    );
    assert!(matches!(
        lion_equip.effect.as_slice(),
        [SpellEffectKind::Equip { .. }]
    ));
}

#[test]
fn issue_misc15_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "cavern_stomper",
            "Cavern Stomper",
            "be3a121c96bc4baf80ef8d7937a64fcb43d67ef25711ac1fc0f47f002a45abca",
        ),
        (
            "dauntless_veteran",
            "Dauntless Veteran",
            "0bc665f95b447f913e902f18212986527e22aeca154b376fa29b7c7b1c5c180e",
        ),
        (
            "friendly_ghost",
            "Friendly Ghost",
            "7f3f7f3db035999ec7267caee9d345736660e864aa0f060142e21d5e4514ebcf",
        ),
        (
            "hermitic_nautilus",
            "Hermitic Nautilus",
            "4491da5313fa77a52533433533a9d7e75de2704852507542010751c2b5254ece",
        ),
        (
            "lion_heart",
            "Lion Heart",
            "8d0b35a3838709abb17c8d43469d709d2511c9c097875d11685fa4e28a73637b",
        ),
        (
            "nebula_dragon",
            "Nebula Dragon",
            "6919cbfffcf1c274741ec995468bd110acf52fa7f33e5e910ddf6a7f0d4459df",
        ),
        (
            "remnant_elemental",
            "Remnant Elemental",
            "c8ad4bf9163adb660fd6da4b34f1aea709c6b41b049cb9f6b57fa678bd012bd2",
        ),
        (
            "skystinger",
            "Skystinger",
            "799f10ecfa51062d7ad309ae1d6afef63e0ed33d422f8ffe46d93a27bb175124",
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
