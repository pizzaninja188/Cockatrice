//! Registry conformance for the reviewed `{T}: Untap target land` cohort.
//!
//! Every included card is a normal-layout creature with the printed
//! tap-to-untap-a-target-land activated ability and no other rules text.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::AbilityCost;
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "blossom_dryad",
        "Blossom Dryad",
        "{2}{G}",
        &["Creature", "Dryad"],
        &[],
        Some((2, 2)),
        "64cd5301df00db506c2df3fa8eb041a273404fa5658f57773920088668b939c5",
    ),
    (
        "juniper_order_druid",
        "Juniper Order Druid",
        "{2}{G}",
        &["Creature", "Human", "Cleric", "Druid"],
        &[],
        Some((1, 1)),
        "64cd5301df00db506c2df3fa8eb041a273404fa5658f57773920088668b939c5",
    ),
    (
        "ley_druid",
        "Ley Druid",
        "{2}{G}",
        &["Creature", "Human", "Druid"],
        &[],
        Some((1, 1)),
        "64cd5301df00db506c2df3fa8eb041a273404fa5658f57773920088668b939c5",
    ),
    (
        "voyaging_satyr",
        "Voyaging Satyr",
        "{1}{G}",
        &["Creature", "Satyr", "Druid"],
        &[],
        Some((1, 2)),
        "64cd5301df00db506c2df3fa8eb041a273404fa5658f57773920088668b939c5",
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
fn tap_untap_land_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
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
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Tap]),
            "{id}: tap cost"
        );
        let tricerules_cards::primitives::SpellEffectKind::Untap {
            subject: tricerules_cards::primitives::EffectSubject::Chosen(filter),
        } = &ability.effect[0]
        else {
            panic!("{id}: untap with a chosen subject");
        };
        assert_eq!(
            filter.kind,
            tricerules_cards::primitives::TargetKind::AnyPermanent,
            "{id}"
        );
        assert_eq!(
            filter.permanent_types,
            [tricerules_cards::primitives::PermanentTypeFilter::Land],
            "{id}"
        );
        assert_eq!(
            filter.controller,
            tricerules_cards::primitives::TargetController::Any,
            "{id}"
        );
        let targeting = ability.targeting.as_ref().expect("{id}: target");
        let [group] = targeting.groups.as_slice() else {
            panic!("{id}: one target group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.effect_indices, [0], "{id}");
    }
}

#[test]
fn tap_untap_land_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
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
