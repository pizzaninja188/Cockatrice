//! Registry conformance for the reviewed "put target creature on top of its
//! owner's library" family.

use tricerules_cards::primitives::{
    EffectSubject, InstructionKind, LibraryPlacement, SpellEffectKind, TargetKind,
};
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
        "excommunicate",
        "Excommunicate",
        "{2}{W}",
        &["Sorcery"],
        "31532dd2a4b05bf10a021a1244094c9aa3e0221d3976cd031d525026cebb136e",
    ),
    (
        "forced_retreat",
        "Forced Retreat",
        "{2}{U}",
        &["Sorcery"],
        "31532dd2a4b05bf10a021a1244094c9aa3e0221d3976cd031d525026cebb136e",
    ),
    (
        "time_ebb",
        "Time Ebb",
        "{2}{U}",
        &["Sorcery"],
        "31532dd2a4b05bf10a021a1244094c9aa3e0221d3976cd031d525026cebb136e",
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
fn put_creature_on_top_batch_maps_definitions() {
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
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let program = f.spell_program.as_ref().expect("spell program");
        let [step] = program.body.steps.as_slice() else {
            panic!("{id}: one instruction");
        };
        let InstructionKind::Perform(effect) = &step.kind else {
            panic!("{id}: perform instruction");
        };
        let SpellEffectKind::PutInOwnersLibrary {
            subject: EffectSubject::Chosen(filter),
            placement,
        } = effect.as_ref()
        else {
            panic!("{id}: put-in-library effect");
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(*placement, LibraryPlacement::Top, "{id}");
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(targeting.groups[0].min, 1, "{id}");
        assert_eq!(targeting.groups[0].max, 1, "{id}");
    }
}

#[test]
fn put_creature_on_top_batch_fingerprints_match_the_pinned_oracle_text() {
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
