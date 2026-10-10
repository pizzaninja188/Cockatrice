use tricerules_cards::registry::RegistryError;
use tricerules_cards::CardRegistry;

const CHILD: &str = r#"(
    ability_id: "activated_01", presentation: Fallback,
    costs: [Tap],
    effect: [ProduceMana(options: [(w: 1), (u: 1), (b: 1), (r: 1), (g: 1)])],
)"#;

fn card(filter: &str, children: &str) -> String {
    format!(
        r#"(
        id: "grant_probe", name: "Grant Probe", face_id: "grant_probe",
        mana_cost: "{{3}}", types: ["Artifact"],
        static_abilities: [(
            ability_id: "static_01", presentation: Fallback,
            definition: GrantActivatedAbilityToPermanents(
                filter: {filter}, activated_abilities: [{children}],
            ),
        )],
    )"#
    )
}

fn invalid(filter: &str, children: &str, reason_fragment: &str) {
    let ron = card(filter, children);
    let result = CardRegistry::from_chunks_and_tokens(&[&ron], &[]);
    assert!(
        matches!(result, Err(RegistryError::InvalidCard { ref reason, .. })
        if reason.contains(reason_fragment)),
        "expected {reason_fragment}: {result:?}"
    );
}

#[test]
fn lantern_and_rite_shaped_grants_validate_without_admitting_rite() {
    for filter in [
        "(kind: AnyPermanent, controller: You, permanent_types: [Land])",
        "(kind: Creature, controller: You)",
    ] {
        let ron = card(filter, CHILD);
        let registry = CardRegistry::from_chunks_and_tokens(&[&ron], &[])
            .expect("dynamic land and creature grants are supported");
        assert!(registry
            .get("grant_probe")
            .unwrap()
            .color_identity()
            .is_empty());
        assert!(registry.get("cryptolith_rite").is_none());
    }
}

#[test]
fn grant_children_require_nonempty_unique_battlefield_nonintrinsic_definitions() {
    let filter = "(kind: AnyPermanent)";
    invalid(filter, "", "at least one ability");
    invalid(
        filter,
        &format!("{CHILD}, {CHILD}"),
        "duplicate sibling ability id",
    );
    for zone in ["Hand", "Graveyard"] {
        invalid(
            filter,
            &CHILD.replace("costs:", &format!("source_zone: {zone}, costs:")),
            "battlefield",
        );
    }
    invalid(
        filter,
        &CHILD.replace("costs:", "intrinsic_land_mana: true, costs:"),
        "intrinsic land mana",
    );
    invalid(
        filter,
        &CHILD.replace("activated_01", "Bad-ID"),
        "canonical",
    );
}

#[test]
fn grant_scope_rejects_later_layer_predicates_recursively() {
    for filter in [
        "(kind: AnyPlayer)",
        "(kind: AnyPermanent, tapped: true)",
        "(kind: Creature, required_keywords: [Flying])",
        "(kind: Creature, excluded_keywords: [Haste])",
        "(kind: AnyPermanent, controller: DefendingPlayer)",
        "(kind: AnyPermanent, excluded_objects: [AttachedObject])",
        "(any_of: [(kind: Creature), (kind: AnyPermanent, tapped: false)])",
    ] {
        invalid(filter, CHILD, "earlier-layer scope");
    }
    let ron = card("(kind: AnyPermanent, excluded_objects: [Source])", CHILD);
    CardRegistry::from_chunks_and_tokens(&[&ron], &[]).unwrap();
}

#[test]
fn grant_child_shape_and_colored_costs_are_traversed() {
    invalid(
        "(kind: AnyPermanent)",
        &CHILD.replace(
            "effect: [ProduceMana(options: [(w: 1), (u: 1), (b: 1), (r: 1), (g: 1)])]",
            "effect: []",
        ),
        "at least one effect",
    );
    let ron = card(
        "(kind: Creature)",
        &CHILD.replace("[Tap]", "[Mana(\"{U}\")]"),
    );
    let registry = CardRegistry::from_chunks_and_tokens(&[&ron], &[]).unwrap();
    assert_eq!(
        registry.get("grant_probe").unwrap().color_identity().len(),
        1
    );
}

fn child_with_effect(effect: &str) -> String {
    format!(
        r#"(ability_id: "activated_01", presentation: Fallback, costs: [], effect: [{effect}])"#
    )
}

