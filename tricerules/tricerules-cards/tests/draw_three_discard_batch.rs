//! Registry conformance for the reviewed "Draw three cards, then discard a card" family.
//!
//! Every included card is a normal-layout card with the same spells that draw three and discard one
//! shape. Card names, costs, types and bodies are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{
    Amount, DiscardQuantity, InstructionKind, PlayerRecipient, SpellEffectKind,
};
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
        "enhanced_awareness",
        "Enhanced Awareness",
        "{4}{U}",
        &["Instant"],
        None,
        "3e8601598ee8dd12b4d5aea9e195eb324fb6aa3a71ed36e86235a17557084f32",
    ),
    (
        "rain_of_revelation",
        "Rain of Revelation",
        "{3}{U}",
        &["Instant"],
        None,
        "3e8601598ee8dd12b4d5aea9e195eb324fb6aa3a71ed36e86235a17557084f32",
    ),
    (
        "sift",
        "Sift",
        "{3}{U}",
        &["Sorcery"],
        None,
        "3e8601598ee8dd12b4d5aea9e195eb324fb6aa3a71ed36e86235a17557084f32",
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
fn draw_three_discard_batch_maps_definitions() {
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
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let program = f.spell_program.as_ref().expect("{id}: compiled program");
        let [draw, discard] = program.body.steps.as_slice() else {
            panic!("{id}: draw then discard");
        };
        assert!(
            matches!(
                &draw.kind,
                InstructionKind::Perform(effect)
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::Draw {
                            who: PlayerRecipient::Controller,
                            count: Amount::Fixed(3),
                        }
                    )
            ),
            "{id}: draws exactly three"
        );
        assert!(
            matches!(
                &discard.kind,
                InstructionKind::Perform(effect)
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::Discard {
                            who: PlayerRecipient::Controller,
                            quantity: DiscardQuantity::Exact(1),
                        }
                    )
            ),
            "{id}: discards exactly one"
        );
    }
}

#[test]
fn draw_three_discard_batch_fingerprints_match_the_pinned_oracle_text() {
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
