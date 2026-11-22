//! Registry conformance for the reviewed "enters, deal 1 damage to any target"
//! family.

use tricerules_cards::primitives::{Amount, SpellEffectKind, TargetKind, TriggerCondition};
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "akoum_boulderfoot",
        "Akoum Boulderfoot",
        "{4}{R}{R}",
        &["Creature", "Giant", "Warrior"],
        Some((4, 5)),
        "14f292f24ebd0bfcfc45335604e9fe68423cb6e50e9ae9bcc960c1d46bbfbe4b",
    ),
    (
        "blisterstick_shaman",
        "Blisterstick Shaman",
        "{2}{R}",
        &["Creature", "Goblin", "Shaman"],
        Some((2, 1)),
        "14f292f24ebd0bfcfc45335604e9fe68423cb6e50e9ae9bcc960c1d46bbfbe4b",
    ),
    (
        "sparkmage_apprentice",
        "Sparkmage Apprentice",
        "{1}{R}",
        &["Creature", "Human", "Wizard"],
        Some((1, 1)),
        "14f292f24ebd0bfcfc45335604e9fe68423cb6e50e9ae9bcc960c1d46bbfbe4b",
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
fn etb_damage_any_target_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
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
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(trigger.effect.len(), 1, "{id}");
        let [SpellEffectKind::DamageTarget { amount, target }] = trigger.effect.as_slice() else {
            panic!("{id}: one damage effect");
        };
        assert_eq!(*amount, Amount::Fixed(1), "{id}");
        assert_eq!(target.kind, TargetKind::AnyTarget, "{id}");
        let targeting = trigger.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(targeting.groups[0].effect_indices, [0], "{id}");
    }
}

#[test]
fn etb_damage_any_target_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, expected) in CARDS {
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
