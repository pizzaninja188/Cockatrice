//! Registry conformance for the reviewed `Destroy target artifact.` family.
//!
//! Every included card is a normal-layout instant whose only effect destroys
//! one target artifact. Card names and costs are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{
    EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind, TargetingDef,
};
use tricerules_cards::Layout;

type Card = (&'static str, &'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    (
        "shatter",
        "Shatter",
        "{1}{R}",
        "0e70d1cb13e4dac4028d88de06de11273bfca593a52a7f1725299ba8c1516749",
    ),
    (
        "smelt",
        "Smelt",
        "{R}",
        "0e70d1cb13e4dac4028d88de06de11273bfca593a52a7f1725299ba8c1516749",
    ),
    (
        "verdigris",
        "Verdigris",
        "{2}{G}",
        "0e70d1cb13e4dac4028d88de06de11273bfca593a52a7f1725299ba8c1516749",
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
fn destroy_artifact_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Instant"], "{id} type");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");

        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id}: exactly one spell effect");
        };
        let SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        } = effect
        else {
            panic!("{id}: destroy chosen permanent");
        };
        assert_eq!(filter.kind, TargetKind::AnyPermanent, "{id}");
        assert_eq!(
            filter.permanent_types,
            [PermanentTypeFilter::Artifact],
            "{id}"
        );

        let Some(TargetingDef { groups }) = &f.targeting else {
            panic!("{id}: one target group");
        };
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert_eq!(groups[0].prompt, "Choose target artifact", "{id}");
    }
}

#[test]
fn destroy_artifact_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, expected) in CARDS {
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
