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
        SpellEffectKind::MillEachOpponentByHandSize => zones::mill_each_opponent_by_hand_size(cx)?,
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
        effect @ SpellEffectKind::PutLandFromHandOntoBattlefield => {
            zones::put_land_from_hand_onto_battlefield(cx, effect)?
        }
        effect @ SpellEffectKind::CreateStaticEmblem { .. } => {
            pump_counters::create_static_emblem(cx, effect)?
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
        SpellEffectKind::MyrBattlesphereAttack => damage::myr_battlesphere_attack(cx)?,
        effect @ SpellEffectKind::Draw { .. } => zones::draw(cx, effect)?,
        effect @ SpellEffectKind::TargetPlayerDraws { .. } => {
            zones::target_player_draws(cx, effect)?
        }
        SpellEffectKind::ShuffleResolvingSpellIntoOwnersLibrary => {
            zones::shuffle_resolving_spell_into_owners_library(cx)?
        }
        SpellEffectKind::ExileResolvingSpell => zones::exile_resolving_spell(cx)?,
        effect @ SpellEffectKind::Discard { .. } => zones::discard(cx, effect)?,
        effect @ SpellEffectKind::DrawDiscard { .. } => zones::draw_discard(cx, effect)?,
        effect @ SpellEffectKind::Scry { .. } => zones::scry(cx, effect)?,
        effect @ SpellEffectKind::LibraryPartition { .. } => zones::library_partition(cx, effect)?,
        effect @ SpellEffectKind::RevealTopCardToHandIfMatches { .. } => {
            zones::reveal_top_card_to_hand_if_matches(cx, effect)?
        }
        effect @ SpellEffectKind::Explore { .. } => zones::explore(cx, effect)?,
        SpellEffectKind::ManifestDread => zones::manifest_dread(cx)?,
        SpellEffectKind::IntoTheWilds => zones::into_the_wilds(cx)?,
        SpellEffectKind::DeployTheGatewatch => zones::deploy_the_gatewatch(cx)?,
        SpellEffectKind::ChaosWarp => chaos_warp::chaos_warp(cx)?,
        // No card admits this instruction until its simultaneous movement and
        // end-to-end pair publication have passed the remaining capability gates.
        SpellEffectKind::ExchangeArtifactWithGraveyard => {
            zones::exchange_artifact_with_graveyard(cx)?
        }
        effect @ SpellEffectKind::LookChooseToHand { .. } => {
            zones::look_choose_to_hand(cx, effect)?
        }
        effect @ SpellEffectKind::PumpTarget { .. } => pump_counters::pump_target(cx, effect)?,
        effect @ SpellEffectKind::SetBasePowerToughness { .. } => {
            pump_counters::set_base_power_toughness(cx, effect)?
        }
        effect @ SpellEffectKind::SetSourceBasePowerToTownCount => {
            pump_counters::set_source_base_power_to_town_count(cx, effect)?
        }
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
        effect @ SpellEffectKind::DoubleCounters { .. } => {
            pump_counters::double_counters(cx, effect)?
        }
        effect @ SpellEffectKind::PutCountersAll { .. } => {
            pump_counters::put_counters_all(cx, effect)?
        }
        effect @ SpellEffectKind::PutCountersAllPlaneswalkers { .. } => {
            pump_counters::put_counters_all_planeswalkers(cx, effect)?
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
        effect @ SpellEffectKind::CopyTargetSpell { .. } => {
            stack_ops::copy_target_spell(cx, effect)?
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
        effect @ SpellEffectKind::ExileTargetGainLifeEqualToPower => {
            zones::exile_target_gain_life_equal_to_power(cx, effect)?
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
        effect @ SpellEffectKind::ChooseHandCards { .. } => zones::choose_hand_cards(cx, effect)?,
        effect @ SpellEffectKind::MillTargetPlayer { .. } => zones::mill_target_player(cx, effect)?,
        effect @ SpellEffectKind::Mill { .. } => zones::mill(cx, effect)?,
        effect @ SpellEffectKind::TargetPlayerSacrifices { .. } => {
            zones::target_player_sacrifices(cx, effect)?
        }
        SpellEffectKind::TapOrUntap { .. } => {
            use tricerules_cards::primitives::{ResolutionBranchDef, ResolutionCost};
            let branches = [("tap", "Tap"), ("untap", "Untap")]
                .into_iter()
                .map(|(id, label)| ResolutionBranchDef {
                    branch_id: tricerules_cards::ChoiceId::new(id).expect("static branch id"),
                    presentation: tricerules_cards::AbilityPresentation::Fallback,
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
        effect @ SpellEffectKind::CreateDelayedTrigger { .. } => {
            misc::create_delayed_trigger(cx, effect)?
        }
        effect @ SpellEffectKind::ExileUntilSourceLeaves { .. } => {
            zones::exile_until_source_leaves(cx, effect)?
        }
        effect @ SpellEffectKind::TapAll { .. } => mass::tap_all(cx, effect)?,
        effect @ SpellEffectKind::UntapAll { .. } => mass::untap_all(cx, effect)?,
        SpellEffectKind::UntapChosenPermanents => mass::untap_chosen_permanents(cx)?,
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
        SpellEffectKind::ExileGraveyards {
            players,
            filter,
            capture_exile_cohort,
        } => zones::exile_graveyards(cx, players, filter.as_ref(), capture_exile_cohort)?,
        SpellEffectKind::ReturnExiledCohortToOwnersBattlefield { cohort_id } => {
            zones::return_exiled_cohort_to_owners_battlefield(cx, &cohort_id)?
        }
        SpellEffectKind::ReturnAllGraveyardPermanents { filter } => {
            zones::return_all_graveyard_permanents(cx, &filter)?
        }
        SpellEffectKind::ReturnAllGraveyardPermanentsWithManaValueXOrLess { filter } => {
            zones::return_all_graveyard_permanents_with_mana_value_x_or_less(cx, &filter)?
        }
        effect @ SpellEffectKind::ChooseGraveyardCard { .. } => {
            zones::choose_graveyard_card(cx, effect)?
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
        effect @ SpellEffectKind::Regenerate { .. } => misc::regenerate(cx, effect)?,
        effect @ SpellEffectKind::ChangeSourceFace { .. } => misc::change_source_face(cx, effect)?,
        SpellEffectKind::CastMadness { cost } => zones::madness_cast(cx, cost)?,
        SpellEffectKind::SiegeDefeat => zones::siege_defeat(cx)?,
        effect @ SpellEffectKind::None => misc::none(cx, effect)?,
        effect @ SpellEffectKind::AuraAttach { .. } => misc::aura_attach(cx, effect)?,
        effect @ SpellEffectKind::ChooseResolutionBranch { .. } => {
            choices::choose_resolution_branch(cx, effect)?
        }
        effect @ SpellEffectKind::ChoosePermanents { .. } => {
            choices::choose_permanents(cx, effect)?
        }
        effect @ SpellEffectKind::CreateReflexiveTrigger { .. } => {
            choices::create_reflexive_trigger(cx, effect)?
        }
    })
}
