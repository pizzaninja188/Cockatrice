//! Registry conformance for the reviewed Mindstatic soft-counter family.
//!
//! Every included card is a normal-layout instant that counters one targeted spell unless
//! that spell's controller pays the printed generic amount. Card names, costs and payment
//! amounts are data; the shared scenario executes every card. Each fingerprint is SHA-256
//! of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, SpellEffectKind};
use tricerules_cards::Layout;

type Card = (&'static str, &'static str, &'static str, u32, &'static str);

const CARDS: &[Card] = &[
    (
        "force_spike",
        "Force Spike",
        "{U}",
        1,
        "3a0b8061274b0df3eeed8aa556f4f8ab16f4f13fc99945d1eef6827bd7f17cad",
    ),
    (
        "mana_leak",
        "Mana Leak",
        "{1}{U}",
        3,
        "d9c333febe909c6242ca125c47507735441811bdd40654e70ed7416a5e51f7d5",
    ),
    (
        "mana_tithe",
        "Mana Tithe",
        "{W}",
        1,
        "3a0b8061274b0df3eeed8aa556f4f8ab16f4f13fc99945d1eef6827bd7f17cad",
    ),
    (
        "mindstatic",
        "Mindstatic",
        "{3}{U}",
        6,
        "18aba727a3bdde05354d3cc2f58e60e2bb760e4f79241325c7741c8ba22756d3",
    ),
    (
        "quench",
        "Quench",
        "{1}{U}",
        2,
        "11a84c1b1e9cfe18ffd11df2e136ce1fedb27246af60b2a6f9a559c9fbb05cdf",
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
fn mindstatic_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, payment, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Instant"], "{id} type");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(f.power, None, "{id}");
        assert_eq!(f.toughness, None, "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(
            f.targeting.is_none(),
            "{id}: the spell chooses its implicit target"
        );
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::CounterTargetSpell {
                    unless_controller_pays: Some(Amount::Fixed(printed)),
                    ..
                }] if *printed == payment
            ),
            "{id}: exact optional payment amount"
        );
    }
}

#[test]
fn mindstatic_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
