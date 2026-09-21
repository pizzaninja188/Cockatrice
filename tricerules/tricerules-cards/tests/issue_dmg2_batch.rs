//! Registry conformance for the reviewed damage-and-riders direct-RON batch.
//!
//! Lightning Helix, Winter's Intervention, Deadly Riposte, Cosmium Blast, Vibrant Outburst,
//! Fear of Lost Teeth and Bile-Vial Boggart were promoted after complete-definition review against
//! the pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-21). Governance:
//! CR 115 (targets), 120 (damage), 119.3 (life gain), 508/509 (attacking/blocking), 701.26 (tap),
//! and 122.1 (counters).

use tricerules_cards::primitives::{
    Amount, CombatRole, CounterKind, EffectSubject, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_dmg2_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "lightning_helix",
            "Lightning Helix",
            "{R}{W}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "winters_intervention",
            "Winter's Intervention",
            "{1}{B}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "deadly_riposte",
            "Deadly Riposte",
            "{1}{W}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "cosmium_blast",
            "Cosmium Blast",
            "{1}{W}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "vibrant_outburst",
            "Vibrant Outburst",
            "{U}{R}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "fear_of_lost_teeth",
            "Fear of Lost Teeth",
            "{B}",
            &["Enchantment", "Creature", "Nightmare"][..],
            Some(1),
            Some(1),
        ),
        (
            "bile-vial_boggart",
            "Bile-Vial Boggart",
            "{B}",
            &["Creature", "Goblin", "Assassin"][..],
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
    }

    // Lightning Helix: three to any target, gain three.
    let helix = primary(registry, "lightning_helix");
    assert_eq!(
        helix.spell_effect,
        [
            SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: any_target_filter(),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(3)
            },
        ]
    );
    assert_eq!(
        helix.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Winter's Intervention: two to a creature, gain two.
    let winter = primary(registry, "winters_intervention");
    assert!(
        matches!(&winter.spell_effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(2) && target.kind == TargetKind::Creature),
        "{:?}",
        winter.spell_effect[0]
    );
    assert_eq!(
        winter.spell_effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(2)
        }
    );

    // Deadly Riposte: three to a tapped creature, gain two.
    let riposte = primary(registry, "deadly_riposte");
    assert!(
        matches!(&riposte.spell_effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(3)
                && target.kind == TargetKind::Creature
                && target.tapped == Some(true)),
        "{:?}",
        riposte.spell_effect[0]
    );
    assert_eq!(
        riposte.spell_effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(2)
        }
    );

    // Cosmium Blast: four to an attacking or blocking creature.
    let cosmium = primary(registry, "cosmium_blast");
    assert!(
        matches!(&cosmium.spell_effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(4)
                && target.kind == TargetKind::Creature
                && target.combat_role == Some(CombatRole::AttackingOrBlocking)),
        "{:?}",
        cosmium.spell_effect[0]
    );

    // Vibrant Outburst: three to any target, then tap up to one target creature.
    let vibrant = primary(registry, "vibrant_outburst");
    assert!(
        matches!(&vibrant.spell_effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(3) && target.kind == TargetKind::AnyTarget),
        "{:?}",
        vibrant.spell_effect[0]
    );
    assert!(
        matches!(&vibrant.spell_effect[1], SpellEffectKind::Tap { subject: EffectSubject::Chosen(filter) }
            if filter.kind == TargetKind::Creature),
        "{:?}",
        vibrant.spell_effect[1]
    );
    let groups = &vibrant.targeting.as_ref().unwrap().groups;
    assert_eq!(
        (
            groups[0].min,
            groups[0].max,
            groups[0].effect_indices.clone()
        ),
        (1, 1, vec![0])
    );
    assert_eq!(
        (
            groups[1].min,
            groups[1].max,
            groups[1].effect_indices.clone()
        ),
        (0, 1, vec![1])
    );

    // Fear of Lost Teeth: dies trigger, one to any target and gain one.
    let fear = primary(registry, "fear_of_lost_teeth");
    let [trigger] = fear.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    assert!(
        matches!(&trigger.effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(1) && target.kind == TargetKind::AnyTarget),
        "{:?}",
        trigger.effect[0]
    );
    assert_eq!(
        trigger.effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(1)
        }
    );
    assert_eq!(
        trigger.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Bile-Vial Boggart: dies trigger, -1/-1 counter on up to one target creature.
    let boggart = primary(registry, "bile-vial_boggart");
    let [trigger] = boggart.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    assert!(
        matches!(&trigger.effect[0], SpellEffectKind::PutCounters { counter, count, subject: EffectSubject::Chosen(filter) }
            if *counter == CounterKind::MinusOneMinusOne
                && *count == Amount::Fixed(1)
                && filter.kind == TargetKind::Creature),
        "{:?}",
        trigger.effect[0]
    );
    let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("one group");
    };
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.effect_indices, [0]);
}

/// `DamageTarget`'s any-target filter.
fn any_target_filter() -> tricerules_cards::primitives::TargetFilter {
    tricerules_cards::primitives::TargetFilter {
        kind: TargetKind::AnyTarget,
        ..tricerules_cards::primitives::TargetFilter::default()
    }
}

#[test]
fn issue_dmg2_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "lightning_helix",
            "Lightning Helix",
            "47dd3b2148b209d3259999b82e16ed9ecc67ea0e39b5cc6dea61087e8b602a4c",
        ),
        (
            "winters_intervention",
            "Winter's Intervention",
            "daf69d8c096206e89fefce31b50e50b2618756fe4cbb07ed984845935d5052b6",
        ),
        (
            "deadly_riposte",
            "Deadly Riposte",
            "b20cdd21fadaa263c3ee8b90b5b922c2c8a9a73e0dea42fa94253d713deca324",
        ),
        (
            "cosmium_blast",
            "Cosmium Blast",
            "0e6602ff9de577765b0ed15a866c823899bb41b43e3a89499d3b0a6809eafb99",
        ),
        (
            "vibrant_outburst",
            "Vibrant Outburst",
            "63c708d5bc3ba8272f7af3d9a462d6767f0cbd710e2119af61d9d384bb4c0f9f",
        ),
        (
            "fear_of_lost_teeth",
            "Fear of Lost Teeth",
            "9568ce7ab41530666261b53e90ff799b2ee1f75bac5096b80620520424f62b21",
        ),
        (
            "bile-vial_boggart",
            "Bile-Vial Boggart",
            "8d28bc742753f46d08c407e766761c6341235d86ff2c270ada465a92cec4a6e5",
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
