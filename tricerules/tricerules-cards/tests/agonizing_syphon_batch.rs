//! Registry conformance for the reviewed Agonizing Syphon damage-and-life family.
//!
//! Every included card is a normal-layout spell with one shared any-target damage
//! effect followed by life gain for the caster. Card names, costs and numbers are
//! printed data; the shared scenario executes every card. Each fingerprint is SHA-256
//! of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{SpellEffectKind, TargetKind};
use tricerules_cards::{Amount, Layout};

type Card = (&'static str, &'static str, &'static str, &'static str, i32);

const CARDS: &[Card] = &[
    (
        "agonizing_syphon",
        "Agonizing Syphon",
        "{3}{B}",
        "Sorcery",
        3,
    ),
    ("essence_drain", "Essence Drain", "{4}{B}", "Sorcery", 3),
    (
        "vampiric_feast",
        "Vampiric Feast",
        "{5}{B}{B}",
        "Sorcery",
        4,
    ),
    ("void_helix", "Void Helix", "{4}{B}{B}", "Instant", 5),
    (
        "warleaders_helix",
        "Warleader's Helix",
        "{2}{R}{W}",
        "Instant",
        4,
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
fn agonizing_syphon_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, kind, amount) in CARDS {
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

        let [damage, gain] = f.spell_effect.as_slice() else {
            panic!("{id}: exact damage then life");
        };
        assert!(
            matches!(
                damage,
                SpellEffectKind::DamageTarget {
                    amount: dealt,
                    target,
                    ..
                } if *dealt == Amount::Fixed(amount as u32) && target.kind == TargetKind::AnyTarget
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert!(
            matches!(gain, SpellEffectKind::GainLife { amount: gained, .. } if *gained == Amount::Fixed(amount as u32)),
            "{id}: life gain"
        );
        let targeting = f.targeting.as_ref().expect("{id}: one target group");
        let [group] = targeting.groups.as_slice() else {
            panic!("{id}: one group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.effect_indices, [0], "{id}");
    }
}

#[test]
fn agonizing_syphon_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _) in CARDS {
        let expected = match id {
            "agonizing_syphon" => {
                "9d66ac4014ea604a9e6809638efdccf4ff836224bb5c93cdfedfbc3a3672e578"
            }
            "essence_drain" => "982ce372e49c4eb3b0a0b7b2f9638ec8ec0f1a45949390b2e98bfb842fa6e46a",
            "vampiric_feast" => "1f3dd4e44b6a7b4eb164d918b4ff750d4c0d7ce1d59d3455869f893a191253f4",
            "void_helix" => "221f70b1d23c56ab1057d1c75f511c3abe7ee65d8a6f5be2b29a76a5802eb50a",
            "warleaders_helix" => {
                "dbe0000ddae3ca29ed5eb823f6771d02d6b0c3fa933b5ebf76c31c8ae5fa8e42"
            }
            _ => unreachable!(),
        };
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
