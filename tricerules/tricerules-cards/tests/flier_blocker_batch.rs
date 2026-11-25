//! Registry conformance for the reviewed Flying / can-block-only-fliers cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is
//! Flying plus "This creature can block only creatures with flying." Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.
use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

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
        "cloud_sprite",
        "Cloud Sprite",
        "{U}",
        &["Creature", "Faerie"],
        &[],
        Some((1, 1)),
        "c4c8f44d699ba3a1f51344f4a8cd0d250f96309ca498eb4e611a523dd18d7c04",
    ),
    (
        "scrapskin_drake",
        "Scrapskin Drake",
        "{2}{U}",
        &["Creature", "Zombie", "Drake"],
        &[],
        Some((2, 3)),
        "c4c8f44d699ba3a1f51344f4a8cd0d250f96309ca498eb4e611a523dd18d7c04",
    ),
    (
        "welkin_tern",
        "Welkin Tern",
        "{1}{U}",
        &["Creature", "Bird"],
        &[],
        Some((2, 1)),
        "c4c8f44d699ba3a1f51344f4a8cd0d250f96309ca498eb4e611a523dd18d7c04",
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
fn flier_blocker_batch_maps_definitions() {
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
        assert_eq!(f.keywords, [Keyword::Flying], "{id} keywords");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(!f.must_attack_if_able, "{id}");
        let [ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert!(
            matches!(
                &ability.definition,
                StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition: None,
                } if restriction.cant_block_creatures_matching == [
                    tricerules_cards::primitives::ObjectFilter {
                        kind: tricerules_cards::primitives::TargetKind::Creature,
                        excluded_keywords: vec![Keyword::Flying],
                        ..Default::default()
                    }
                ]
            ),
            "{id}: must block only creatures with flying"
        );
    }
}

#[test]
fn flier_blocker_batch_fingerprints_match_the_pinned_oracle_text() {
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
