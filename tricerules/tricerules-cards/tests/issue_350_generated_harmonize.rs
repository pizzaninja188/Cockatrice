//! Registry and presentation conformance for issue #350.
//!
//! The Harmonize cast-method template must register the two completed Standard identities with
//! their complete typed payloads: CR 702.180 the graveyard alternative cost on the generated
//! sorcery face, plus the preserved basic-land search and return-target-creature effects.

use tricerules_cards::primitives::{CardTypeFilter, SearchDestination};
use tricerules_cards::{CardRegistry, Color, Layout, SpellEffectKind};

#[test]
fn issue_350_registers_the_two_completed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id) in [
        ("roamers_routine", "Roamer's Routine", "roamer_s_routine"),
        ("urenis_rebuff", "Ureni's Rebuff", "ureni_s_rebuff"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        assert_eq!(definition.primary_face().face_id.as_str(), face_id);
    }
}

#[test]
fn issue_350_roamers_routine_keeps_the_basic_land_search_and_harmonize_cost() {
    let definition = CardRegistry::global()
        .get("roamers_routine")
        .expect("Roamer's Routine");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(
        face.harmonize_cost.as_ref().map(ToString::to_string),
        Some("{4}{G}".to_string())
    );
    assert!(face.flashback_cost.is_none());
    assert!(face.warp_cost.is_none());
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Green]);

    let [SpellEffectKind::SearchLibrary {
        optional,
        count,
        filter,
        destination,
        shuffle,
        reveal,
        ..
    }] = face.spell_effect.as_slice()
    else {
        panic!("Roamer's Routine must keep exactly one library-search effect");
    };
    assert!(!optional);
    assert_eq!(*count, 1);
    let filter = filter
        .as_ref()
        .expect("the search must filter by card type");
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
    assert_eq!(
        *destination,
        SearchDestination::Battlefield { tapped: true }
    );
    assert!(shuffle);
    assert!(!reveal);
    assert!(face.targeting.is_none());
    assert!(face.triggered_abilities.is_empty());
}

#[test]
fn issue_350_urenis_rebuff_keeps_the_return_effect_and_harmonize_cost() {
    let definition = CardRegistry::global()
        .get("urenis_rebuff")
        .expect("Ureni's Rebuff");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(
        face.harmonize_cost.as_ref().map(ToString::to_string),
        Some("{5}{U}".to_string())
    );
    assert!(face.flashback_cost.is_none());
    assert!(face.warp_cost.is_none());
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::ReturnToOwnersHand {
            subject: tricerules_cards::primitives::EffectSubject::Chosen(Box::new(
                tricerules_cards::primitives::TargetFilter::default_creature()
            )),
        }]
    );
    assert!(face.targeting.is_none());
    assert!(face.triggered_abilities.is_empty());
}

#[test]
fn issue_350_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "roamers_routine",
            "roamer_s_routine",
            "Roamer's Routine",
            "Roamer's Routine",
        ),
        (
            "urenis_rebuff",
            "ureni_s_rebuff",
            "Ureni's Rebuff",
            "Ureni's Rebuff",
        ),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id, card_name, face_name) in cases {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *face_name);
        assert_eq!(presentation.oracle_text_sha256.len(), 64);

        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
