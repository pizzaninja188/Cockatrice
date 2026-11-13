//! Registry conformance for the reviewed attack-trigger self-pump family.
//!
//! Every card is a normal-layout creature whose only rules text is a self-pump attack trigger.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "benalish_veteran",
        "Benalish Veteran",
        "{2}{W}",
        &["Creature", "Human", "Soldier"],
        2,
        2,
        1,
        1,
        "66fbc1a1fcf5bdf22934a8a250c6cd41a50b497a3f7055aafc9077b504595aaf",
    ),
    (
        "bramble_creeper",
        "Bramble Creeper",
        "{4}{G}",
        &["Creature", "Elemental"],
        0,
        3,
        5,
        0,
        "d162a160ce56bd8f46dc6bad4afc92b8168fc8417592f0849fe7b6e4af85d742",
    ),
    (
        "charging_paladin",
        "Charging Paladin",
        "{2}{W}",
        &["Creature", "Human", "Knight"],
        2,
        2,
        0,
        3,
        "d96f3c1710cb24e142a8199a1f697b0e97854d1b804da61e86cc1130e5b0949c",
    ),
    (
        "graceful_cat",
        "Graceful Cat",
        "{2}{W}",
        &["Creature", "Cat"],
        2,
        2,
        1,
        1,
        "66fbc1a1fcf5bdf22934a8a250c6cd41a50b497a3f7055aafc9077b504595aaf",
    ),
    (
        "kiln_walker",
        "Kiln Walker",
        "{3}",
        &["Artifact", "Creature", "Phyrexian", "Construct"],
        0,
        3,
        3,
        0,
        "a76291ff08786f792ebd16f293be7905b3d4c468c57b33ed81de11444a8f48de",
    ),
    (
        "reckless_pangolin",
        "Reckless Pangolin",
        "{2}{G}",
        &["Creature", "Pangolin"],
        2,
        2,
        1,
        1,
        "66fbc1a1fcf5bdf22934a8a250c6cd41a50b497a3f7055aafc9077b504595aaf",
    ),
    (
        "steadfast_cathar",
        "Steadfast Cathar",
        "{1}{W}",
        &["Creature", "Human", "Soldier"],
        2,
        1,
        0,
        2,
        "7f5a59584eef979bacfccf2598c4045355c9b81d7ca72d8790a174af64af09bb",
    ),
];

#[test]
fn attack_pump_values_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, dp, dt, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one attack trigger");
        };
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0
            },
            "{id}"
        );
        assert!(
            matches!(
                triggered.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power: p, toughness: t, subject: EffectSubject::Source, .. }]
                    if *p == dp && *t == dt
            ),
            "{id}: source-only pump"
        );
    }
}

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn attack_pump_values_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, .., expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
