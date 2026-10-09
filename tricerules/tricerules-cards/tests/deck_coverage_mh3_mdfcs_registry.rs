//! Registry and typed-definition checks for the two Modern Horizons 3 land MDFCs.
//!
//! Source checked 2026-09-27. CR 712.11b-c / 712.12 govern modal-face choice, CR 614.1c
//! the life-paid tapped-entry replacement, CR 118.3b life payment, CR 701.14 fight, CR
//! 701.23-24 searching and shuffling, and CR 509.1b / 611.2a-c the temporary dynamic block
//! restriction.

use tricerules_cards::primitives::{
    AbilityCost, CombatRestriction, CombatRestrictionScope, EffectSubject, EntersTappedAffected,
    EntryCost, PlayerRecipient, SearchDestination, SpellEffectKind, StaticAbilityDef,
    TargetController, TargetFilter, TargetKind, TargetSchema,
};
use tricerules_cards::{AbilityPresentation, Color, Keyword, Layout};

const BRIDGEWORKS: &str = "bridgeworks_battle_tanglespan_bridgeworks";
const SUNDERING: &str = "sundering_eruption_volcanic_fissure";

fn assert_land_back(card_id: &str, face_id: &str, name: &str) {
    let definition = tricerules_cards::registry::global()
        .get(card_id)
        .expect("registered MDFC");
    assert_eq!(definition.layout, Layout::ModalDfc, "{card_id}");
    assert_eq!(definition.face_count(), 2, "{card_id}");
    let back = &definition.faces[1];
    assert_eq!(back.face_id.as_str(), face_id);
    assert_eq!(back.name, name);
    assert_eq!(back.mana_cost.to_string(), "");
    assert_eq!(back.types, ["Land"]);
    assert!(back.colors().is_empty(), "the land face is colorless");
    assert_eq!((back.power, back.toughness), (None, None));

    let [entry] = back.static_abilities.as_slice() else {
        panic!("{name} must have exactly one entry replacement");
    };
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        &entry.definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: Some(EntryCost::PayLife { amount: 3 }),
        }
    ));

    let [mana] = back.activated_abilities.as_slice() else {
        panic!("{name} must have exactly one mana ability");
    };
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert!(matches!(
        mana.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1
    ));
}

#[test]
fn pilot3_registers_both_complete_mdfc_identities_and_land_faces() {
    let registry = tricerules_cards::registry::global();
    for (id, name, front, cost, color) in [
        (
            BRIDGEWORKS,
            "Bridgeworks Battle // Tanglespan Bridgeworks",
            "Bridgeworks Battle",
            "{2}{G}",
            Color::Green,
        ),
        (
            SUNDERING,
            "Sundering Eruption // Volcanic Fissure",
            "Sundering Eruption",
            "{2}{R}",
            Color::Red,
        ),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, Layout::ModalDfc);
        assert_eq!(definition.face_count(), 2);
        let primary = definition.primary_face();
        assert_eq!(primary.name, front);
        assert_eq!(
            primary.face_id.as_str(),
            if id == BRIDGEWORKS {
                "bridgeworks_battle"
            } else {
                "sundering_eruption"
            }
        );
        assert_eq!(primary.mana_cost.to_string(), cost);
        assert_eq!(primary.types, ["Sorcery"]);
        assert!(primary.keywords.is_empty());
        assert_eq!((primary.power, primary.toughness), (None, None));
        assert_eq!(primary.colors(), vec![color]);
        let back = &definition.faces[1];
        assert_eq!(back.colors(), Vec::<Color>::new());
        assert_eq!(back.types, ["Land"]);
        assert_eq!((back.power, back.toughness), (None, None));
    }

    assert_land_back(
        BRIDGEWORKS,
        "tanglespan_bridgeworks",
        "Tanglespan Bridgeworks",
    );
    assert_land_back(SUNDERING, "volcanic_fissure", "Volcanic Fissure");
}

#[test]
fn bridgeworks_front_requires_its_controller_target_and_keeps_fight_optional() {
    let face = tricerules_cards::registry::global()
        .get(BRIDGEWORKS)
        .expect("Bridgeworks Battle")
        .primary_face();
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                })),
            },
            SpellEffectKind::Fight {
                first: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                })),
                second: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::Opponent,
                    ..TargetFilter::default()
                })),
            },
        ]
    );
    let groups = &face.targeting.as_ref().expect("two target groups").groups;
    assert_eq!(groups.len(), 2);
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
    assert_eq!(groups[0].prompt, "Choose target creature you control");
    assert_eq!(groups[0].effect_indices, [0, 1]);
    assert_eq!((groups[1].min, groups[1].max), (0, 1));
    assert_eq!(
        groups[1].prompt,
        "Choose up to one target creature you don't control"
    );
    assert_eq!(groups[1].effect_indices, [1]);
    assert_eq!(groups[0].distinct_from, [1]);
    assert_eq!(groups[1].distinct_from, [0]);
    TargetSchema::compile(&face.spell_effect, face.targeting.as_ref())
        .expect("the required and optional groups bind the pump and both fight subjects");
}

#[test]
fn sundering_front_keeps_target_controller_search_and_dynamic_restriction_ordered() {
    let face = tricerules_cards::registry::global()
        .get(SUNDERING)
        .expect("Sundering Eruption")
        .primary_face();
    assert_eq!(face.spell_effect.len(), 3);
    assert!(matches!(
        &face.spell_effect[0],
        SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        } if filter.kind == TargetKind::AnyPermanent
            && filter.permanent_types == [tricerules_cards::PermanentTypeFilter::Land]
    ));
    assert!(matches!(
        &face.spell_effect[1],
        SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
            optional: true,
            destination: SearchDestination::Battlefield { tapped: true },
            shuffle: true,
            ..
        }
    ));
    let SpellEffectKind::SearchLibrary { filter, .. } = &face.spell_effect[1] else {
        unreachable!()
    };
    assert!(
        format!("{filter:?}").contains("BasicLand"),
        "the search is restricted to basic lands"
    );
    assert!(matches!(
        &face.spell_effect[2],
        SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Matching(filter),
            restriction: CombatRestriction { cant_block: true, .. },
        } if filter.kind == TargetKind::Creature
            && filter.excluded_keywords == [Keyword::Flying]
    ));
    let groups = &face.targeting.as_ref().expect("target land").groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
    assert_eq!(groups[0].prompt, "Choose target land");
    assert_eq!(groups[0].effect_indices, [0]);
    TargetSchema::compile(&face.spell_effect, face.targeting.as_ref())
        .expect("the destroy effect is the sole required target");
}
