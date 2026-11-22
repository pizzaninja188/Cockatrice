//! Registry conformance for the reviewed tap-for-two-damage self-damage cohort.
//!
//! Every included card is a normal-layout creature with one tap-activated ability that deals 2 damage to any target and 3 damage to its controller.
//! Each included card is executed by the shared actual-card scenarios.
use tricerules_cards::primitives::{AbilityCost, PlayerRecipient, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Amount, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "goblin_artillery",
        "Goblin Artillery",
        "{1}{R}{R}",
        &["Creature", "Goblin", "Warrior"],
        &[],
        Some((1, 3)),
        "d97e8dd164e52a17f3f3a24c7c7ffb82c280514e34ff9ddd5f81647f0d77e086",
    ),
    (
        "orcish_cannoneers",
        "Orcish Cannoneers",
        "{1}{R}{R}",
        &["Creature", "Orc", "Warrior"],
        &[],
        Some((1, 3)),
        "d97e8dd164e52a17f3f3a24c7c7ffb82c280514e34ff9ddd5f81647f0d77e086",
    ),
    (
        "orcish_artillery",
        "Orcish Artillery",
        "{1}{R}{R}",
        &["Creature", "Orc", "Warrior"],
        &[],
        Some((1, 3)),
        "d97e8dd164e52a17f3f3a24c7c7ffb82c280514e34ff9ddd5f81647f0d77e086",
    ),
];

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
fn tap_ping_self_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Tap]),
            "{id}"
        );
        let [SpellEffectKind::DamageTarget {
            amount: dealt,
            target,
        }, SpellEffectKind::DamagePlayer {
            amount: self_damage,
            who: PlayerRecipient::Controller,
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: damage then self-damage");
        };
        assert_eq!(*dealt, Amount::Fixed(2), "{id}");
        assert_eq!(target.kind, TargetKind::AnyTarget, "{id}");
        assert_eq!(*self_damage, Amount::Fixed(3), "{id}");
        let targeting = ability.targeting.as_ref().expect("{id}: any target");
        let [group] = targeting.groups.as_slice() else {
            panic!("{id}: one target group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.effect_indices, [0], "{id}");
    }
}

#[test]
fn tap_ping_self_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
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
