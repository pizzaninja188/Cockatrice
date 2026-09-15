use tricerules_cards::primitives::{
    CardTypeFilter, PlayerRecipient, ResolutionBranchRequirement, ResolutionBranchSelection,
    ResolutionCost, SearchDestination, SearchZoneSelection, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, TriggerCondition};

fn assert_optional_basic_land_to_top(
    id: &str,
    name: &str,
    face_id: &str,
    types: &[&str],
    keywords: &[Keyword],
    oracle_line: u16,
    fingerprint: &str,
) {
    let registry = CardRegistry::global();
    let definition = registry
        .get(id)
        .unwrap_or_else(|| panic!("missing issue #284 card {id}"));
    assert_eq!(definition.name, name);
    assert_eq!(registry.id_for_name(name), Some(id));
    assert_eq!(definition.face_count(), 1);

    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), face_id);
    assert_eq!(face.name, name);
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(
        face.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types
    );
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.keywords, keywords);
    assert!(face.spell_effect.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.modal_spell.is_none());
    assert!(face.targeting.is_none());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.characteristic_defining_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 1);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("issue #284 has one ETB ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.modal.is_none());
    assert!(ability.targeting.is_none());
    assert!(ability.intervening_if.is_none());

    let [SpellEffectKind::ChooseResolutionBranch {
        chooser,
        optional,
        selection,
        branches,
        otherwise,
    }] = ability.effect.as_slice()
    else {
        panic!("issue #284 must use one optional resolution branch");
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert_eq!(*selection, ResolutionBranchSelection::PlayerChoice);
    assert!(otherwise.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("issue #284 must have one search branch");
    };
    assert_eq!(branch.branch_id.as_str(), "search_for_a_basic_land");
    assert_eq!(branch.presentation, AbilityPresentation::Fallback);
    assert_eq!(branch.cost, ResolutionCost::None);
    assert_eq!(branch.requirement, ResolutionBranchRequirement::Always);

    let [SpellEffectKind::SearchLibrary {
        who,
        optional: search_optional,
        count,
        count_by_cast_cost,
        filter: Some(filter),
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
    }] = branch.effects.as_slice()
    else {
        panic!("issue #284 branch must search the library");
    };
    assert_eq!(*who, PlayerRecipient::Controller);
    assert!(!search_optional);
    assert_eq!(*count, 1);
    assert!(count_by_cast_cost.is_none());
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
    assert!(filter.any_of.is_none());
    assert!(filter.exact_name.is_none());
    assert!(filter.excluded_card_types.is_empty());
    assert!(filter.required_subtypes.is_empty());
    assert!(slots.is_empty());
    assert_eq!(zones, &SearchZoneSelection::default());
    assert_eq!(*destination, SearchDestination::TopOfLibrary);
    assert!(conditional_destination.is_none());
    assert!(*shuffle);
    assert!(*reveal);
    assert!(result_id.is_none());

    let metadata = registry
        .presentation_face(id, face.face_id.as_str())
        .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
    assert_eq!(metadata.card_name, name);
    assert_eq!(metadata.face_name, name);
    assert_eq!(metadata.oracle_text_sha256, fingerprint);
}

#[test]
fn issue_284_cards_have_exact_registry_characteristics_and_typed_tree() {
    assert_optional_basic_land_to_top(
        "campus_guide",
        "Campus Guide",
        "campus_guide",
        &["Artifact", "Creature", "Golem"],
        &[],
        1,
        "8567e78bd1eed7e594f22544f7217deaa65310e980acc9fccfe43c4bbda1065b",
    );
    assert_optional_basic_land_to_top(
        "spider-bot",
        "Spider-Bot",
        "spider_bot",
        &["Artifact", "Creature", "Spider", "Robot", "Scout"],
        &[Keyword::Reach],
        2,
        "f82e59b17b68e675f21cc25304895eeac9b8bbd1776c9ffabbc94ce58a00a9be",
    );
}
