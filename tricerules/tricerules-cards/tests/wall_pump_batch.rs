//! Registry conformance for the queue-v19 defender self-pump cohort.
//!
//! Every included card is a normal-layout Wall with defender and one
//! `{cost}: This creature gets +X/+Y until end of turn.` activated ability. Card names,
//! costs, types and stats are printed data; the shared scenario executes every card.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    u32,
    u32,
    &'static str,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "wall_of_faith",
        "Wall of Faith",
        "{3}{W}",
        0,
        5,
        "{W}",
        0,
        1,
        "7bf21e67cc7848e4a5448941853897bd95458fdaa3442a3146f2caef737d84c1",
    ),
    (
        "wall_of_fire",
        "Wall of Fire",
        "{1}{R}{R}",
        0,
        5,
        "{R}",
        1,
        0,
        "e2068ccf9debcb33c8ba2f6f37774d495f6e7a32e730a316d699f27b31a4d6ad",
    ),
    (
        "wall_of_lava",
        "Wall of Lava",
        "{1}{R}{R}",
        1,
        3,
        "{R}",
        1,
        1,
        "e2bb8ee701232c76fea47b902bd3654bd6066f432f223e73b1e5ac40dde39d41",
    ),
    (
        "wall_of_opposition",
        "Wall of Opposition",
        "{3}{R}{R}",
        0,
        6,
        "{1}",
        1,
        0,
        "888d8898ad0ce7966eb4fb6587e7156791eb21d9ce405b349b2bb320601ccc1b",
    ),
    (
        "wall_of_water",
        "Wall of Water",
        "{1}{U}{U}",
        0,
        5,
        "{U}",
        1,
        0,
        "ca07b305850f57685369dfee1a03fe669652aff345ab74f7a1dec4f6b4234907",
    ),
];

#[test]
fn wall_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, power, toughness, pump_cost, delta_power, delta_toughness, _f) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, ["Creature", "Wall"], "{id} types");
        assert_eq!(
            (face.power, face.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        assert_eq!(face.keywords, [Keyword::Defender], "{id} keywords");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == pump_cost
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    scale: None,
                    subject: EffectSubject::Source,
                }] if *power == delta_power && *toughness == delta_toughness
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(ability.targeting.is_none(), "{id}: no targets");
    }
}

#[test]
fn wall_pump_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _power, _toughness, _cost, _dp, _dt, fingerprint) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
