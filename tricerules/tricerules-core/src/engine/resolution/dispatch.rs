//! The one compiler-checked primitive dispatcher, shared by conditional instructions.
use super::*;

/// Execute one instruction in its existing target/result/continuation context.
pub(super) fn execute_effect(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    Ok(match effect {
        SpellEffectKind::WinGameIf { condition } => {
            if !cx.engine.state.is_terminal()
                && cx.engine.condition_holds(
                    &condition,
                    ConditionContext::for_stack_item(cx.top)
                        .with_previous_effect_result(cx.previous_effect_result),
                )
            {
                cx.engine.state.outcome = Some(crate::state::GameOutcome::Winner(cx.controller));
                EffectOutcome::GameEnded
            } else {
                EffectOutcome::Continue
            }
        }
        SpellEffectKind::ThassaOracle => zones::thassa_oracle(cx)?,
        effect @ SpellEffectKind::BoompileFlipCoinAndDestroyNonlands => {
            boompile::flip_coin_and_destroy_nonlands(cx, effect)?
        }
        SpellEffectKind::Conditional { condition, effect } => {
            if !cx.engine.condition_holds(
                &condition,
                ConditionContext::for_stack_item(cx.top)
                    .with_previous_effect_result(cx.previous_effect_result),
            ) {
                EffectOutcome::Continue
            } else {
                if !effect.supports_conditional_instruction() {
                    return Err(EngineError::Illegal("unsupported conditional inner effect"));
                }
                execute_effect(cx, *effect)?
            }
        }
        SpellEffectKind::ConditionalCastCost { condition, effect } => {
            if !cx.top.cast_cost_condition_matches(&condition) {
                EffectOutcome::Continue
            } else {
                if !effect.supports_cast_cost_conditional_instruction() {
                    return Err(EngineError::Illegal(
                        "unsupported cast-cost conditional inner effect",
                    ));
                }
                execute_effect(cx, *effect)?
            }
        }
        effect @ SpellEffectKind::DamageTarget { .. } => damage::damage_target(cx, effect)?,
        effect @ SpellEffectKind::PutAbilitySourceOntoBattlefieldTappedAndAttacking => {
            zones::put_ability_source_onto_battlefield_tapped_and_attacking(cx, effect)?
        }
        effect @ SpellEffectKind::PutAbilitySourceOntoBattlefield => {
            zones::put_ability_source_onto_battlefield(cx, effect)?
        }
        effect @ SpellEffectKind::PutPermanentFromHand { .. } => {
            zones::put_permanent_from_hand(cx, effect)?
        }
        effect @ SpellEffectKind::CreateStaticEmblem { .. } => {
            pump_counters::create_static_emblem(cx, effect)?
        }
        effect @ SpellEffectKind::CreateTriggeredEmblem { .. } => {
            pump_counters::create_triggered_emblem(cx, effect)?
        }
        effect @ SpellEffectKind::ExileIfWouldDieThisTurn { .. } => {
            zones::exile_if_would_die_this_turn(cx, effect)?
        }
        effect @ SpellEffectKind::CreatureDealsDamageEqualToPower { .. } => {
            damage::creature_deals_damage_equal_to_power(cx, effect)?
        }
        effect @ SpellEffectKind::Fight { .. } => damage::fight(cx, effect)?,
        effect @ SpellEffectKind::DamageTargets { .. } => damage::damage_targets(cx, effect)?,
        effect @ SpellEffectKind::DamagePlayer { .. } => damage::damage_player(cx, effect)?,
        effect @ SpellEffectKind::DamageAttackedPlayerOrPlaneswalker { .. } => {
            damage::damage_attacked_player_or_planeswalker(cx, effect)?
        }
        effect @ SpellEffectKind::Draw { .. } => zones::draw(cx, effect)?,
        SpellEffectKind::TakeExtraTurns { count } => {
            cx.engine.schedule_extra_turns(cx.controller, count);
            EffectOutcome::Continue
        }
        SpellEffectKind::JoinForces => join_forces::begin(cx)?,
        effect @ SpellEffectKind::TargetPlayerDraws { .. } => {
            zones::target_player_draws(cx, effect)?
        }
        SpellEffectKind::ShuffleResolvingSpellIntoOwnersLibrary => {
            zones::shuffle_resolving_spell_into_owners_library(cx)?
        }
        SpellEffectKind::ExileResolvingSpell => zones::exile_resolving_spell(cx)?,
        SpellEffectKind::MemoryJarExileHands => memory_jar::exile_hands(cx)?,
        SpellEffectKind::ReturnMemoryJarHands => memory_jar::return_hands(cx)?,
        SpellEffectKind::GrantChainVeilLoyaltyPermission => {
            misc::grant_chain_veil_loyalty_permission(cx)?
        }
        effect @ SpellEffectKind::Discard { .. } => zones::discard(cx, effect)?,
        effect @ SpellEffectKind::Scry { .. } => zones::scry(cx, effect)?,
        effect @ SpellEffectKind::LibraryPartition { .. } => zones::library_partition(cx, effect)?,
        effect @ SpellEffectKind::RevealTopCardToHandIfMatches { .. } => {
            zones::reveal_top_card_to_hand_if_matches(cx, effect)?
        }
        SpellEffectKind::RevealUntilArtifact => zones::reveal_until_artifact(cx)?,
        effect @ SpellEffectKind::Discover { .. } => zones::discover(cx, effect)?,
        effect @ SpellEffectKind::Cascade { .. } => zones::cascade(cx, effect)?,
        effect @ SpellEffectKind::Explore { .. } => zones::explore(cx, effect)?,
        SpellEffectKind::ManifestDread => zones::manifest_dread(cx)?,
        SpellEffectKind::LookChooseBattlefield {
            count,
            filter,
            max_selected,
            remainder,
        } => zones::look_choose_battlefield(cx, count, filter, max_selected, remainder)?,
        SpellEffectKind::ChaosWarp => chaos_warp::chaos_warp(cx)?,
        // No card admits this instruction until its simultaneous movement and
        // end-to-end pair publication have passed the remaining capability gates.
        SpellEffectKind::ExchangeArtifactWithGraveyard => {
            zones::exchange_artifact_with_graveyard(cx)?
        }
        effect @ SpellEffectKind::LookChooseToHand { .. } => {
            zones::look_choose_to_hand(cx, effect)?
        }
        effect @ SpellEffectKind::Hideaway { .. } => zones::hideaway(cx, effect)?,
        effect @ SpellEffectKind::PumpTarget { .. } => pump_counters::pump_target(cx, effect)?,
        effect @ SpellEffectKind::SetBasePowerToughness { .. } => {
            pump_counters::set_base_power_toughness(cx, effect)?
        }
        effect @ SpellEffectKind::SetSourceBasePower { .. } => {
            pump_counters::set_source_base_power(cx, effect)?
        }
        SpellEffectKind::SetClassLevel { level } => misc::set_class_level(cx, level)?,
        SpellEffectKind::SolveCase => misc::solve_case(cx)?,
        effect @ SpellEffectKind::PumpAll { .. } => pump_counters::pump_all(cx, effect)?,
        effect @ SpellEffectKind::DoublePowerToughnessAll { .. } => {
            pump_counters::double_power_toughness_all(cx, effect)?
        }
        effect @ SpellEffectKind::GrantKeywordsAll { .. } => {
            pump_counters::grant_keywords_all(cx, effect)?
        }
        effect @ SpellEffectKind::RemoveAbilitiesAll { .. } => {
            pump_counters::remove_abilities_all(cx, effect)?
        }
        effect @ SpellEffectKind::RemoveAllAbilities { .. } => {
            pump_counters::remove_all_abilities(cx, effect)?
        }
        effect @ SpellEffectKind::ApplyPermanentModifier { .. } => {
            pump_counters::apply_permanent_modifier(cx, effect)?
        }
        effect @ SpellEffectKind::GrantKeywords { .. } => {
            pump_counters::grant_keywords(cx, effect)?
        }
        effect @ SpellEffectKind::GrantProtection { .. } => {
            pump_counters::grant_protection(cx, effect)?
        }
        effect @ SpellEffectKind::GrantKeywordChoice { .. } => {
            pump_counters::grant_keyword_choice(cx, effect)?
        }
        effect @ SpellEffectKind::GrantTriggeredAbility { .. } => {
            pump_counters::grant_triggered_ability(cx, effect)?
        }
        effect @ SpellEffectKind::AddTypes { .. } => pump_counters::add_types(cx, effect)?,
        effect @ SpellEffectKind::GrantKeywordsAllPermanents { .. } => {
            pump_counters::grant_keywords_all_permanents(cx, effect)?
        }
        SpellEffectKind::ProtectAllPermanentsYouControlWithCounters => {
            pump_counters::protect_all_permanents_you_control_with_counters(cx)?
        }
        effect @ SpellEffectKind::ApplyCombatRestriction { .. } => {
            restrictions::apply_combat_restriction(cx, effect)?
        }
        SpellEffectKind::Blight { count } => blight::blight(cx, count)?,
        effect @ (SpellEffectKind::RemoveCounters { .. }
        | SpellEffectKind::RemoveAllCounters { .. }
        | SpellEffectKind::PutCounterSnapshot { .. }) => {
            pump_counters::change_counters(cx, effect)?
        }
        effect @ SpellEffectKind::PutCounters { .. } => pump_counters::put_counters(cx, effect)?,
        SpellEffectKind::MoveOneCounterBetweenTargets => {
            choices::move_one_counter_between_targets(cx)?
        }
        SpellEffectKind::DoubleYourCounters => pump_counters::double_your_counters(cx)?,
        effect @ SpellEffectKind::DoubleCounters { .. } => {
            pump_counters::double_counters(cx, effect)?
        }
        effect @ SpellEffectKind::PutCountersAll { .. } => {
            pump_counters::put_counters_all(cx, effect)?
        }
        effect @ SpellEffectKind::PutCountersAllPlaneswalkers { .. } => {
            pump_counters::put_counters_all_planeswalkers(cx, effect)?
        }
        SpellEffectKind::PutCountersOnCreaturesAndOtherPlaneswalkers => {
            pump_counters::put_counters_on_creatures_and_other_planeswalkers(cx)?
        }
        SpellEffectKind::Proliferate => proliferate::proliferate(cx)?,
        effect @ (SpellEffectKind::Destroy { .. }
        | SpellEffectKind::DestroyPreventingRegeneration { .. }) => misc::destroy(cx, effect)?,
        effect @ SpellEffectKind::Sacrifice { .. } => misc::sacrifice(cx, effect)?,
        effect @ SpellEffectKind::SacrificeAll { .. } => mass::sacrifice_all(cx, effect)?,
        effect @ SpellEffectKind::DestroyAttached { .. } => mass::destroy_attached(cx, effect)?,
        effect @ SpellEffectKind::CounterTargetSpell { .. } => {
            stack_ops::counter_target_spell(cx, effect)?
        }
        effect @ SpellEffectKind::CounterTargetAbility => {
            stack_ops::counter_target_ability(cx, effect)?
        }
        effect @ SpellEffectKind::CounterTriggeringStackObjectUnlessPays { .. } => {
            stack_ops::counter_triggering_stack_object_unless_pays(cx, effect)?
        }
        SpellEffectKind::CounterTriggeringSpell => stack_ops::counter_triggering_spell(cx)?,
        effect @ SpellEffectKind::CopyTargetSpell { .. } => {
            stack_ops::copy_target_spell(cx, effect)?
        }
        SpellEffectKind::CopyTriggeringActivatedAbility => {
            stack_ops::copy_triggering_activated_ability(cx)?
        }
        effect @ SpellEffectKind::CopyTargetArtifactWithManaValueX { .. } => {
            misc::copy_target_artifact_with_mana_value_x(cx, effect)?
        }
        effect @ SpellEffectKind::GainLife { .. } => life::gain_life(cx, effect)?,
        effect @ SpellEffectKind::LoseLife { .. } => life::lose_life(cx, effect)?,
        effect @ SpellEffectKind::TargetPlayerGainsLife { .. } => {
            life::target_player_gains_life(cx, effect)?
        }
        effect @ SpellEffectKind::TargetPlayerLosesLife { .. } => {
            life::target_player_loses_life(cx, effect)?
        }
        effect @ SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. } => {
            life::each_opponent_loses_life_you_gain_equal(cx, effect)?
        }
        effect @ SpellEffectKind::DrainTarget { .. } => life::drain_target(cx, effect)?,
        effect @ SpellEffectKind::Exile { .. } => zones::exile(cx, effect)?,
        effect @ SpellEffectKind::ExileWithOwnerCastPermission { .. } => {
            zones::exile_with_owner_cast_permission(cx, effect)?
        }
        effect @ SpellEffectKind::ExileTopWithPlayPermission { .. } => {
            zones::exile_top_with_play_permission(cx, effect)?
        }
        effect @ (SpellEffectKind::ReturnToOwnersHand { .. }
        | SpellEffectKind::ReturnAllToOwnersHand { .. }) => {
            zones::return_to_owners_hand(cx, effect)?
        }
        effect @ SpellEffectKind::PutInOwnersLibrary { .. } => {
            zones::put_in_owners_library(cx, effect)?
        }
        effect @ SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { .. } => {
            zones::shuffle_permanents_into_owners_libraries(cx, effect)?
        }
        effect @ SpellEffectKind::ShuffleNonlandPermanentsIntoOwnersLibraries { .. } => {
            zones::shuffle_nonland_permanents_into_owners_libraries(cx, effect)?
        }
        effect @ SpellEffectKind::ChooseHandCards { .. } => zones::choose_hand_cards(cx, effect)?,
        effect @ SpellEffectKind::MillTargetPlayer { .. } => zones::mill_target_player(cx, effect)?,
        effect @ SpellEffectKind::Mill { .. } => zones::mill(cx, effect)?,
        effect @ SpellEffectKind::TargetPlayerSacrifices { .. } => {
            zones::target_player_sacrifices(cx, effect)?
        }
        SpellEffectKind::TapOrUntap { .. } => {
            use tricerules_card_model::primitives::{ResolutionBranchDef, ResolutionCost};
            let branches = [("tap", "Tap"), ("untap", "Untap")]
                .into_iter()
                .map(|(id, label)| ResolutionBranchDef {
                    branch_id: tricerules_card_model::ChoiceId::new(id).expect("static branch id"),
                    presentation: tricerules_card_model::AbilityPresentation::Fallback,
                    runtime_fallback: Some(label.into()),
                    cost: ResolutionCost::None,
                    requirement: Default::default(),
                    effects: Vec::new(),
                })
                .collect();
            choices::park_resolution_branches(cx, true, branches)?
        }
        effect @ SpellEffectKind::Tap { .. } => misc::tap(cx, effect)?,
        effect @ SpellEffectKind::SkipNextUntap { .. } => misc::skip_next_untap(cx, effect)?,
        effect @ SpellEffectKind::Untap { .. } => misc::untap(cx, effect)?,
        effect @ SpellEffectKind::SetPrepared { .. } => misc::set_prepared(cx, effect)?,
        SpellEffectKind::CopyNextSpellThisTurn => {
            cx.engine
                .register_next_spell_copy(cx.top, cx.controller, cx.spell_label);
            EffectOutcome::Continue
        }
        effect @ SpellEffectKind::CopyCapturedSpell { .. } => {
            stack_ops::copy_target_spell(cx, effect)?
        }
        effect @ SpellEffectKind::GainControl { .. } => misc::gain_control(cx, effect)?,
        SpellEffectKind::GiveControlOfSourceToAttackingPlayer => {
            misc::give_control_of_source_to_attacking_player(cx)?
        }
        effect @ SpellEffectKind::CreateDelayedTrigger { .. } => {
            misc::create_delayed_trigger(cx, effect)?
        }
        effect @ SpellEffectKind::ExileUntilSourceLeaves { .. } => {
            zones::exile_until_source_leaves(cx, effect)?
        }
        effect @ SpellEffectKind::TapAll { .. } => mass::tap_all(cx, effect)?,
        effect @ SpellEffectKind::UntapAll { .. } => mass::untap_all(cx, effect)?,
        SpellEffectKind::UntapChosenPermanents => mass::untap_chosen_permanents(cx)?,
        SpellEffectKind::TapChosenPermanents => mass::tap_chosen_permanents(cx)?,
        effect @ SpellEffectKind::DestroyAll { .. } => mass::destroy_all(cx, effect)?,
        effect @ SpellEffectKind::ExileAll { .. } => mass::exile_all(cx, effect)?,
        effect @ SpellEffectKind::DamageAll { .. } => mass::damage_all(cx, effect)?,
        effect @ SpellEffectKind::CreateTokens { .. } => tokens::create_tokens(cx, effect)?,
        effect @ SpellEffectKind::CreateTokenBatch { .. } => {
            tokens::create_token_batch(cx, effect)?
        }
        effect @ SpellEffectKind::CreateTokenCopies { .. } => {
            tokens::create_token_copies(cx, effect)?
        }
        SpellEffectKind::Populate => tokens::populate(cx)?,
        effect @ SpellEffectKind::Amass { .. } => amass::amass(cx, effect)?,
        effect @ SpellEffectKind::CreateAttackingTokens { .. } => {
            tokens::create_attacking_tokens(cx, effect)?
        }
        effect @ SpellEffectKind::SacrificeObservedObjects => {
            tokens::sacrifice_observed_objects(cx, effect)?
        }
        effect @ SpellEffectKind::ExileObservedObjects => {
            tokens::exile_observed_objects(cx, effect)?
        }
        SpellEffectKind::ExileWarpedObject => {
            cx.engine.resolve_warp_exile(cx.top, cx.events)?;
            EffectOutcome::Continue
        }
        effect @ SpellEffectKind::AttachSource { .. } => misc::attach_source(cx, effect)?,
        effect @ SpellEffectKind::AttachEquipment { .. } => misc::attach_equipment(cx, effect)?,
        effect @ SpellEffectKind::Equip { .. } => misc::equip(cx, effect)?,
        effect @ SpellEffectKind::PreventNextDamage { .. } => {
            misc::prevent_next_damage(cx, effect)?
        }
        effect @ SpellEffectKind::PreventAllCombatDamageToTargetTurn { .. } => {
            misc::prevent_all_combat_damage_to_target_turn(cx, effect)?
        }
        effect @ SpellEffectKind::PreventAllCombatDamageByTargetTurn { .. } => {
            misc::prevent_all_combat_damage_by_target_turn(cx, effect)?
        }
        effect @ SpellEffectKind::PreventAllCombatDamageTurn => {
            misc::prevent_all_combat_damage_turn(cx, effect)?
        }
        effect @ SpellEffectKind::DamageCantBePreventedThisTurn => {
            misc::damage_cant_be_prevented_this_turn(cx, effect)?
        }
        effect @ SpellEffectKind::MoveGraveyardCards { .. } => {
            zones::move_graveyard_cards(cx, effect)?
        }
        effect @ SpellEffectKind::ReturnLinkedExiledCards { .. } => {
            zones::return_linked_exiled_cards(cx, effect)?
        }
        effect @ SpellEffectKind::ReturnOtherLinkedExiledCards { .. } => {
            zones::return_other_linked_exiled_cards(cx, effect)?
        }
        SpellEffectKind::ExileGraveyards {
            players,
            filter,
            linked_exile_id,
            capture_exile_cohort,
        } => zones::exile_graveyards(
            cx,
            players,
            filter.as_ref(),
            linked_exile_id,
            capture_exile_cohort,
        )?,
        SpellEffectKind::ReturnExiledCohortToOwnersBattlefield { cohort_id } => {
            zones::return_exiled_cohort_to_owners_battlefield(cx, &cohort_id)?
        }
        SpellEffectKind::ReturnAllGraveyardPermanents { players, filter } => {
            zones::return_all_graveyard_permanents(cx, players, &filter)?
        }
        SpellEffectKind::ReturnDifferentNamedPermanentsFromGraveyard => {
            zones::return_different_named_permanents_from_graveyard(cx)?
        }
        SpellEffectKind::ReturnAllGraveyardPermanentsWithManaValueXOrLess { filter } => {
            zones::return_all_graveyard_permanents_with_mana_value_x_or_less(cx, &filter)?
        }
        effect @ SpellEffectKind::ChooseCard { .. } => zones::choose_card(cx, effect)?,
        effect @ SpellEffectKind::ChooseLinkedExiledCard { .. } => {
            zones::choose_linked_exiled_card(cx, effect)?
        }
        effect @ SpellEffectKind::PlayLinkedExiledCard { .. } => {
            zones::play_linked_exiled_card(cx, effect)?
        }
        SpellEffectKind::Earthbend { count } => pump_counters::earthbend(cx, count)?,
        effect @ SpellEffectKind::AnimateSelf { .. } => pump_counters::animate_self(cx, effect)?,
        effect @ SpellEffectKind::ReturnTriggeredCard { .. } => {
            zones::return_triggered_card(cx, effect)?
        }
        effect @ SpellEffectKind::ExileSourceThenReturnTransformed { .. } => {
            zones::exile_source_then_return_transformed(cx, effect)?
        }
        effect @ (SpellEffectKind::ProduceMana { .. }
        | SpellEffectKind::ProduceManaFromOpponentLands { .. }
        | SpellEffectKind::ProduceManaPerSourceCounter { .. }
        | SpellEffectKind::ProduceSplitManaFromRemovedStorageCounters { .. }) => {
            misc::produce_mana(cx, effect)?
        }
        effect @ SpellEffectKind::AddMana { .. } => misc::add_mana(cx, effect)?,
        effect @ SpellEffectKind::MayBehold { .. } => choices::may_behold(cx, effect)?,
        effect @ SpellEffectKind::SearchLibrary { .. } => zones::search_library(cx, effect)?,
        SpellEffectKind::RevealCohort => library_cohorts::reveal_cohort(cx)?,
        effect @ SpellEffectKind::ChooseCohortPartition { .. } => {
            library_cohorts::choose_cohort_partition(cx, effect)?
        }
        effect @ SpellEffectKind::MoveCohortPartition { .. } => {
            library_cohorts::move_cohort_partition(cx, effect)?
        }
        effect @ SpellEffectKind::ShuffleLibrary { .. } => {
            library_cohorts::shuffle_library(cx, effect)?
        }
        effect @ SpellEffectKind::Regenerate { .. } => misc::regenerate(cx, effect)?,
        effect @ SpellEffectKind::ChangeSourceFace { .. } => misc::change_source_face(cx, effect)?,
        SpellEffectKind::CastMadness { cost } => zones::madness_cast(cx, cost)?,
        SpellEffectKind::SiegeDefeat => zones::siege_defeat(cx)?,
        effect @ SpellEffectKind::None => misc::none(cx, effect)?,
        effect @ SpellEffectKind::AuraAttach { .. } => misc::aura_attach(cx, effect)?,
        effect @ SpellEffectKind::ChoosePermanents { .. } => {
            choices::choose_permanents(cx, effect)?
        }
        SpellEffectKind::SelectHandCards { .. }
        | SpellEffectKind::PutSelectedHandCardsOnLibraryTop => {
            return Err(EngineError::Illegal(
                "program-only hand operation dispatched outside a resolution program",
            ));
        }
        effect @ SpellEffectKind::ChooseAndSacrifice { .. } => {
            mass::choose_and_sacrifice(cx, effect)?
        }
        effect @ SpellEffectKind::ChoosePermanentsByTypeAndSacrificeRest { .. } => {
            mass::choose_permanents_by_type_and_sacrifice_rest(cx, effect)?
        }
        effect @ SpellEffectKind::CreateReflexiveTrigger { .. } => {
            choices::create_reflexive_trigger(cx, effect)?
        }
    })
}

