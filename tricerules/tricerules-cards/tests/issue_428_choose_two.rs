//! Issue #428 registry conformance for the retained `Choose two —` identity.
//!
//! Return from the Wilds is the only fully shipped identity in the cohort: its three bullets are
//! an untargeted basic-land tutor, a plain white Human token, and a Food token, and the assembly
//! fixes the printed two-of-three selection (CR 700.2 / 700.2a). The five Command cards each
//! print at least one bullet with no shipped vocabulary, so they must stay unregistered rather
//! than generate partially.

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, PlayerRecipient, SearchDestination, SearchZoneSelection,
    SpellEffectKind, ZoneCardFilter,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, ModalDef};

fn modal(card_id: &str) -> ModalDef {
    let definition = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} is registered"));
    let face = definition.primary_face();
    assert!(
        face.spell_effect.is_empty(),
        "{card_id} keeps every bullet out of the ordinary spell-effect slot"
    );
    assert!(
        face.targeting.is_none(),
        "{card_id} keeps every bullet out of the face-level targeting slot"
    );
    face.modal_spell
        .clone()
        .unwrap_or_else(|| panic!("{card_id} is a modal spell"))
}

#[test]
fn issue_428_registers_return_from_the_wilds_and_keeps_unfinished_commands_out() {
    let registry = CardRegistry::global();
    let definition = registry
        .get("return_from_the_wilds")
        .expect("Return from the Wilds is registered");
    assert_eq!(definition.name, "Return from the Wilds");
    assert_eq!(
        registry.id_for_name("Return from the Wilds"),
        Some("return_from_the_wilds")
    );
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "return_from_the_wilds");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(
        face.types.iter().map(String::as_str).collect::<Vec<_>>(),
        ["Sorcery"]
    );
    assert!(face.keywords.is_empty());
    assert!(face.cast_cost_groups.is_empty());

    let modal = modal("return_from_the_wilds");
    assert_eq!((modal.min_modes, modal.max_modes), (2, 2));
    assert!(modal.all_modes_cast_cost.is_none());
    assert_eq!(modal.modes.len(), 3);
    assert_eq!(
        modal
            .modes
            .iter()
            .map(|mode| mode.mode_id.as_str())
            .collect::<Vec<_>>(),
        ["mode_01", "mode_02", "mode_03"]
    );
    assert_eq!(
        modal
            .modes
            .iter()
            .map(|mode| mode.presentation.clone())
            .collect::<Vec<_>>(),
        [
            AbilityPresentation::OracleLines(vec![2]),
            AbilityPresentation::OracleLines(vec![3]),
            AbilityPresentation::OracleLines(vec![4]),
        ]
    );
    assert!(
        modal.modes.iter().all(|mode| mode.targeting.is_none()),
        "every printed mode is untargeted"
    );

    for excluded in ["ashlings_command", "brigids_command", "grubs_command"] {
        assert!(
            registry.get(excluded).is_none(),
            "{excluded} must stay unregistered while a printed mode lacks shipped vocabulary"
        );
    }
}

#[test]
fn issue_428_search_mode_tutors_a_basic_land_onto_the_battlefield_tapped() {
    let modal = modal("return_from_the_wilds");
    assert_eq!(
        modal.modes[0].effects,
        [SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::BasicLand),
                ..ZoneCardFilter::default()
            }),
            slots: Vec::new(),
            zones: SearchZoneSelection::default(),
            destination: SearchDestination::Battlefield { tapped: true },
            conditional_destination: None,
            shuffle: true,
            reveal: false,
            result_id: None,
        }]
    );
}

#[test]
fn issue_428_human_and_food_modes_create_their_exact_registered_tokens() {
    let modal = modal("return_from_the_wilds");
    assert_eq!(
        modal.modes[1].effects,
        [SpellEffectKind::CreateTokens {
            token: "human_w_1_1".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert_eq!(
        modal.modes[2].effects,
        [SpellEffectKind::CreateTokens {
            token: "food".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );

    let registry = CardRegistry::global();
    assert!(registry.is_token("human_w_1_1"));
    let human = registry.get("human_w_1_1").expect("human token definition");
    assert_eq!(human.name, "Human");
    let human_face = human.primary_face();
    assert_eq!(
        human_face
            .types
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Creature", "Human"]
    );
    assert_eq!(human_face.power, Some(1));
    assert_eq!(human_face.toughness, Some(1));
    assert_eq!(
        human_face.colors_override.as_deref(),
        Some(&[tricerules_cards::primitives::Color::White][..])
    );

    let food = registry.get("food").expect("Food token definition");
    assert_eq!(food.name, "Food");
    assert!(
        food.primary_face()
            .types
            .iter()
            .any(|card_type| card_type == "Food"),
        "the predefined Food token keeps its artifact subtype"
    );
}
