//! Registry conformance for the reviewed Glint pump-and-hexproof family.
//!
//! Every included card is a normal-layout instant that pumps one chosen creature you
//! control and grants it hexproof until end of turn. Card names, costs and printed
//! pump values are data; the shared scenario executes every card. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    EffectSubject, Keyword, SpellEffectKind, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "beaming_defiance",
        "Beaming Defiance",
        "{1}{W}",
        2,
        2,
        "f4895234262d05750afddba4722e3cf70d8bcdad0abf1d163b793135cb0a4035",
    ),
    (
        "glint",
        "Glint",
        "{1}{U}",
        0,
        3,
        "6e4c005934626155ce42e8ab12e0019837070a835e42a0194a9bf4e4f6113009",
    ),
    (
        "woodcutters_grit",
        "Woodcutter's Grit",
        "{2}{G}",
        3,
        3,
        "07f4fd104e93db5026b72403834473d3efcb649414c1d50f2f5d9e64bbda6616",
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

fn chosen_creature_you() -> EffectSubject {
    EffectSubject::Chosen(Box::new(TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        ..TargetFilter::default()
    }))
}

#[test]
fn glint_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, power, toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, vec!["Instant".to_string()], "{id} type");
        assert!(f.keywords.is_empty(), "{id} has no keyword");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");

        let [pump, grant] = f.spell_effect.as_slice() else {
            panic!("{id}: exact pump then keyword grant");
        };
        assert!(
            matches!(
                pump,
                SpellEffectKind::PumpTarget {
                    power: printed_power,
                    toughness: printed_toughness,
                    subject: EffectSubject::Chosen(filter),
                    ..
                } if *printed_power == power as i32
                    && *printed_toughness == toughness as i32
                    && filter.kind == TargetKind::Creature
                    && filter.controller == TargetController::You
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert!(
            matches!(
                grant,
                SpellEffectKind::GrantKeywords { subject, keywords }
                    if subject == &chosen_creature_you()
                        && keywords == &vec![Keyword::Hexproof]
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
fn glint_batch_fingerprints_match_the_pinned_oracle_text() {
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