#[test]
fn scoped_children_validate_tokens_in_cards_and_tokens_including_nested_grants() {
    let treasure = include_str!("../data/tokens/treasure.ron");
    for effect in [
        r#"CreateTokens(token: "treasure", count: 1)"#.to_string(),
        format!(
            r#"ApplyPermanentModifier(subject: Source, modifier: GrantActivatedAbility({}), duration: UntilEndOfTurn)"#,
            child_with_effect(r#"CreateTokens(token: "treasure", count: 1)"#)
        ),
    ] {
        let ron = card("(kind: AnyPermanent)", &child_with_effect(&effect));
        let token_ron = ron.replace(r#"mana_cost: "{3}", "#, "");
        for is_token in [false, true] {
            let result = if is_token {
                CardRegistry::from_chunks_and_tokens(&[], &[&token_ron])
            } else {
                CardRegistry::from_chunks_and_tokens(&[&ron], &[])
            };
            assert!(
                matches!(result, Err(RegistryError::InvalidCard { ref reason, .. }) if reason.contains("unknown token 'treasure'")),
                "{result:?}"
            );
            let result = if is_token {
                CardRegistry::from_chunks_and_tokens(&[], &[&token_ron, treasure])
            } else {
                CardRegistry::from_chunks_and_tokens(&[&ron], &[treasure])
            };
            result.expect("a loaded token satisfies the dependency");
        }
        let ordinary =
            r#"(id: "treasure", name: "Treasure", face_id: "treasure", types: ["Artifact"])"#;
        assert!(CardRegistry::from_chunks_and_tokens(&[ordinary, &ron], &[]).is_err());
    }
}

#[test]
fn scoped_children_fail_closed_for_recipient_layout_and_cast_receipts() {
    for effect in [
        "ChangeSourceFace(action: Transform)",
        "AttachSource(target: (kind: Creature))",
        "PutCounters(counter: PlusOnePlusOne, count: 1, subject: AttachedObject)",
    ] {
        invalid(
            "(kind: AnyPermanent)",
            &child_with_effect(effect),
            "recipient layout or attachment",
        );
    }
    let reduction = CHILD.replace("costs:", "cost_modifiers: [ConditionalSourceManaCostReduction(condition: PermanentsEnteredThisTurn(controllers: All, filter: (source_only: true), min: 1))], costs:");
    invalid(
        "(kind: AnyPermanent)",
        &reduction,
        "source mana cost reduction",
    );
    let receipt = "ConditionalCastCost(condition: (group_id: \"cast_01\", option_id: \"option_01\", expected_selected: true), effect: GainLife(amount: 1))";
    invalid(
        "(kind: AnyPermanent)",
        &child_with_effect(receipt),
        "unknown group",
    );
}

#[test]
fn scoped_children_reject_payment_receipts_hidden_in_instruction_wrappers() {
    let receipt = "Draw(count: Count(CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller))))";
    let wrapped = format!("Conditional(condition: ControllerLibraryEmpty, effect: {receipt})");
    invalid(
        "(kind: AnyPermanent)",
        &child_with_effect(&wrapped),
        "compatible card cost",
    );
}

#[test]
fn scoped_children_reject_engine_synthesized_context() {
    for effect in ["SiegeDefeat", "CastMadness(cost: \"{R}\")"] {
        invalid(
            "(kind: AnyPermanent)",
            &child_with_effect(effect),
            "engine-synthesized",
        );
    }
}

#[test]
fn scoped_nested_activations_use_their_own_payment_receipts() {
    let draw = "Conditional(condition: ControllerLibraryEmpty, effect: Draw(count: Count(CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller)))))";
    let nested = child_with_effect(draw).replace("costs: []", "costs: [Discard]");
    let effect = format!("ApplyPermanentModifier(subject: Source, modifier: GrantActivatedAbility({nested}), duration: UntilEndOfTurn)");
    let ron = card("(kind: AnyPermanent)", &child_with_effect(&effect));
    CardRegistry::from_chunks_and_tokens(&[&ron], &[])
        .expect("nested activation owns its discard receipt");
    let nested_without_cost = effect.replace("costs: [Discard]", "costs: []");
    let parent =
        child_with_effect(&nested_without_cost).replacen("costs: []", "costs: [Discard]", 1);
    invalid("(kind: AnyPermanent)", &parent, "compatible card cost");
}

#[test]
fn scoped_nested_triggered_definitions_cannot_borrow_cast_cost_metadata() {
    let targeting = r#"Some((groups: [(min: 1, max: 1, prompt: "Choose a target", effect_indices: [0], cast_cost_expansion: Some((condition: (group_id: "cast_01", option_id: "option_01", expected_selected: true), without_cost: (kind: Creature))))]))"#;
    let effect = format!(
        r#"GrantTriggeredAbility(subject: Source, ability: (
        ability_id: "nested_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield,
        effect: [DamageTarget(amount: 1, target: (kind: AnyTarget))], targeting: {targeting},
    ))"#
    );
    invalid(
        "(kind: AnyPermanent)",
        &child_with_effect(&effect),
        "cast-cost",
    );
    let effect = r#"GrantTriggeredAbility(subject: Source, ability: (
        ability_id: "nested_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield,
        modal: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "mode_01", presentation: Fallback, linked_cast_cost: Some((group_id: "cast_01", option_id: "option_01")), effects: [GainLife(amount: 1)]),
            (mode_id: "mode_02", presentation: Fallback, effects: [GainLife(amount: 2)]),
        ])),
    ))"#;
    invalid(
        "(kind: AnyPermanent)",
        &child_with_effect(effect),
        "cast-cost",
    );
}

