//! Exact authored-program and token conformance for Tend the Sprigs.
use tricerules_cards::primitives::{
    Amount, BattlefieldAggregate, CardSearchZone, CardTypeFilter, GameCondition, InstructionKind,
    PlayerRecipient, RelativePlayerSet, SearchDestination, SearchSelectionBounds,
    SearchZoneSelection, SpellEffectKind,
};

#[test]
fn tend_the_sprigs_has_the_exact_post_search_land_or_treefolk_program() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("tend_the_sprigs")
        .expect("Tend the Sprigs is registered");
    let face = card.primary_face();
    assert_eq!(card.name, "Tend the Sprigs");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Sorcery"]);
    assert!(
        face.spell_effect.is_empty(),
        "the spell is fully authored as a program"
    );

    let program = face
        .spell_program
        .as_ref()
        .expect("Tend's search and count are one ordered spell program");
    assert!(program.body.yields.is_empty());
    assert!(program.target_bindings.is_empty());
    let [search_instruction, threshold_instruction] = program.body.steps.as_slice() else {
        panic!("Tend searches first and checks its count second");
    };
    assert_eq!(search_instruction.id, "search_basic_land");
    let InstructionKind::Perform(search_effect) = &search_instruction.kind else {
        panic!("Tend's first instruction is its direct search");
    };
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        count_by_cast_cost,
        filter,
        selection_constraint,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
        selection_bounds,
    } = search_effect.as_ref()
    else {
        panic!("Tend's first instruction searches a library");
    };
    assert_eq!(*who, PlayerRecipient::Controller);
    assert!(!optional, "the search instruction itself is mandatory");
    assert_eq!(*count, 1);
    assert!(count_by_cast_cost.is_none());
    let filter = filter
        .as_ref()
        .expect("the search is restricted to Basic Lands");
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
    assert!(filter.any_of.is_none());
    assert!(filter.exact_name.is_none());
    assert!(filter.required_subtypes.is_empty());
    assert!(filter.required_supertypes.is_empty());
    assert!(filter.excluded_card_types.is_empty());
    assert!(selection_constraint.is_none());
    assert!(slots.is_empty());
    assert_eq!(
        zones,
        &SearchZoneSelection::Fixed(vec![CardSearchZone::Library])
    );
    assert_eq!(
        *destination,
        SearchDestination::Battlefield { tapped: true }
    );
    assert!(conditional_destination.is_none());
    assert!(*shuffle);
    assert!(!reveal);
    assert!(result_id.is_none());
    assert_eq!(
        *selection_bounds,
        Some(SearchSelectionBounds { min: 0, max: 1 }),
        "quality-based search retains the legal fail-to-find choice"
    );

    assert_eq!(threshold_instruction.id, "seven_land_or_treefolk");
    let InstructionKind::If { condition, yes, no } = &threshold_instruction.kind else {
        panic!("Tend checks the post-search battlefield with an instruction-time If");
    };
    let GameCondition::BattlefieldAggregate {
        filter,
        aggregate,
        min,
        max,
        exclude_observed_object,
    } = condition.as_ref()
    else {
        panic!("Tend uses the general battlefield aggregate matcher");
    };
    assert_eq!(*aggregate, BattlefieldAggregate::Count);
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert!(filter.required_supertypes.is_empty());
    assert!(filter.token.is_none());
    assert!(filter.card_type.is_none());
    assert!(filter.required_subtypes.is_empty());
    assert!(filter.color.is_none());
    assert!(filter.name.is_none());
    assert!(!filter.exclude_source);
    assert_eq!(*min, Some(7));
    assert_eq!(*max, None);
    assert!(!exclude_observed_object);

    let [land_filter, treefolk_filter] = filter
        .any_of
        .as_ref()
        .expect("the qualifying filter is a Land-or-Treefolk union")
        .as_slice()
    else {
        panic!("Tend has exactly two union branches");
    };
    assert_eq!(land_filter.controllers, RelativePlayerSet::Controller);
    assert!(land_filter.required_supertypes.is_empty());
    assert!(land_filter.token.is_none());
    assert_eq!(land_filter.card_type, Some(CardTypeFilter::Land));
    assert!(land_filter.color.is_none());
    assert!(land_filter.name.is_none());
    assert!(land_filter.required_subtypes.is_empty());
    assert!(land_filter.any_of.is_none());
    assert!(!land_filter.exclude_source);
    assert_eq!(treefolk_filter.controllers, RelativePlayerSet::Controller);
    assert!(treefolk_filter.required_supertypes.is_empty());
    assert!(treefolk_filter.token.is_none());
    assert!(treefolk_filter.card_type.is_none());
    assert!(treefolk_filter.color.is_none());
    assert!(treefolk_filter.name.is_none());
    assert_eq!(treefolk_filter.required_subtypes, ["Treefolk"]);
    assert!(treefolk_filter.any_of.is_none());
    assert!(!treefolk_filter.exclude_source);

    let [make_treefolk] = yes.steps.as_slice() else {
        panic!("the threshold creates one Treefolk token");
    };
    let InstructionKind::Perform(effect) = &make_treefolk.kind else {
        panic!("Tend's true body creates a token");
    };
    assert_eq!(
        **effect,
        SpellEffectKind::CreateTokens {
            token: "treefolk_g_3_4_reach".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }
    );
    assert!(no.steps.is_empty());

    let token = registry
        .get("treefolk_g_3_4_reach")
        .expect("the existing Treefolk token remains registered");
    assert!(registry.is_token("treefolk_g_3_4_reach"));
    let token_face = token.primary_face();
    assert_eq!(token_face.types, ["Creature", "Treefolk"]);
    assert_eq!((token_face.power, token_face.toughness), (Some(3), Some(4)));
    assert_eq!(token_face.colors(), [tricerules_cards::Color::Green]);
    assert!(token_face
        .keywords
        .contains(&tricerules_cards::Keyword::Reach));
}