#[cfg(test)]
mod extra_turn_effect_tests {
    use super::*;
    use crate::state::{EffectResult, StackItem, TriggerContext};
    use tricerules_card_model::primitives::SpellEffectKind;

    #[test]
    fn copied_ability_uses_its_captured_controller_for_each_turn() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            50_029,
            &[10, 20, 30, 40],
            20,
            None,
            true,
        )
        .expect("four-player engine");
        let definition = tricerules_cards::registry::global()
            .get("magistrates_scepter")
            .expect("actual Scepter definition");
        let copied_ability = StackItem {
            mana_colors_spent_to_cast: Default::default(),
            id: 500,
            controller: 30,
            card_id: "magistrates_scepter".into(),
            prototyped: false,
            targets: Vec::new(),
            ability_text: Some("Take an extra turn".into()),
            source_permanent_id: Some(900),
            source_owner: Some(10),
            source_zone_change: 1,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: Some(1),
            activated_ability: Some(definition.primary_face().activated_abilities[1].clone()),
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: true,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            payment_result: Default::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        };
        let captured_controller = copied_ability.controller;
        let previous_effect_result = EffectResult::default();
        let mut effect_result = EffectResult::default();
        let mut events = Vec::new();
        let mut cx = EffectCx {
            engine: &mut engine,
            events: &mut events,
            targets: &[],
            targets_by_role: &[],
            target_receipts: &[],
            target_role_group_indices: &[],
            top: &copied_ability,
            controller: captured_controller,
            affected_player: captured_controller,
            spell_label: "copied Scepter ability",
            previous_effect_result: &previous_effect_result,
            resume_previous_result: None,
            effect_result: &mut effect_result,
            effect_index: 0,
            program_state: None,
        };

        assert_eq!(
            execute_effect(&mut cx, SpellEffectKind::TakeExtraTurns { count: 2 })
                .expect("extra-turn effect resolves"),
            EffectOutcome::Continue
        );
        drop(cx);

        assert_eq!(engine.state.extra_turn_queue, [30, 30]);
        assert_ne!(engine.state.extra_turn_queue, [10, 10]);
        assert_eq!(engine.state.normal_turn_resume_player, None);
    }
}
