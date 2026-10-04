use tricerules_cards::primitives::{
    CardTypeFilter, PlayerRecipient, SearchDestination, SpellEffectKind,
};
use tricerules_cards::{CardRegistry, Color, Layout};

#[test]
fn deploy_the_gatewatch_complete_definition_and_identity_are_exact() {
    let registry = CardRegistry::global();
    let card = registry.get("deploy_the_gatewatch").unwrap();
    assert_eq!(
        registry.id_for_name("Deploy the Gatewatch"),
        Some("deploy_the_gatewatch")
    );
    assert_eq!(card.face_count(), 1);
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.color_identity(), [Color::White]);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "deploy_the_gatewatch");
    assert_eq!(face.mana_cost.to_string(), "{4}{W}{W}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), [Color::White]);
    assert!(
        face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
            && face.triggered_abilities.is_empty()
    );
    assert!(face.targeting.is_none());
    assert_eq!(face.spell_effect, [SpellEffectKind::DeployTheGatewatch]);
    face.spell_effect[0]
        .validate(tricerules_cards::EffectContext::Spell)
        .unwrap();
}

fn assert_search_defaults(
    card_id: &str,
    face_id: &str,
    expected_name: &str,
    expected_destination: SearchDestination,
) -> tricerules_cards::primitives::ZoneCardFilter {
    let registry = CardRegistry::global();
    let card = registry
        .get(card_id)
        .unwrap_or_else(|| panic!("missing {card_id}"));
    assert_eq!(card.name, expected_name, "{card_id}");
    assert_eq!(card.layout, Layout::Normal, "{card_id}");
    assert_eq!(card.face_count(), 1, "{card_id}");

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), face_id, "{card_id}");
    assert_eq!(face.types, ["Sorcery"], "{card_id}");
    assert_eq!(face.mana_cost.to_string(), "{1}{G}", "{card_id}");
    assert_eq!(face.colors(), [Color::Green], "{card_id}");
    assert_eq!(face.spell_effect.len(), 1, "{card_id}");
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        filter: Some(filter),
        destination,
        shuffle,
        reveal,
        ..
    } = &face.spell_effect[0]
    else {
        panic!("{card_id} has a filtered SearchLibrary instruction");
    };
    assert_eq!(*who, PlayerRecipient::Controller, "{card_id}");
    assert!(
        !optional,
        "{card_id} does not let its controller decline to search"
    );
    assert_eq!(*count, 1, "{card_id}");
    assert_eq!(*destination, expected_destination, "{card_id}");
    assert!(*shuffle, "{card_id}");
    assert!(!reveal, "{card_id} does not reveal the found card");
    filter.clone()
}

#[test]
fn nature_s_lore_searches_for_a_forest_subtype_and_enters_untapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Nature's Lore"),
        Some("natures_lore")
    );
    let filter = assert_search_defaults(
        "natures_lore",
        "nature_s_lore",
        "Nature's Lore",
        SearchDestination::Battlefield { tapped: false },
    );
    assert_eq!(filter.required_subtypes, ["Forest"]);
    assert_eq!(filter.card_type, None);
}

#[test]
fn three_visits_searches_for_a_forest_subtype_and_enters_untapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Three Visits"),
        Some("three_visits")
    );
    let filter = assert_search_defaults(
        "three_visits",
        "three_visits",
        "Three Visits",
        SearchDestination::Battlefield { tapped: false },
    );
    assert_eq!(filter.required_subtypes, ["Forest"]);
    assert_eq!(filter.card_type, None);
}

#[test]
fn rampant_growth_searches_for_a_basic_land_and_enters_tapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Rampant Growth"),
        Some("rampant_growth")
    );
    let filter = assert_search_defaults(
        "rampant_growth",
        "rampant_growth",
        "Rampant Growth",
        SearchDestination::Battlefield { tapped: true },
    );
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
}

