//! Registry conformance for the reviewed `Counter target creature spell.` family.
//!
//! Every included card is a normal-layout instant whose only effect counters one
//! target creature spell. Card names and costs are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{CardTypeFilter, SpellEffectKind};
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
        "false_summoning",
        "False Summoning",
        "{1}{U}",
        &["Instant"],
        None,
        "610b3ff41023168008e4b70606e2006b047d2609505b546d3617a13af020cbf9",
    ),
    (
        "preemptive_strike",
        "Preemptive Strike",
        "{1}{U}",
        &["Instant"],
        None,
        "610b3ff41023168008e4b70606e2006b047d2609505b546d3617a13af020cbf9",
    ),
    (
        "remove_soul",
        "Remove Soul",
        "{1}{U}",
        &["Instant"],
        None,
        "610b3ff41023168008e4b70606e2006b047d2609505b546d3617a13af020cbf9",
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
fn counter_creature_spell_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(
            f.targeting.is_none(),
            "{id}: the spell target is engine-inferred"
        );

        let [SpellEffectKind::CounterTargetSpell {
            spell_filter,
            unless_controller_pays,
            ..
        }] = f.spell_effect.as_slice()
        else {
            panic!("{id}: one creature-spell counter");
        };
        assert_eq!(
            spell_filter.card_type,
            Some(CardTypeFilter::Creature),
            "{id}"
        );
        assert!(unless_controller_pays.is_none(), "{id}");
    }
}

#[test]
fn counter_creature_spell_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