#[test]
fn scoped_linked_exile_validation_reaches_wrappers_and_nested_modes_once() {
    let consumer = r#"ReturnLinkedExiledCards(producer_ability_id: "activated_01", linked_exile_id: "linked_01", filter: (card_type: Creature))"#;
    let beheld = format!(
        r#"MayBehold(choice_id: "behold_01", hand_filter: (card_type: Creature), permanent_filter: (kind: Creature, controller: You), if_beheld: [{consumer}])"#
    );
    invalid(
        "(kind: AnyPermanent)",
        &child_with_effect(&beheld),
        "exactly one producer and one consumer",
    );
    let trigger = format!(
        r#"GrantTriggeredAbility(subject: Source, ability: (
        ability_id: "nested_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield,
        modal: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "mode_01", presentation: Fallback, effects: [{consumer}]),
            (mode_id: "mode_02", presentation: Fallback, effects: [GainLife(amount: 1)]),
        ])),
    ))"#
    );
    invalid(
        "(kind: AnyPermanent)",
        &child_with_effect(&trigger),
        "exactly one producer and one consumer",
    );
    let producer = r#"MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: "linked_01")"#;
    let consumer_ability = child_with_effect(&beheld).replacen(
        "(ability_id: \"activated_01\"",
        "(ability_id: \"activated_02\"",
        1,
    );
    let children = format!("{}, {}", child_with_effect(producer), consumer_ability);
    let ron = card("(kind: AnyPermanent)", &children);
    CardRegistry::from_chunks_and_tokens(&[&ron], &[]).expect("one exact producer-consumer pair");
}

#[test]
fn scoped_children_reject_additional_attachment_subjects() {
    for effect in [
        "Sacrifice(subject: AttachedObject)",
        "RemoveAllAbilities(subject: AttachedObject, duration: UntilEndOfTurn)",
    ] {
        invalid(
            "(kind: AnyPermanent)",
            &child_with_effect(effect),
            "recipient layout or attachment",
        );
    }
}

#[test]
fn scoped_children_validate_all_supported_payment_amount_fields() {
    let amount = "Count(CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller)))";
    for effect in [
        format!("TargetPlayerGainsLife(amount: {amount}, target: (kind: AnyPlayer))"),
        format!("PutCountersAllPlaneswalkers(counter: Loyalty, count: {amount})"),
    ] {
        invalid(
            "(kind: AnyPermanent)",
            &child_with_effect(&effect),
            "compatible card cost",
        );
    }
}

#[test]
fn scoped_children_cannot_read_face_cast_origin_through_inline_conditions() {
    for effect in [
        "Conditional(condition: CastOrigin(origin: Hand), effect: Draw(count: 1))",
        "Draw(count: Conditional(condition: CastOrigin(origin: Hand), when_true: 1, otherwise: 0))",
        "ChooseResolutionBranch(optional: true, branches: [(branch_id: \"branch_01\", presentation: Fallback, cost: None, requirement: GameCondition(CastOrigin(origin: Hand)), effects: [Draw(count: 1)])])",
        "GrantTriggeredAbility(subject: Source, ability: (ability_id: \"nested_01\", presentation: Fallback, trigger: WhenSelfEntersBattlefield, intervening_if: CastOrigin(origin: Hand), effect: [Draw(count: 1)]))",
        "CreateReflexiveTrigger(ability: (ability_id: \"nested_01\", presentation: Fallback, intervening_if: CastOrigin(origin: Hand), effect: [Draw(count: 1)]))",
    ] {
        invalid("(kind: AnyPermanent)", &child_with_effect(effect), "CastOrigin");
    }
}