#[test]
fn farseek_uses_four_or_branches_for_land_subtypes_and_enters_tapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Farseek"),
        Some("farseek")
    );
    let filter = assert_search_defaults(
        "farseek",
        "farseek",
        "Farseek",
        SearchDestination::Battlefield { tapped: true },
    );
    assert!(filter.card_type.is_none());
    assert_eq!(filter.required_subtypes, Vec::<String>::new());
    let branches = filter
        .any_of
        .expect("Farseek's listed land types are alternatives");
    assert_eq!(
        branches
            .iter()
            .map(|branch| branch.required_subtypes.as_slice())
            .collect::<Vec<_>>(),
        vec![
            &["Plains".to_string()][..],
            &["Island".to_string()][..],
            &["Swamp".to_string()][..],
            &["Mountain".to_string()][..],
        ]
    );
    assert!(branches.iter().all(|branch| branch.any_of.is_none()));
}

// Myriad Landscape adds a relation between cards in the shared land-search family.
use tricerules_cards::primitives::*;
use tricerules_cards::AbilityPresentation;

#[test]
fn myriad_landscape_complete_definition_is_exact_and_all_lines_are_presented() {
    let registry = CardRegistry::global();
    let card = registry.get("myriad_landscape").unwrap();
    assert_eq!(
        registry.id_for_name("Myriad Landscape"),
        Some("myriad_landscape")
    );
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "myriad_landscape");
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.types, ["Land"]);
    assert!(
        face.supertypes.is_empty()
            && face.spell_effect.is_empty()
            && face.triggered_abilities.is_empty()
    );
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        face.static_abilities[0].definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            ..
        }
    ));
    assert_eq!(face.activated_abilities.len(), 2);
    let mana = &face.activated_abilities[0];
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert!(
        matches!(mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, restriction: None, conditional: None, commander_color_identity: false }] if options == &[ManaAmount { c: 1, ..Default::default() }])
    );
    let search = &face.activated_abilities[1];
    assert_eq!(
        search.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(
        matches!(search.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf] if cost.to_string() == "{2}")
    );
    assert!(search.targeting.is_none());
    assert!(
        matches!(search.effect.as_slice(), [SpellEffectKind::SearchLibrary {
        who: PlayerRecipient::Controller, optional: false, count: 2, count_by_cast_cost: None,
        filter: Some(filter), selection_constraint: Some(SearchSelectionConstraint::SharedLandType),
        slots, zones: SearchZoneSelection::Fixed(zones), destination: SearchDestination::Battlefield { tapped: true },
        conditional_destination: None, shuffle: true, reveal: false, result_id: None,
    }] if filter == &ZoneCardFilter { card_type: Some(CardTypeFilter::BasicLand), ..Default::default() }
       && slots.is_empty() && zones == &[CardSearchZone::Library])
    );
}

const SEARCH: &str = "SearchLibrary(count:2, filter:Some((card_type:Some(BasicLand))), selection_constraint:Some(SharedLandType), destination:Battlefield(tapped:true))";

#[test]
fn shared_land_search_constraint_is_typed_and_round_trips() {
    let effect: SpellEffectKind = ron::from_str(SEARCH).expect("typed Myriad search");
    effect.validate(EffectContext::Ability).unwrap();
    let saved = ron::to_string(&effect).unwrap();
    assert!(
        saved.contains("selection_constraint:Some(SharedLandType)"),
        "the relation must survive authored data, not be ignored: {saved}"
    );
    let restored: SpellEffectKind = ron::from_str(&saved).unwrap();
    assert_eq!(effect, restored);
}

