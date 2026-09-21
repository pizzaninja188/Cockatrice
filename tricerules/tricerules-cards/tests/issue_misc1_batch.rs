//! Registry conformance for the reviewed activated-ability and combat-trick direct-RON batch.
//!
//! Riverguard's Reflexes, Exorcise, Rock Soldiers, Aetherize and Intrepid Tenderfoot were promoted
//! after complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-21). Governance: CR 115 (targets), 508 (attacking), 602/605
//! (activated abilities and timing), 611.2c/613 layer 7c (until end of turn), 122.1 (counters),
//! and 701.13 (exile).

use tricerules_cards::primitives::{
    AbilityCost, ActivationTiming, CombatRole, CounterKind, EffectSubject, PermanentTypeFilter,
    PowerComparison, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_misc1_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "riverguards_reflexes",
            "Riverguard's Reflexes",
            "{1}{W}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "exorcise",
            "Exorcise",
            "{1}{W}",
            &["Sorcery"][..],
            None,
            None,
        ),
        (
            "rock_soldiers",
            "Rock Soldiers",
            "{3}{R}",
            &["Artifact", "Creature", "Elemental", "Soldier"][..],
            Some(4),
            Some(3),
        ),
        (
            "aetherize",
            "Aetherize",
            "{3}{U}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "intrepid_tenderfoot",
            "Intrepid Tenderfoot",
            "{1}{G}",
            &["Creature", "Insect", "Citizen"][..],
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

    // Riverguard's Reflexes: +2/+2, first strike and untap on one target.
    let reflexes = primary(registry, "riverguards_reflexes");
    assert_eq!(
        reflexes.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(creature())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(creature())),
                keywords: vec![Keyword::FirstStrike],
            },
            SpellEffectKind::Untap {
                subject: EffectSubject::Chosen(Box::new(creature())),
            },
        ]
    );
    assert_eq!(
        reflexes.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0, 1, 2]
    );

    // Exorcise: exile artifact, enchantment, or a creature with power 4+.
    let exorcise = primary(registry, "exorcise");
    let SpellEffectKind::Exile {
        subject: EffectSubject::Chosen(filter),
    } = &exorcise.spell_effect[0]
    else {
        panic!("exile, got {:?}", exorcise.spell_effect[0]);
    };
    let any_of = filter.any_of.as_ref().expect("three branches");
    assert!(any_of
        .iter()
        .any(|f| f.permanent_types.contains(&PermanentTypeFilter::Artifact)));
    assert!(any_of.iter().any(|f| f
        .permanent_types
        .contains(&PermanentTypeFilter::Enchantment)));
    assert!(any_of
        .iter()
        .any(|f| f.kind == TargetKind::Creature && f.power == Some(PowerComparison::AtLeast(4))));

    // Rock Soldiers: entry destroy of a noncreature artifact, optional.
    let soldiers = primary(registry, "rock_soldiers");
    let [trigger] = soldiers.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    let SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(filter),
    } = &trigger.effect[0]
    else {
        panic!("destroy, got {:?}", trigger.effect[0]);
    };
    assert!(filter
        .permanent_types
        .contains(&PermanentTypeFilter::Artifact));
    assert!(filter
        .excluded_permanent_types
        .contains(&PermanentTypeFilter::Creature));
    let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("one group");
    };
    assert_eq!((group.min, group.max), (0, 1));

    // Aetherize: return all attacking creatures.
    let aetherize = primary(registry, "aetherize");
    assert!(
        matches!(&aetherize.spell_effect[0], SpellEffectKind::ReturnAllToOwnersHand { kind }
            if kind.kind == TargetKind::Creature && kind.combat_role == Some(CombatRole::Attacking)),
        "{:?}",
        aetherize.spell_effect[0]
    );

    // Intrepid Tenderfoot: {3} at sorcery speed, one +1/+1 counter on the source.
    let tenderfoot = primary(registry, "intrepid_tenderfoot");
    let [ability] = tenderfoot.activated_abilities.as_slice() else {
        panic!("one activated ability");
    };
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    assert_eq!(
        ability.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![1]),
        "the ability text is Oracle line 1"
    );
    assert!(
        matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{3}"),
        "{:?}",
        ability.costs
    );
    assert!(
        matches!(&ability.effect[0], SpellEffectKind::PutCounters { counter, count, subject: EffectSubject::Source }
            if *counter == CounterKind::PlusOnePlusOne && *count == tricerules_cards::primitives::Amount::Fixed(1)),
        "{:?}",
        ability.effect[0]
    );
}

fn creature() -> tricerules_cards::primitives::TargetFilter {
    tricerules_cards::primitives::TargetFilter {
        kind: TargetKind::Creature,
        ..tricerules_cards::primitives::TargetFilter::default()
    }
}

#[test]
fn issue_misc1_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "riverguards_reflexes",
            "Riverguard's Reflexes",
            "1903b4d5d9d232d1ff3548d7432d7dc049b72f316ccee732a8c1849f811b04f8",
        ),
        (
            "exorcise",
            "Exorcise",
            "d79fa2405230d6db5893b1d50f2fa39088083260e3daf6cc4376f04828ee1dcf",
        ),
        (
            "rock_soldiers",
            "Rock Soldiers",
            "edb6c7a32990c8d16dec8c331e2bc43c8b09663b4fed3f9c76f734eb164eab6b",
        ),
        (
            "aetherize",
            "Aetherize",
            "5f6e42d648b5836ccee6c5d5fb0f6b4d21eb45ff93e53e35077b1abdb7dc58e4",
        ),
        (
            "intrepid_tenderfoot",
            "Intrepid Tenderfoot",
            "2628a43d2fb40a85ec757db61eacb266e796e3879ae0b95750a76d63fe14cac6",
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
