//! Registry conformance for the reviewed Puncturing Blow damage-and-exile family.
//!
//! Every included card is a normal-layout red spell that deals printed damage to one
//! creature target and replaces that target's death this turn with exile. Card names,
//! costs, types, amounts and timing are data; the shared scenario executes every card.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{SpellEffectKind, TargetKind};
use tricerules_cards::{Amount, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "magma_spray",
        "Magma Spray",
        "{R}",
        "Instant",
        2,
        "22a13b783a0db26a4dde8d2e3bffaa53ff0f695e43741291320aef62254aebe1",
    ),
    (
        "puncturing_blow",
        "Puncturing Blow",
        "{2}{R}{R}",
        "Sorcery",
        5,
        "42b1333691b5bc94ba26431cef0b51c777268e860d810fd3a97ae6b9142c74aa",
    ),
    (
        "reduce_to_ashes",
        "Reduce to Ashes",
        "{4}{R}",
        "Sorcery",
        5,
        "73961d3c7663cde46192d89ab88ccbf42caa100402ace675231dda054373ff3c",
    ),
    (
        "scorchmark",
        "Scorchmark",
        "{1}{R}",
        "Instant",
        2,
        "ddafffd3fa0957e96fbf15a86619c54163269169e40aa664f31de3d73d83fb19",
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
fn puncturing_blow_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, kind, amount, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, vec![kind.to_string()], "{id} type");
        assert!(f.keywords.is_empty(), "{id} has no keyword");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");

        let [damage, exile] = f.spell_effect.as_slice() else {
            panic!("{id}: exact damage then exile replacement");
        };
        assert!(
            matches!(
                damage,
                SpellEffectKind::DamageTarget {
                    amount: dealt,
                    target,
                    ..
                } if *dealt == Amount::Fixed(amount) && target.kind == TargetKind::Creature
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert!(
            matches!(
                exile,
                SpellEffectKind::ExileIfWouldDieThisTurn { target }
                    if target.kind == TargetKind::Creature
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        let targeting = f.targeting.as_ref().expect("{id}: one target group");
        let [group] = targeting.groups.as_slice() else {
            panic!("{id}: one group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.effect_indices, [0, 1], "{id}");
    }
}

#[test]
fn puncturing_blow_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