#[test]
fn shared_land_search_rejects_every_unsupported_shape() {
    let invalid = [
        SEARCH.replace("count:2", "count:1"),
        SEARCH.replace("count:2", "count:0"),
        SEARCH.replace("count:2", "count:3"),
        SEARCH.replace("count:2", "optional:true,count:2"),
        SEARCH.replace("count:2", "who:EachPlayer,count:2"),
        SEARCH.replace("filter:Some((card_type:Some(BasicLand)))", "filter:None"),
        SEARCH.replace("BasicLand", "Land"),
        SEARCH.replace("destination:Battlefield(tapped:true)", "destination:Hand"),
        SEARCH.replace("tapped:true", "tapped:false"),
        SEARCH.replace("count:2", "zones:Fixed([Graveyard]),count:2"),
        SEARCH.replace("count:2", "zones:Fixed([Library,Hand]),count:2"),
        SEARCH.replace("count:2", "zones:PlayerChoice([Library]),count:2"),
        SEARCH.replace("count:2", "result_id:Some(\"found\"),count:2"),
        SEARCH.replace("count:2", "count_by_cast_cost:Some((condition:(group_id:\"kicker\",option_id:\"paid\",expected_selected:true),if_selected:2,otherwise:1)),count:2"),
        SEARCH.replace("count:2", "slots:[(slot_id:\"basic\",presentation:OracleLines([1]),filter:(card_type:Some(BasicLand)))],count:2"),
        SEARCH.replace("count:2", "conditional_destination:Some((condition:ControllerLibraryEmpty,destination:Hand)),count:2"),
    ];
    for source in invalid {
        let effect: SpellEffectKind = ron::from_str(&source).expect(&source);
        assert!(
            effect.validate(EffectContext::Ability).is_err(),
            "must reject: {source}"
        );
    }
}

#[test]
fn ordinary_searches_keep_their_default_schema_and_semantics() {
    let effect: SpellEffectKind = ron::from_str("SearchLibrary(filter:Some((card_type:Some(BasicLand))),destination:Battlefield(tapped:true))").unwrap();
    effect.validate(EffectContext::Ability).unwrap();
    let round_trip: SpellEffectKind = ron::from_str(&ron::to_string(&effect).unwrap()).unwrap();
    assert_eq!(effect, round_trip);
}

#[test]
fn into_the_wilds_complete_definition_and_oracle_presentation_are_exact() {
    let registry = CardRegistry::global();
    let card = registry.get("into_the_wilds").unwrap();
    assert_eq!(
        registry.id_for_name("Into the Wilds"),
        Some("into_the_wilds")
    );
    assert_eq!(card.face_count(), 1);
    assert_eq!(card.color_identity(), vec![Color::Green]);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "into_the_wilds");
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(
        face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
            && face.spell_effect.is_empty()
    );
    assert_eq!(face.triggered_abilities.len(), 1);
    let ability = &face.triggered_abilities[0];
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(!ability.may && ability.targeting.is_none());
    assert!(matches!(
        ability.trigger,
        TriggerCondition::AtBeginningOfUpkeep {
            player: CastTriggerPlayer::Controller
        }
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::IntoTheWilds]
    ));
}

#[test]
fn chaos_warp_complete_definition_has_exact_face_cost_identity_and_permanent_target() {
    let registry = CardRegistry::global();
    let card = registry.get("chaos_warp").unwrap();
    assert_eq!(registry.id_for_name("Chaos Warp"), Some("chaos_warp"));
    assert_eq!(card.face_count(), 1);
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.color_identity(), [Color::Red]);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "chaos_warp");
    assert_eq!(face.mana_cost.to_string(), "{2}{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), [Color::Red]);
    assert!(
        face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
            && face.triggered_abilities.is_empty()
            && face.modal_spell.is_none()
    );
    assert_eq!(face.spell_effect, [SpellEffectKind::ChaosWarp]);
    face.spell_effect[0]
        .validate(tricerules_cards::EffectContext::Spell)
        .unwrap();
    let groups = &face.targeting.as_ref().unwrap().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
    assert_eq!(groups[0].effect_indices, [0]);
    assert_eq!(face.spell_effect[0].target_roles().len(), 1);
}
