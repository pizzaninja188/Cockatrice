//! Registry conformance for the reviewed "can't attack or block" Aura family.

use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef, TargetKind};
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "bound_in_silence",
        "Bound in Silence",
        "{2}{W}",
        &["Kindred", "Enchantment", "Rebel", "Aura"],
        "5ac7468dba932c764ecd3d6b1b05dce9aee7267a22efc361f53fb904b069bbd9",
    ),
    (
        "detained_by_legionnaires",
        "Detained by Legionnaires",
        "{1}{W}",
        &["Enchantment", "Aura"],
        "5ac7468dba932c764ecd3d6b1b05dce9aee7267a22efc361f53fb904b069bbd9",
    ),
    (
        "luminous_bonds",
        "Luminous Bonds",
        "{2}{W}",
        &["Enchantment", "Aura"],
        "5ac7468dba932c764ecd3d6b1b05dce9aee7267a22efc361f53fb904b069bbd9",
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
fn aura_cant_attack_block_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, _) in CARDS {
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
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [SpellEffectKind::AuraAttach { target }] = f.spell_effect.as_slice() else {
            panic!("{id}: one Aura attach effect");
        };
        assert_eq!(target.kind, TargetKind::Creature, "{id}");
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        let StaticAbilityDef::AttachedModifier {
            restriction,
            delta_power,
            delta_toughness,
            ..
        } = &static_ability.definition
        else {
            panic!("{id}: attached modifier");
        };
        assert!(restriction.cant_attack, "{id}");
        assert!(restriction.cant_block, "{id}");
        assert_eq!((*delta_power, *delta_toughness), (0, 0), "{id}");
    }
}

#[test]
fn aura_cant_attack_block_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, expected) in CARDS {
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
