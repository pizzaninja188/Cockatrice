#include "ruled_event_dispatcher.h"

#include "ruled_client_host.h"
#include "ruled_client_state.h"

#include <QDebug>
#include <algorithm>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/utility/ruled_debug.h>

namespace
{

using RuledCombatPhase = RuledClientState::RuledCombatPhase;
using RuledOpeningUiKind = RuledClientState::RuledOpeningUiKind;
using PickZone = RuledClientState::PickZone;

RuledCombatPhase mapRuledPhaseToCombatPhase(ruled::v1::PhaseId phase)
{
    switch (phase) {
        case ruled::v1::PHASE_ID_DECLARE_ATTACKERS:
            return RuledCombatPhase::DeclareAttackers;
        case ruled::v1::PHASE_ID_DECLARE_BLOCKERS:
            return RuledCombatPhase::DeclareBlockers;
        case ruled::v1::PHASE_ID_ASSIGN_COMBAT_DAMAGE:
            return RuledCombatPhase::AssignCombatDamage;
        case ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE:
            return RuledCombatPhase::FirstStrikeDamage;
        case ruled::v1::PHASE_ID_COMBAT_DAMAGE:
            return RuledCombatPhase::CombatDamage;
        default:
            // Includes end of combat: combat is over, so no combat UI.
            return RuledCombatPhase::None;
    }
}

int mapRuledPhaseToToolbarPhase(ruled::v1::PhaseId phase)
{
    switch (phase) {
        case ruled::v1::PHASE_ID_UNTAP:
            return 0;
        case ruled::v1::PHASE_ID_UPKEEP:
            return 1;
        case ruled::v1::PHASE_ID_DRAW:
            return 2;
        case ruled::v1::PHASE_ID_MAIN1:
            return 3;
        case ruled::v1::PHASE_ID_BEGIN_COMBAT:
            return 4;
        case ruled::v1::PHASE_ID_DECLARE_ATTACKERS:
            return 5;
        case ruled::v1::PHASE_ID_DECLARE_BLOCKERS:
        case ruled::v1::PHASE_ID_ASSIGN_COMBAT_DAMAGE:
            return 6;
        case ruled::v1::PHASE_ID_COMBAT_DAMAGE:
            return 7;
        case ruled::v1::PHASE_ID_END_COMBAT:
            return 8;
        case ruled::v1::PHASE_ID_MAIN2:
            return 9;
        case ruled::v1::PHASE_ID_END_STEP:
        case ruled::v1::PHASE_ID_CLEANUP:
            return 10;
        default:
            // CR 510.4: the first-strike substep deliberately leaves the toolbar highlight where
            // it is (see inFirstStrikeDamageStep). Opening pseudo-phases have no slot either.
            return -1;
    }
}

bool isCombatPhase(RuledCombatPhase phase)
{
    return phase == RuledCombatPhase::DeclareAttackers || phase == RuledCombatPhase::DeclareBlockers ||
           phase == RuledCombatPhase::AssignCombatDamage || phase == RuledCombatPhase::FirstStrikeDamage ||
           phase == RuledCombatPhase::CombatDamage;
}

RuledClientState::SpellTargetData parseSpellTargets(const ruled::v1::SpellTargets &src)
{
    RuledClientState::SpellTargetData data;
    for (const auto &group : src.groups()) {
        RuledTargetGroupData parsed;
        parsed.groupIndex = static_cast<int>(group.group_index());
        for (const quint32 oid : group.valid_permanent_ids()) {
            parsed.validPermanentIds.insert(oid);
        }
        for (const quint32 oid : group.valid_stack_ids()) {
            parsed.validStackIds.insert(oid);
        }
        for (const quint32 oid : group.valid_graveyard_ids()) {
            parsed.validGraveyardIds.insert(oid);
        }
        parsed.canTargetSelf = group.can_target_self();
        parsed.canTargetOpponent = group.can_target_opponent();
        parsed.minTargets = static_cast<int>(group.min());
        parsed.maxTargets = static_cast<int>(group.max());
        parsed.promptText = QString::fromStdString(group.prompt_text());
        parsed.sameGraveyard = group.same_graveyard();
        for (const quint32 other : group.distinct_from_group_indices()) {
            parsed.distinctFromGroupIndices.append(static_cast<int>(other));
        }
        data.groups.append(parsed);
    }
    // Existing rendering/query helpers expose the active first group through the flat fields.
    // Pending collectors index `groups` directly as they advance.
    if (!data.groups.isEmpty()) {
        static_cast<RuledTargetGroupData &>(data) = data.groups.first();
    }
    data.fixedDamage = static_cast<int>(src.fixed_damage());
    data.isDamageTargets = src.is_damage_targets();
    data.extraManaPerTarget = static_cast<int>(src.extra_mana_per_target());
    data.damageDividedEvenly = src.damage_division() == ruled::v1::DAMAGE_DIVISION_EVEN_AT_RESOLUTION;
    for (const auto &application : src.targeting_cost_applications()) {
        RuledTargetingCostApplication parsed;
        parsed.applicationId = static_cast<quint64>(application.application_id());
        parsed.genericMana = static_cast<int>(application.generic_mana());
        for (const auto &candidate : application.affected_targets()) {
            parsed.affectedTargets.append({candidate.kind(), static_cast<quint32>(candidate.object_id())});
        }
        data.targetingCostApplications.append(parsed);
    }
    for (const auto &application : src.targeted_cost_reduction_applications()) {
        RuledTargetedCostReductionApplication parsed;
        parsed.applicationId = static_cast<quint64>(application.application_id());
        parsed.genericMana = static_cast<int>(application.generic_mana());
        for (const auto &candidate : application.qualifying_targets()) {
            parsed.qualifyingTargets.append({candidate.kind(), static_cast<quint32>(candidate.object_id())});
        }
        data.targetedCostReductionApplications.append(parsed);
    }
    return data;
}

RuledCostData parseCostData(const ruled::v1::LegalCostChoices &src)
{
    RuledCostData data;
    data.nonManaCostsPayable = src.non_mana_costs_payable();
    for (const auto &choice : src.choices()) {
        RuledCostChoice parsed;
        parsed.costIndex = static_cast<int>(choice.cost_index());
        switch (choice.zone()) {
            case ruled::v1::COST_CHOICE_ZONE_HAND:
                parsed.zone = RuledCostChoiceZone::Hand;
                break;
            case ruled::v1::COST_CHOICE_ZONE_GRAVEYARD:
                parsed.zone = RuledCostChoiceZone::Graveyard;
                break;
            default:
                parsed.zone = RuledCostChoiceZone::Battlefield;
                break;
        }
        parsed.min = static_cast<int>(choice.min());
        parsed.max = static_cast<int>(choice.max());
        parsed.blightCount = choice.blight_count();
        switch (choice.kind()) {
            case ruled::v1::COST_CHOICE_KIND_DISCARD:
                parsed.kind = RuledCostChoiceKind::Discard;
                break;
            case ruled::v1::COST_CHOICE_KIND_SACRIFICE:
                parsed.kind = RuledCostChoiceKind::Sacrifice;
                break;
            case ruled::v1::COST_CHOICE_KIND_EXILE:
                parsed.kind = RuledCostChoiceKind::Exile;
                break;
            case ruled::v1::COST_CHOICE_KIND_TAP:
                parsed.kind = RuledCostChoiceKind::Tap;
                break;
            case ruled::v1::COST_CHOICE_KIND_BLIGHT:
                parsed.kind = RuledCostChoiceKind::Blight;
                break;
            default:
                parsed.kind = RuledCostChoiceKind::Unspecified;
                break;
        }
        for (const quint32 candidate : choice.candidate_ids()) {
            parsed.candidateIds.insert(candidate);
        }
        for (const auto &candidate : choice.candidate_objects()) {
            parsed.candidateGenerations.insert(candidate.object_id(), candidate.zone_change_generation());
        }
        data.choices.append(parsed);
    }
    for (const auto &group : src.cast_cost_groups()) {
        RuledCastCostGroup parsedGroup;
        parsedGroup.groupIndex = static_cast<int>(group.group_index());
        parsedGroup.prompt = QString::fromStdString(group.prompt());
        parsedGroup.min = static_cast<int>(group.min());
        parsedGroup.max = static_cast<int>(group.max());
        parsedGroup.skipLabel = QString::fromStdString(group.skip_label());
        for (const auto &option : group.options()) {
            RuledCastCostOption parsedOption;
            parsedOption.optionIndex = static_cast<int>(option.option_index());
            parsedOption.label = QString::fromStdString(option.label());
            switch (option.kind()) {
                case ruled::v1::CAST_COST_OPTION_KIND_BEHOLD:
                    parsedOption.kind = RuledCastCostOptionKind::Behold;
                    break;
                case ruled::v1::CAST_COST_OPTION_KIND_TAP_PERMANENT_FOR_GENERIC_REDUCTION:
                    parsedOption.kind = RuledCastCostOptionKind::TapPermanentForGenericReduction;
                    break;
                case ruled::v1::CAST_COST_OPTION_KIND_BLIGHT:
                    parsedOption.kind = RuledCastCostOptionKind::Blight;
                    break;
                case ruled::v1::CAST_COST_OPTION_KIND_MANA:
                    parsedOption.kind = RuledCastCostOptionKind::Mana;
                    break;
                default:
                    parsedOption.kind = RuledCastCostOptionKind::Unspecified;
                    break;
            }
            parsedOption.additionalManaCost = QString::fromStdString(option.additional_mana_cost());
            parsedOption.selectable = option.selectable() && parsedOption.kind != RuledCastCostOptionKind::Unspecified;
            for (const quint32 candidate : option.valid_hand_indices()) {
                parsedOption.validHandIndices.insert(candidate);
            }
            for (const quint32 candidate : option.valid_permanent_ids()) {
                parsedOption.validPermanentIds.insert(candidate);
            }
            const int generationCount = std::min(option.valid_permanent_ids_size(),
                                                 option.valid_permanent_generations_size());
            for (int i = 0; i < generationCount; ++i) {
                parsedOption.validPermanentGenerations.insert(option.valid_permanent_ids(i),
                                                               option.valid_permanent_generations(i));
            }
            const int reductionCount =
                std::min(option.valid_permanent_ids_size(), option.valid_permanent_generic_reductions_size());
            for (int i = 0; i < reductionCount; ++i) {
                parsedOption.validPermanentGenericReductions.insert(
                    option.valid_permanent_ids(i), static_cast<int>(option.valid_permanent_generic_reductions(i)));
            }
            parsedGroup.options.append(parsedOption);
        }
        data.castCostGroups.append(parsedGroup);
    }
    return data;
}

/// Copies the engine's structured hand-action contract into the generic client-side indexes.
QHash<RuledHandActionKind, RuledHandActionSet> copyHandActions(const ruled::v1::LegalActions &actions)
{
    QHash<RuledHandActionKind, RuledHandActionSet> parsed;
    for (const auto &action : actions.hand_actions()) {
        const int handIndex = static_cast<int>(action.hand_index());
        const int faceIndex = static_cast<int>(action.face_index());
        const int castKey = RuledClientState::spellTargetKey(handIndex, faceIndex);
        RuledHandActionSet &set = parsed[action.kind()];
        set.handIndices.insert(handIndex);
        const QString cardName = QString::fromStdString(action.card_name());
        set.indicesByCardName.insert(cardName, handIndex);
        set.faceOptionsByIndex[handIndex].append({faceIndex, cardName, QString::fromStdString(action.cost()),
                                               static_cast<int>(action.generic_cost_reduction()),
                                               ruled::v1::CAST_METHOD_NORMAL, action.has_convoke()});
        if (action.has_cost_choices()) {
            set.costDataByCastKey.insert(castKey, parseCostData(action.cost_choices()));
        }
        QSet<quint32> eligibleGroups;
        for (const quint32 groupId : action.eligible_restricted_mana_group_ids()) {
            eligibleGroups.insert(groupId);
        }
        set.eligibleRestrictedManaByCastKey.insert(castKey, eligibleGroups);
        if (action.needs_target()) {
            set.needsTargetCastKeys.insert(castKey);
        }
        if (action.modes_size() > 0) {
            set.modalMinModesByCastKey.insert(castKey, static_cast<int>(action.min_modes()));
            set.modalMaxModesByCastKey.insert(castKey, static_cast<int>(action.max_modes()));
            QVector<RuledModalSpellOption> modes;
            modes.reserve(action.modes_size());
            for (const auto &mode : action.modes()) {
                modes.append(
                    {static_cast<int>(mode.mode_index()), QString::fromStdString(mode.label()), mode.selectable(),
                     mode.needs_target(),
                     mode.has_targets() ? parseSpellTargets(mode.targets()) : RuledClientState::SpellTargetData{}});
            }
            set.modalOptionsByCastKey.insert(castKey, modes);
        }
    }
    return parsed;
}

} // namespace

RuledEventDispatcher::RuledEventDispatcher(RuledClientState *_state, RuledClientHost *_host, QObject *parent)
    : QObject(parent), state(_state), host(_host)
{
}

bool RuledEventDispatcher::processPayload(const std::string &payload)
{
    ruled::v1::RuledEventBatch batch;
    if (!batch.ParseFromString(payload)) {
        return false;
    }
    if (batch.has_spell_payment_preview()) {
        if (batch.events_size() != 0 || !batch.legal_by_player().empty())
            return false;
        if (state->spellPayment.apply(batch.spell_payment_preview()))
            emit state->spellPaymentPreviewReceived();
        return true;
    }
    resetPerBatchLegalActions();
    processBatch(batch);
    return true;
}

void RuledEventDispatcher::resetPerBatchLegalActions()
{
    for (const quint32 oid : state->zoneAbilitySourceByOid.keys()) {
        state->engineOidToActivatedAbilityTexts.remove(oid);
        state->engineOidToActivatedAbilityManaCosts.remove(oid);
        state->engineOidToActivatedAbilityManaProduced.remove(oid);
        state->engineOidToActivatedAbilityCostLabels.remove(oid);
        state->engineOidToActivatedAbilityActivatable.remove(oid);
    }
    state->handAbilityOidBySlot.clear();
    state->zoneAbilitySourceByOid.clear();
    state->abilitySourceGenerationByOid.clear();
    state->zoneAbilityIndicesByOid.clear();
    state->clearHandActions();
    state->zoneCastActions = {};
    state->zoneCastSourceByOid.clear();
    state->zoneCastCostsByCastKey.clear();
    state->zoneLandFacesByOid.clear();
    state->validTargetsByHandSlot.clear();
    state->validTargetsByZoneObject.clear();
    state->validTargetsByAbility.clear();
    state->abilityCostData.clear();
    state->eligibleRestrictedManaByAbility.clear();
    state->permanentActionsByOid.clear();
    state->openingBottomSelectedIndices.clear();
    state->openingPickSeatIds.clear();
    state->openingUiKind = RuledOpeningUiKind::None;
    state->resolutionChoiceWaitingPlayerId = -1;
}

void RuledEventDispatcher::processBatch(const ruled::v1::RuledEventBatch &batch)
{
    BatchContext ctx;

    for (const auto &event : batch.events()) {
        if (event.has_attackers_preview() || event.has_blockers_preview()) {
            // Servatrice synthesizes preview payloads locally and appends ordinary identity maps.
            // They are not an authoritative engine snapshot and must not retire a parked reveal.
            ctx.reconcilePublicReveal = false;
            break;
        }
    }

    for (const auto &e : batch.events()) {
        if (e.has_log()) {
            const QString logLine = QString::fromStdString(e.log().text()).trimmed();
            if (!logLine.isEmpty()) {
                ctx.timeline += logLine + QLatin1Char('\n');
            }
        }
        if (e.has_phase_changed()) {
            applyPhaseChanged(e.phase_changed(), ctx);
        }
        if (e.has_priority_changed()) {
            host->setPriorityPlayerId(static_cast<int>(e.priority_changed().player_id()));
            ctx.promptFeed += QStringLiteral("Priority: P%1\n").arg(e.priority_changed().player_id());
        }
        if (e.has_stack_pushed()) {
            applyStackPushed(e.stack_pushed(), ctx);
        }
        if (e.has_stack_resolved()) {
            applyStackResolved(e.stack_resolved(), ctx);
        }
        if (e.has_stack_object_countered()) {
            applyStackObjectCountered(e.stack_object_countered(), ctx);
        }
        if (e.has_trigger_needs_target()) {
            applyTriggerNeedsTarget(e.trigger_needs_target(), ctx);
        }
        if (e.has_trigger_order_required()) {
            applyTriggerOrderRequired(e.trigger_order_required(), ctx);
        }
        if (e.has_resolution_choice_required()) {
            applyResolutionChoiceRequired(e.resolution_choice_required(), ctx);
        }
        if (e.has_active_public_reveal_snapshot()) {
            applyActivePublicRevealSnapshot(e.active_public_reveal_snapshot());
        }
        if (e.has_battlefield_object_map()) {
            applyBattlefieldObjectMap(e.battlefield_object_map(), ctx);
        }
        if (e.has_face_down_object_map()) {
            applyFaceDownObjectMap(e.face_down_object_map(), ctx);
        }
        if (e.has_hand_slot_map()) {
            applyHandSlotMap(e.hand_slot_map());
        }
        if (e.has_graveyard_object_map()) {
            applyGraveyardObjectMap(e.graveyard_object_map());
        }
        if (e.has_exile_object_map()) {
            applyExileObjectMap(e.exile_object_map());
        }
        if (e.has_zone_view()) {
            applyZoneView(e.zone_view(), ctx);
        }
        if (e.has_attackers_declared()) {
            applyAttackersDeclared(e.attackers_declared(), ctx);
        }
        if (e.has_attackers_added()) {
            applyAttackersAdded(e.attackers_added(), ctx);
        }
        if (e.has_attackers_preview()) {
            applyAttackersPreview(e.attackers_preview(), ctx);
        }
        if (e.has_blockers_declared()) {
            applyBlockersDeclared(e.blockers_declared(), ctx);
        }
        if (e.has_combat_damage_assigned()) {
            applyCombatDamageAssigned(e.combat_damage_assigned(), ctx);
        }
        if (e.has_blockers_preview()) {
            applyBlockersPreview(e.blockers_preview(), ctx);
        }
        if (e.has_removed_from_combat()) {
            applyRemovedFromCombat(e.removed_from_combat(), ctx);
        }
        if (e.has_life_changed()) {
            applyLifeChanged(e.life_changed(), ctx);
        }
        if (e.has_mana_pool_updated()) {
            applyManaPoolUpdated(e.mana_pool_updated(), ctx);
        }
    }

    const auto lit = batch.legal_by_player().find(host->localPlayerId());
    if (lit != batch.legal_by_player().end()) {
        applyLegalActions(lit->second, ctx);
    } else {
        applyNoLegalActions();
    }
    if (state->hasPendingTriggerTarget() && state->pendingChoice.has_value() &&
        !state->pendingChoice->triggerTargets.groups.isEmpty()) {
        state->validTargetsByAbility.insert(
            RuledClientState::abilityTargetKey(state->lastTriggerSourceOid,
                                               static_cast<int>(state->lastTriggerAbilityIndex)),
            state->pendingChoice->triggerTargets);
    }

    finishBatch(ctx);
}

// ---------------------------------------------------------------------------------------
// Per-event-kind appliers
// ---------------------------------------------------------------------------------------

void RuledEventDispatcher::applyPhaseChanged(const ruled::v1::PhaseChanged &pc, BatchContext &ctx)
{
    const ruled::v1::PhaseId prevPhase = state->lastEnginePhaseId;
    state->lastEnginePhaseId = pc.phase_id();
    // CR 510.4: when entering or leaving the first-strike damage substep, notify the prompt
    // widget so it can label the pass button "Combat Damage" while inside the FS step (the
    // next step is regular combat damage).
    const bool wasFsStep = prevPhase == ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE;
    const bool isFsStep = state->lastEnginePhaseId == ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE;
    if (wasFsStep != isFsStep) {
        emit state->firstStrikeDamageStepActiveChanged(isFsStep);
    }
    // Phase is already reflected by Event_SetActivePhase from the server (toolbar highlight +
    // logSetActivePhase); do not duplicate here.
    // Reaching a new phase guarantees the previous stack emptied.
    state->stackOidOrder.clear();
    state->stackTargetsByStackOid.clear();
    state->stackTargetKindByStackAndTargetOid.clear();
    ctx.stackTrackingDirty = true;
    if (host->currentActivePlayerId() != static_cast<int>(pc.active_player_id())) {
        host->setActivePlayerId(static_cast<int>(pc.active_player_id()));
    }
    const int mappedPhase = mapRuledPhaseToToolbarPhase(state->lastEnginePhaseId);
    if (mappedPhase >= 0) {
        host->setToolbarPhase(mappedPhase);
    }
    const RuledCombatPhase combatPhase = mapRuledPhaseToCombatPhase(state->lastEnginePhaseId);
    if (combatPhase == state->currentCombatPhase &&
        state->currentActivePlayerId == static_cast<int>(pc.active_player_id())) {
        return;
    }
    const RuledCombatPhase previousCombatPhase = state->currentCombatPhase;
    state->currentCombatPhase = combatPhase;
    state->currentActivePlayerId = static_cast<int>(pc.active_player_id());
    // Phase transitions reset any local pending selections.
    state->pendingAttackerOids.clear();
    state->pendingAttackAssignments.clear();
    state->attackerAwaitingDefenderOid = 0;
    state->pendingBlocks.clear();
    state->remoteBlockPreviewPairs.clear();
    state->remoteAttackerPreviewOids.clear();
    state->remoteAttackPreviewAssignments.clear();
    state->stagedBlockerOids.clear();
    // Keep committed block assignments visible/interactive while progressing from declare
    // blockers -> assign combat damage -> combat damage. Clear only when leaving combat.
    if (!isCombatPhase(previousCombatPhase) || !isCombatPhase(combatPhase)) {
        state->committedBlocks.clear();
    }
    if (combatPhase == RuledCombatPhase::DeclareAttackers) {
        state->attackersSubmittedThisStep = false;
    } else if (combatPhase == RuledCombatPhase::DeclareBlockers) {
        state->blockersSubmittedThisStep = false;
    } else if (combatPhase == RuledCombatPhase::None) {
        state->attackersSubmittedThisStep = false;
        state->blockersSubmittedThisStep = false;
    }
    if (combatPhase == RuledCombatPhase::None) {
        state->currentAttackerOids.clear();
        state->currentAttackAssignments.clear();
        state->remoteAttackerPreviewOids.clear();
        state->clearCombatDamageAssignmentState();
        state->committedBlockerGroups.clear();
    }
    if (combatPhase == RuledCombatPhase::AssignCombatDamage) {
        state->seedDefaultCombatDamageForCurrentAttacker();
    }
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyStackPushed(const ruled::v1::StackPushed &sp, BatchContext &ctx)
{
    state->stackOidOrder.prepend(sp.object_id());
    // CR 603.3b: a candidate reaching the stack means the ordering prompt has been answered. The
    // deciding client already cleared it in submitTriggerOrder(); this covers every other path to
    // the same fact (a resynced batch, a reconnect) and closes opponents' "waiting" state.
    if (state->triggerOrderCandidateOids.remove(sp.object_id())) {
        state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerOrder);
        // Only a *possible* close: a later event in this same batch may raise the next prompt, so
        // the decision is deferred to finishBatch, which reads the settled state.
        ctx.triggerOrderDirty = true;
    }
    QVector<quint32> tlist;
    tlist.reserve(sp.targets_size());
    for (int ti = 0; ti < sp.targets_size(); ++ti) {
        const auto &target = sp.targets(ti);
        const quint32 targetOid = static_cast<quint32>(target.object_id());
        tlist.append(targetOid);
        RuledTargetItemKind kind = RuledTargetItemKind::Unknown;
        switch (target.kind()) {
            case ruled::v1::TARGET_REF_KIND_PLAYER:
                kind = RuledTargetItemKind::Player;
                break;
            case ruled::v1::TARGET_REF_KIND_PERMANENT:
                kind = RuledTargetItemKind::Battlefield;
                break;
            case ruled::v1::TARGET_REF_KIND_STACK:
                kind = RuledTargetItemKind::Stack;
                break;
            case ruled::v1::TARGET_REF_KIND_GRAVEYARD:
                kind = RuledTargetItemKind::Graveyard;
                break;
            case ruled::v1::TARGET_REF_KIND_UNSPECIFIED:
                break;
        }
        state->latchTargetKind(sp.object_id(), targetOid, kind);
    }
    state->stackTargetsByStackOid.insert(sp.object_id(), tlist);
    RULED_TRACE("client") << "stackPushed: oid=" << sp.object_id() << " cardId=" << QString::fromStdString(sp.card_id())
                          << " isCopy=" << sp.is_copy() << " isTriggered=" << sp.is_triggered() << " annotation='"
                          << QString::fromStdString(sp.ability_annotation()) << "'"
                          << " description='" << QString::fromStdString(sp.description()) << "'"
                          << " -> syntheticCard=" << (sp.card_id().empty() || sp.is_copy())
                          << " (a spell with a card_id expects a REAL CardItem the relay moved onto"
                             " the stack zone; if the stack looks empty, that move is what to check)";
    // CR 608.2b: latch every target that sits in a graveyard *now*, while the choice is still fresh.
    // This is the only kind the dispatcher can identify without the UI, and it is the one that has
    // to be right immediately: `emitGraveyardTargetsNeeded` runs at the end of this batch, before
    // the deferred arrow sync classifies the rest. Doing it here rather than there also means a
    // permanent that later dies can never be mistaken for a graveyard target — by then the latch
    // already says Battlefield.
    for (quint32 targetOid : tlist) {
        if (state->graveyardOidToPlayerId.contains(targetOid)) {
            state->latchTargetKind(sp.object_id(), targetOid, RuledTargetItemKind::Graveyard);
        }
    }
    // Overlay the annotation on the stack card: an ability's text, an X value for an X spell
    // ("X = N", CR 107.3), or the cast face of a multi-face spell (e.g. "Fire" / "Ice").
    if (!sp.ability_annotation().empty()) {
        state->stackAnnotationByOid.insert(sp.object_id(), QString::fromStdString(sp.ability_annotation()));
        // Abilities (no card_id) and spell copies (is_copy) both lack a physical CardItem on the
        // stack and need a synthetic one. A normal spell (non-empty card_id, not a copy) already
        // has a real card.
        if (sp.card_id().empty()) {
            // A *triggered* ability reaching the stack means its target was chosen and is no
            // longer pending. An activated ability also has an empty card_id, and must not clear
            // the prompt: paying a sacrifice cost queues a dies trigger whose prompt would then be
            // wiped by the very ability that caused it, stranding the player with no way to answer.
            if (sp.is_triggered()) {
                state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerTarget);
                state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerMode);
            }
            // Record the source permanent so the targeting arrow starts from it.
            if (state->lastTriggerSourceOid != 0) {
                state->stackSourceOidByStackOid.insert(sp.object_id(), state->lastTriggerSourceOid);
            }
            host->createSyntheticStackCard(sp.object_id(), QString::fromStdString(sp.description()),
                                           state->lastTriggerControllerPlayerId, {});
        } else if (sp.is_copy()) {
            // The copy is being placed on the stack — any pending copy-target choice has been
            // accepted by the engine.
            state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget);
            // CR 707.10: a spell copy (Twincast/Fork) has no physical card; create a synthetic
            // stack card so the copy is visible. Inherit the original spell's printing so the copy
            // shows the same card art rather than defaulting to the newest printing.
            const auto srcOid = static_cast<quint32>(sp.copy_source_object_id());
            const QString copySetName = srcOid != 0 ? host->stackCardProviderId(srcOid) : QString{};
            host->createSyntheticStackCard(sp.object_id(), QString::fromStdString(sp.description()), -1, copySetName);
            // The StackResolved for the copy-maker (Twincast/Fork) uses counterspell cleanup logic
            // that removes targets from stackOidOrder — but the source spell was NOT removed from
            // the engine stack, so restore it here.
            if (srcOid != 0 && !state->stackOidOrder.contains(srcOid) &&
                state->stackTargetsByStackOid.contains(srcOid)) {
                state->stackOidOrder.insert(1, srcOid);
            }
        }
    }
    ctx.stackTrackingDirty = true;
}

void RuledEventDispatcher::applyStackResolved(const ruled::v1::StackResolved &sr, BatchContext &ctx)
{
    const quint32 rid = sr.object_id();
    retireStackObject(rid, ctx);
    RULED_TRACE("client") << "stackResolved: oid=" << rid << " destination=" << static_cast<int>(sr.destination())
                          << " (1=graveyard 2=battlefield 3=exile 4=library) stackOidOrderRemaining="
                          << state->stackOidOrder.size()
                          << " — the physical card is moved by the RELAY, not here; this line only"
                             " confirms the client saw the resolve";
}

void RuledEventDispatcher::applyActivePublicRevealSnapshot(
    const ruled::v1::ActivePublicRevealSnapshot &snapshot)
{
    QVector<RuledClientState::RuledActivePublicReveal> reveals;
    reveals.reserve(snapshot.reveals_size());
    for (const auto &entry : snapshot.reveals()) {
        reveals.append({entry.source_stack_object_id(), entry.group_index(), entry.revealing_player_id(),
                        QString::fromStdString(entry.source_description()), QString::fromStdString(entry.card_id()),
                        QString::fromStdString(entry.card_name())});
    }
    state->setActivePublicReveals(std::move(reveals));
}

void RuledEventDispatcher::applyStackObjectCountered(const ruled::v1::StackObjectCountered &countered,
                                                      BatchContext &ctx)
{
    const quint32 objectId = countered.object_id();
    retireStackObject(objectId, ctx);
    RULED_TRACE("client") << "stackObjectCountered: oid=" << objectId
                          << " stackOidOrderRemaining=" << state->stackOidOrder.size();
}

void RuledEventDispatcher::retireStackObject(quint32 objectId, BatchContext &ctx)
{
    const quint32 rid = objectId;
    const QVector<quint32> spellTargets = state->stackTargetsByStackOid.value(rid);
    state->stackOidOrder.removeOne(rid);
    state->stackTargetsByStackOid.remove(rid);
    for (quint32 t : spellTargets) {
        state->stackTargetKindByStackAndTargetOid.remove(RuledClientState::stackTargetKey(rid, t));
    }
    state->stackAnnotationByOid.remove(rid);
    state->stackSourceOidByStackOid.remove(rid);
    host->removeSyntheticStackCard(rid);
    ctx.stackTrackingDirty = true;
}

void RuledEventDispatcher::applyTriggerNeedsTarget(const ruled::v1::TriggerNeedsTarget &tnt, BatchContext &ctx)
{
    // Stack bookkeeping, recorded on every client (see RuledClientState::lastTriggerSourceOid).
    state->lastTriggerSourceOid = tnt.source_permanent_id();
    state->lastTriggerAbilityIndex = tnt.ability_index();
    state->lastTriggerControllerPlayerId = static_cast<int>(tnt.controller_player_id());
    const QString abilityText = QString::fromStdString(tnt.ability_text());
    // Only the controller parks the choice, so that only they can send ChooseTriggerTarget.
    if (state->lastTriggerControllerPlayerId == host->localPlayerId()) {
        RuledClientState::RuledPendingChoice choice;
        choice.kind = tnt.modes_size() > 0 ? RuledClientState::ChoiceKind::TriggerMode
                                           : RuledClientState::ChoiceKind::TriggerTarget;
        choice.promptText = abilityText;
        choice.mayDecline = tnt.may_decline();
        if (tnt.has_targets()) {
            choice.triggerTargets = parseSpellTargets(tnt.targets());
        }
        for (const auto &mode : tnt.modes()) {
            RuledChoiceOption option;
            option.index = static_cast<int>(mode.mode_index());
            option.label = QString::fromStdString(mode.label());
            option.enabled = mode.selectable();
            option.needsTarget = mode.needs_target();
            if (mode.has_targets()) {
                option.targets = parseSpellTargets(mode.targets());
            }
            choice.choiceOptions.append(option);
        }
        state->setPendingChoice(std::move(choice));
        ctx.promptFeed += tnt.modes_size() > 0 ? QStringLiteral("Choose a mode for “%1”.\n").arg(abilityText)
                                               : QStringLiteral("Choose a target for “%1”.\n").arg(abilityText);
    } else {
        state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerTarget);
        state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerMode);
    }
    emit state->triggerNeedsTarget(abilityText);
}

void RuledEventDispatcher::applyTriggerOrderRequired(const ruled::v1::TriggerOrderRequired &tor, BatchContext &ctx)
{
    using PendingChoice = RuledClientState::RuledPendingChoice;
    using ChoiceKind = RuledClientState::ChoiceKind;

    // Recorded on every client: which abilities triggered is public, so an opponent can be told
    // what is being decided even though only the decider can answer.
    QVector<RuledTriggerOrderCandidate> candidates;
    candidates.reserve(tor.candidates_size());
    for (const auto &c : tor.candidates()) {
        RuledTriggerOrderCandidate candidate;
        candidate.oid = c.trigger_object_id();
        candidate.sourceOid = c.source_permanent_id();
        candidate.cardName = QString::fromStdString(c.source_card_name());
        candidate.abilityText = QString::fromStdString(c.ability_text());
        candidates.append(candidate);
    }
    state->triggerOrderCandidateOids.clear();
    for (const auto &candidate : candidates) {
        state->triggerOrderCandidateOids.insert(candidate.oid);
    }

    const bool isDecider = static_cast<int>(tor.deciding_player_id()) == host->localPlayerId();
    if (isDecider) {
        PendingChoice choice;
        choice.kind = ChoiceKind::TriggerOrder;
        choice.promptText =
            QStringLiteral("Choose which of %1 triggers goes on the stack next.").arg(candidates.size());
        choice.orderCandidates = candidates;
        // Index ids: the popup is a ZoneViewWidget over synthetic cards, which identifies a card by
        // an int, so each candidate gets its position and maps back to its trigger oid.
        for (int i = 0; i < candidates.size(); ++i) {
            choice.orderCardIdToOid.insert(i, candidates[i].oid);
        }
        state->setPendingChoice(std::move(choice));
        ctx.promptFeed += QStringLiteral("Click the triggered ability to put on the stack next "
                                         "(%1 left) — what you pick first resolves last.\n")
                              .arg(candidates.size());
    } else {
        state->clearPendingChoiceOfKind(ChoiceKind::TriggerOrder);
        ctx.promptFeed +=
            QStringLiteral("Waiting: opponent is ordering %1 simultaneous triggers.\n").arg(candidates.size());
    }
    ctx.triggerOrderDirty = true;
}

void RuledEventDispatcher::applyResolutionChoiceRequired(const ruled::v1::ResolutionChoiceRequired &rcr,
                                                         BatchContext &ctx)
{
    using PendingChoice = RuledClientState::RuledPendingChoice;
    using ChoiceKind = RuledClientState::ChoiceKind;

    // Tier-3 custom resolution paused for a player choice (CR 608).
    ctx.promptFeed += QString::fromStdString(rcr.prompt_text()) + QStringLiteral("\n");
    const bool isDecider = static_cast<int>(rcr.deciding_player_id()) == host->localPlayerId();
    const bool isPublicReveal = rcr.reveal_audience() == ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS;
    // Retire the previous resolution UI before publishing its replacement. A repeated public
    // reveal marks its pending pick as shared, so this does not close the existing popup.
    state->clearPendingChoiceOfKind(ChoiceKind::ResolutionPick);
    state->clearPendingChoiceOfKind(ChoiceKind::ResolutionPayment);
    state->clearPendingChoiceOfKind(ChoiceKind::ResolutionBranch);
    state->clearPendingChoiceOfKind(ChoiceKind::SiegeCast);
    state->clearPendingChoiceOfKind(ChoiceKind::AttackingTokenDefender);
    if (isPublicReveal) {
        const int count = rcr.candidate_names_size();
        const bool selectableShapeValid =
            isDecider ? rcr.candidate_selectable_size() == count : rcr.candidate_selectable_size() == 0;
        const bool identityShapeValid =
            count > 0 && rcr.has_revealed_zone_owner_player_id() && rcr.candidate_object_ids_size() == count &&
            rcr.candidate_card_ids_size() == count && rcr.candidate_server_card_ids_size() == count;
        if (!identityShapeValid || !selectableShapeValid) {
            qWarning() << "Rejecting malformed ruled public reveal";
            return;
        }
        RuledClientState::RuledPublicReveal reveal;
        reveal.sourceObjectId = rcr.source_object_id();
        reveal.zoneOwnerPlayerId = rcr.revealed_zone_owner_player_id();
        for (int i = 0; i < count; ++i) {
            reveal.candidateNames.append(QString::fromStdString(rcr.candidate_names(i)));
            reveal.candidateServerCardIds.append(rcr.candidate_server_card_ids(i));
        }
        const bool snapshotChanged = !state->publicReveal.has_value() || *state->publicReveal != reveal;
        if (snapshotChanged) {
            ctx.timeline += QStringLiteral("P%1 reveals: %2.\n")
                                .arg(reveal.zoneOwnerPlayerId)
                                .arg(reveal.candidateNames.join(QStringLiteral(", ")));
        }
        state->setPublicReveal(std::move(reveal));
        ctx.publicRevealSeen = true;
    }
    if (!isDecider) {
        state->resolutionChoiceWaitingPlayerId = static_cast<int>(rcr.deciding_player_id());
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH) {
        PendingChoice choice;
        choice.kind = ChoiceKind::ResolutionBranch;
        choice.promptText = QString::fromStdString(rcr.prompt_text());
        choice.mayDecline = rcr.min() == 0;
        bool anySearchZones = false;
        bool malformedSearchZones = false;
        for (const auto &branch : rcr.resolution_branches()) {
            RuledChoiceOption option;
            option.index = static_cast<int>(branch.branch_index());
            option.label = QString::fromStdString(branch.label());
            option.enabled = branch.selectable();
            for (const int rawZone : branch.search_zones()) {
                if (rawZone != ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND &&
                    rawZone != ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD &&
                    rawZone != ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY) {
                    malformedSearchZones = true;
                    break;
                }
                if (option.searchZones.contains(rawZone)) {
                    malformedSearchZones = true;
                    break;
                }
                option.searchZones.insert(rawZone);
            }
            anySearchZones = anySearchZones || !option.searchZones.isEmpty();
            choice.choiceOptions.append(option);
        }
        if (malformedSearchZones ||
            (anySearchZones && std::any_of(choice.choiceOptions.cbegin(), choice.choiceOptions.cend(),
                                           [](const RuledChoiceOption &option) {
                                               return option.searchZones.isEmpty();
                                           }))) {
            qWarning() << "Rejecting malformed ruled search-zone branches";
            return;
        }
        state->setPendingChoice(std::move(choice));
        emit state->combatStateChanged();
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANA_PAYMENT) {
        PendingChoice payment;
        payment.kind = ChoiceKind::ResolutionPayment;
        payment.promptText = QString::fromStdString(rcr.prompt_text());
        payment.genericManaCost = static_cast<int>(rcr.generic_mana_cost());
        payment.paymentCurrentlyLegal = rcr.payment_currently_legal();
        payment.manaCost = QString::fromStdString(rcr.mana_cost());
        state->setPendingChoice(std::move(payment));
        emit state->resolutionPaymentUiChanged(true);
        emit state->combatStateChanged();
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER) {
        if (rcr.min() != 1 || rcr.max() != 1 || rcr.combat_defender_options_size() <= 0) {
            qWarning() << "Rejecting malformed attacking-token defender choice";
            return;
        }
        PendingChoice choice;
        choice.kind = ChoiceKind::AttackingTokenDefender;
        choice.promptText = QString::fromStdString(rcr.prompt_text());
        for (const auto &option : rcr.combat_defender_options()) {
            if (!option.has_defender() ||
                (option.defender().kind() != ruled::v1::TARGET_REF_KIND_PLAYER &&
                 option.defender().kind() != ruled::v1::TARGET_REF_KIND_PERMANENT)) {
                qWarning() << "Rejecting malformed attacking-token defender option";
                return;
            }
            choice.combatDefenderOptions.append(option);
        }
        state->setPendingChoice(std::move(choice));
        emit state->combatStateChanged();
        return;
    }

    const bool isEmptyLibrarySearch = rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH &&
                                      rcr.candidate_object_ids_size() == 0 && rcr.min() == 0;
    if (rcr.candidate_object_ids_size() <= 0 && !isEmptyLibrarySearch) {
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_TARGET_OBJECTS ||
        rcr.choice_kind() == ruled::v1::CHOICE_KIND_COST_OBJECTS ||
        rcr.choice_kind() == ruled::v1::CHOICE_KIND_COPY_SOURCE ||
        rcr.choice_kind() == ruled::v1::CHOICE_KIND_LEGEND_KEEP ||
        rcr.choice_kind() == ruled::v1::CHOICE_KIND_AURA_PERMANENT ||
        rcr.choice_kind() == ruled::v1::CHOICE_KIND_AURA_PLAYER ||
        rcr.choice_kind() == ruled::v1::CHOICE_KIND_BATTLE_PROTECTOR) {
        // Click-to-select on the battlefield rather than a modal list:
        //   TARGET_OBJECTS — CR 707.10c, the controller of a spell copy may redirect its targets.
        //   LEGEND_KEEP    — CR 704.5j, the controller clicks the legend to KEEP; the rest are
        //                    sacrificed.
        PendingChoice choice;
        if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_COST_OBJECTS) {
            choice.kind = ChoiceKind::CostObjects;
            choice.min = static_cast<int>(rcr.min());
            choice.max = static_cast<int>(rcr.max());
        } else if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LEGEND_KEEP) {
            choice.kind = ChoiceKind::LegendKeep;
        } else if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_COPY_SOURCE) {
            choice.kind = ChoiceKind::CopySource;
        } else if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_AURA_PERMANENT) {
            choice.kind = ChoiceKind::AuraPermanent;
        } else if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_AURA_PLAYER) {
            choice.kind = ChoiceKind::AuraPlayer;
        } else if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_BATTLE_PROTECTOR) {
            choice.kind = ChoiceKind::BattleProtector;
        } else {
            choice.kind = ChoiceKind::CopyTarget;
        }
        choice.promptText = QString::fromStdString(rcr.prompt_text());
        choice.mayDecline = rcr.min() == 0;
        for (int i = 0; i < rcr.candidate_object_ids_size(); ++i) {
            choice.candidateOids.append(rcr.candidate_object_ids(i));
        }
        const bool isLegendKeep = choice.kind == ChoiceKind::LegendKeep;
        state->setPendingChoice(std::move(choice));
        if (isLegendKeep) {
            emit state->combatStateChanged();
        }
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS &&
        rcr.candidate_server_card_ids_size() == rcr.candidate_object_ids_size()) {
        // HandCards with server card ids: use the hand-click UI.
        PendingChoice pick;
        pick.kind = ChoiceKind::ResolutionPick;
        pick.min = static_cast<int>(rcr.min());
        pick.max = static_cast<int>(rcr.max());
        pick.promptText = QString::fromStdString(rcr.prompt_text());
        pick.pickZone = PickZone::Hand;
        for (int i = 0; i < rcr.candidate_object_ids_size(); ++i) {
            const quint32 oid = rcr.candidate_object_ids(i);
            const int scid = rcr.candidate_server_card_ids(i);
            if (scid >= 0) {
                pick.serverCardIdToOid.insert(scid, oid);
                if (i < rcr.candidate_names_size()) {
                    pick.candidateNames.append(QString::fromStdString(rcr.candidate_names(i)));
                }
            }
        }
        const int required = pick.min;
        state->setPendingChoice(std::move(pick));
        emit state->resolutionHandPickUiChanged(required, 0);
        emit state->combatStateChanged();
        return;
    }

    const bool isLibrarySearch = rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH;
    const bool isLibraryLook = rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_LOOK;
    const bool isManifestDread = rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANIFEST_DREAD;
    const bool isZoneSearch = rcr.choice_kind() == ruled::v1::CHOICE_KIND_ZONE_SEARCH;
    const bool isGraveyardCards = rcr.choice_kind() == ruled::v1::CHOICE_KIND_GRAVEYARD_CARDS;
    if (isLibraryLook && (rcr.candidate_object_ids_size() != rcr.candidate_names_size() ||
                          rcr.candidate_server_card_ids_size() != rcr.candidate_names_size() ||
                          rcr.candidate_selectable_size() != rcr.candidate_names_size())) {
        qWarning() << "Rejecting malformed ruled library-look choice";
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_SIEGE_CAST) {
        if (rcr.candidate_object_ids_size() != 1) {
            qWarning() << "Rejecting malformed Siege cast offer";
            return;
        }
        PendingChoice choice;
        choice.kind = ChoiceKind::SiegeCast;
        choice.promptText = QString::fromStdString(rcr.prompt_text());
        choice.candidateOids.append(rcr.candidate_object_ids(0));
        choice.choiceOptions.append({0, tr("Decline"), true});
        choice.choiceOptions.append({1, tr("Cast transformed"), true});
        state->setPendingChoice(std::move(choice));
        emit state->combatStateChanged();
        return;
    }
    if ((isZoneSearch || isGraveyardCards) &&
        (rcr.candidate_source_zones_size() != rcr.candidate_names_size() ||
         rcr.candidate_object_ids_size() != rcr.candidate_names_size())) {
        qWarning() << "Rejecting malformed ruled multi-zone choice";
        return;
    }

    if ((isLibrarySearch || rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_TOP || isLibraryLook ||
         isManifestDread || isZoneSearch || isGraveyardCards) &&
        rcr.candidate_server_card_ids_size() == rcr.candidate_names_size() &&
        (rcr.candidate_names_size() > 0 || isEmptyLibrarySearch)) {
        // LibrarySearch, LibraryTop, LibraryLook, or ManifestDread with server card ids: deck
        // zone-view pick. All
        // show card images from the local library, so they share the popup — only the title and
        // optional engine-authored click eligibility differ.
        // LIBRARY_TOP is CR 701.18 scry, which may arrive twice for one spell: once to pick the
        // cards going to the bottom (min 0), then, if two or more stay on top, ordered to arrange
        // them. Click order carries the ordering, exactly as it does for Brainstorm's hand pick.
        // unique_names is always true for Gifts Ungiven step 1.
        PendingChoice pick;
        pick.kind = ChoiceKind::ResolutionPick;
        pick.min = static_cast<int>(rcr.min());
        pick.max = static_cast<int>(rcr.max());
        pick.uniqueNames = rcr.unique_names();
        pick.promptText = QString::fromStdString(rcr.prompt_text());
        pick.pickZone = PickZone::Deck;
        if (isLibraryLook) {
            pick.viewTitle = tr("Look at cards");
            pick.hasSelectableRestriction = true;
            pick.showViewControls = false;
        } else if (isManifestDread) {
            pick.viewTitle = tr("Manifest dread");
            pick.showViewControls = false;
        } else if (isZoneSearch) {
            pick.viewTitle = tr("Search selected zones");
            pick.showViewControls = false;
        } else if (isGraveyardCards) {
            pick.viewTitle = tr("Choose from your graveyard");
            pick.showViewControls = false;
        } else {
            pick.viewTitle =
                rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_TOP ? tr("Scry") : tr("Search your library");
        }
        QVector<int> libScids;
        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
            const quint32 oid = (i < rcr.candidate_object_ids_size()) ? rcr.candidate_object_ids(i) : 0;
            const int scid = rcr.candidate_server_card_ids(i);
            const QString name = QString::fromStdString(rcr.candidate_names(i));
            if (scid >= 0) {
                pick.serverCardIdToOid.insert(scid, oid);
                pick.serverCardIdToName.insert(scid, name);
                if (isLibraryLook && rcr.candidate_selectable(i)) {
                    pick.selectableServerCardIds.insert(scid);
                }
            }
            pick.candidateNames.append(name);
            if (isZoneSearch || isGraveyardCards) {
                switch (rcr.candidate_source_zones(i)) {
                    case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND:
                        pick.candidateAnnotations.append(tr("Hand"));
                        break;
                    case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD:
                        pick.candidateAnnotations.append(tr("Graveyard"));
                        break;
                    case ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY:
                        pick.candidateAnnotations.append(tr("Library"));
                        break;
                    default:
                        qWarning() << "Rejecting ruled choice with unspecified source zone";
                        return;
                }
            } else {
                pick.candidateAnnotations.append(QString{});
            }
            libScids.append(scid);
        }
        const int required = pick.min;
        const QStringList pickNames = pick.candidateNames;
        state->setPendingChoice(std::move(pick));
        emit state->resolutionHandPickUiChanged(required, 0);
        emit state->librarySearchPickStarted(pickNames, libScids);
        emit state->combatStateChanged();
        return;
    }

    if ((rcr.choice_kind() == ruled::v1::CHOICE_KIND_REVEALED ||
         rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND) &&
        rcr.candidate_server_card_ids_size() == rcr.candidate_names_size() && rcr.candidate_names_size() > 0) {
        const bool isOpponentHand = rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND;
        if (isOpponentHand && rcr.candidate_selectable_size() != rcr.candidate_names_size()) {
            qWarning() << "Rejecting malformed ruled opponent-hand choice";
            return;
        }
        // RevealedCards or PrivateRevealedHand with server card ids: zone popup pick. The deciding
        // player chooses from the revealed cards (OpponentHand = a target player's hand shown only
        // to the caster; the relay redacted it from everyone else).
        PendingChoice pick;
        pick.kind = ChoiceKind::ResolutionPick;
        pick.min = static_cast<int>(rcr.min());
        pick.max = static_cast<int>(rcr.max());
        pick.promptText = QString::fromStdString(rcr.prompt_text());
        pick.pickZone = PickZone::Revealed;
        pick.hasSelectableRestriction = isOpponentHand;
        pick.publicReveal = isPublicReveal;
        // Both kinds render identically, but they are not the same thing: REVEALED was shown to
        // the whole table, OPPONENT_HAND is a hand only the decider may look at (CR 701.7). The
        // proto does not carry whose hand it is, so the title stays seat-agnostic — if a future
        // card picks a player without targeting one, that owner id has to come across the wire.
        pick.viewTitle = rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND ? tr("Target player's hand")
                                                                                   : tr("Revealed cards");
        QStringList names;
        QVector<int> scids;
        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
            const quint32 oid = (i < rcr.candidate_object_ids_size()) ? rcr.candidate_object_ids(i) : 0;
            const int scid = rcr.candidate_server_card_ids(i);
            if (scid >= 0) {
                pick.serverCardIdToOid.insert(scid, oid);
                if (isOpponentHand && rcr.candidate_selectable(i)) {
                    pick.selectableServerCardIds.insert(scid);
                }
            }
            const QString name = QString::fromStdString(rcr.candidate_names(i));
            pick.candidateNames.append(name);
            names.append(name);
            scids.append(scid);
        }
        const int required = pick.min;
        const int maximum = pick.max;
        state->setPendingChoice(std::move(pick));
        emit state->resolutionHandPickUiChanged(required, 0);
        if (!isPublicReveal) {
            emit state->revealedPickChanged(true, names, scids, required, maximum);
        }
        emit state->combatStateChanged();
        return;
    }

    // Fallback: modal dialog for unrecognised kinds or missing server card ids.
    QVector<quint32> oids;
    for (int i = 0; i < rcr.candidate_object_ids_size(); ++i) {
        oids.append(rcr.candidate_object_ids(i));
    }
    QStringList names;
    for (int i = 0; i < rcr.candidate_names_size(); ++i) {
        names.append(QString::fromStdString(rcr.candidate_names(i)));
    }
    host->requestResolutionChoiceDialog(QString::fromStdString(rcr.prompt_text()), oids, names,
                                        static_cast<int>(rcr.min()), static_cast<int>(rcr.max()), rcr.ordered(),
                                        rcr.unique_names());
}

void RuledEventDispatcher::applyBattlefieldObjectMap(const ruled::v1::BattlefieldObjectMap &map, BatchContext &ctx)
{
    state->ownerCardIdToEngineOid.clear();
    state->engineOidToCardId.clear();
    state->engineOidOwner.clear();
    state->engineOidSummoningSick.clear();
    state->engineOidHaste.clear();
    state->engineOidTrample.clear();
    state->engineOidCreature.clear();
    state->engineOidMarkedDamage.clear();
    state->engineOidBattlefieldPower.clear();
    state->engineOidBattlefieldToughness.clear();
    QSet<quint32> validOids;
    for (const auto &entry : map.entries()) {
        validOids.insert(entry.engine_object_id());
        state->engineOidOwner.insert(entry.engine_object_id(), entry.player_id());
        state->engineOidSummoningSick.insert(entry.engine_object_id(), entry.summoning_sick());
        bool hasHaste = false;
        bool hasTrample = false;
        for (const std::string &keyword : entry.keywords()) {
            hasHaste = hasHaste || keyword == "Haste";
            hasTrample = hasTrample || keyword == "Trample";
        }
        state->engineOidHaste.insert(entry.engine_object_id(), hasHaste);
        state->engineOidTrample.insert(entry.engine_object_id(), hasTrample);
        state->engineOidCreature.insert(entry.engine_object_id(), entry.is_creature());
        if (entry.server_card_id() >= 0) {
            state->ownerCardIdToEngineOid.insert(
                RuledClientState::makeOwnedCardKey(entry.player_id(), entry.server_card_id()),
                entry.engine_object_id());
            state->engineOidToCardId.insert(entry.engine_object_id(), entry.server_card_id());
        }
    }
    auto pruneByKnownOid = [&validOids](QHash<quint32, quint32> &pairs) {
        for (auto it = pairs.begin(); it != pairs.end();) {
            if (!validOids.contains(it.key()) || !validOids.contains(it.value())) {
                it = pairs.erase(it);
            } else {
                ++it;
            }
        }
    };
    pruneByKnownOid(state->pendingBlocks);
    pruneByKnownOid(state->committedBlocks);
    pruneByKnownOid(state->remoteBlockPreviewPairs);
    for (auto it = state->remoteAttackerPreviewOids.begin(); it != state->remoteAttackerPreviewOids.end();) {
        if (!validOids.contains(*it)) {
            it = state->remoteAttackerPreviewOids.erase(it);
        } else {
            ++it;
        }
    }
    for (auto it = state->stagedBlockerOids.begin(); it != state->stagedBlockerOids.end();) {
        if (!validOids.contains(*it)) {
            it = state->stagedBlockerOids.erase(it);
        } else {
            ++it;
        }
    }
    // Re-register synthetic ability stack card OID mappings cleared above. Use the per-OID
    // controller pid (not always localPid) so the mapping is keyed consistently with the host's
    // createSyntheticStackCard.
    if (!state->syntheticAbilityFakeIds.isEmpty()) {
        const int localPid = host->localPlayerId();
        for (auto sit = state->syntheticAbilityFakeIds.constBegin(); sit != state->syntheticAbilityFakeIds.constEnd();
             ++sit) {
            const int ctrlPid = state->syntheticAbilityControllerPid.value(sit.key(), localPid);
            state->ownerCardIdToEngineOid.insert(RuledClientState::makeOwnedCardKey(ctrlPid, sit.value()), sit.key());
        }
    }
    ctx.battlefieldMapDirty = true;
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyHandSlotMap(const ruled::v1::HandSlotMap &map)
{
    // Servatrice sends this map only when hand composition changed, so an absent event means
    // "unchanged" and the previous slots stay valid; a present one is a full replacement, hence
    // the clear here rather than per batch (same contract as applyBattlefieldObjectMap /
    // applyGraveyardObjectMap).
    state->ownedCardToEngineHandSlot.clear();
    for (int hi = 0; hi < map.entries_size(); ++hi) {
        const auto &ent = map.entries(hi);
        state->ownedCardToEngineHandSlot.insert(
            RuledClientState::makeOwnedCardKey(ent.player_id(), ent.server_card_id()),
            static_cast<int>(ent.hand_index()));
    }
}

void RuledEventDispatcher::applyFaceDownObjectMap(const ruled::v1::FaceDownObjectMap &map, BatchContext &ctx)
{
    state->privateFaceDownNameByOwnedCard.clear();
    state->privateFaceDownGenerationByOid.clear();
    for (const auto &entry : map.entries()) {
        state->privateFaceDownNameByOwnedCard.insert(
            RuledClientState::makeOwnedCardKey(entry.controller_player_id(), entry.server_card_id()),
            QString::fromStdString(entry.card_name()));
        state->privateFaceDownGenerationByOid.insert(entry.engine_object_id(), entry.zone_change_generation());
    }
    ctx.battlefieldMapDirty = true;
}

void RuledEventDispatcher::applyGraveyardObjectMap(const ruled::v1::GraveyardObjectMap &map)
{
    state->ownedGraveyardCardToEngineOid.clear();
    state->graveyardOidToPlayerId.clear();
    state->graveyardOidToServerCardId.clear();
    for (int gi = 0; gi < map.entries_size(); ++gi) {
        const auto &ent = map.entries(gi);
        // Key on (owner, card id): the entry's player_id has always been on the wire, and it is
        // what keeps two graveyards holding the same Server_Card.id apart.
        state->ownedGraveyardCardToEngineOid.insert(
            RuledClientState::makeOwnedCardKey(ent.player_id(), ent.server_card_id()),
            static_cast<quint32>(ent.engine_object_id()));
        state->graveyardOidToPlayerId.insert(static_cast<quint32>(ent.engine_object_id()), ent.player_id());
        state->graveyardOidToServerCardId.insert(static_cast<quint32>(ent.engine_object_id()), ent.server_card_id());
    }
}

void RuledEventDispatcher::applyExileObjectMap(const ruled::v1::ExileObjectMap &map)
{
    state->ownedExileCardToEngineOid.clear();
    state->exileOidToPlayerId.clear();
    state->exileOidToServerCardId.clear();
    for (const auto &entry : map.entries()) {
        const quint32 oid = static_cast<quint32>(entry.engine_object_id());
        state->ownedExileCardToEngineOid.insert(
            RuledClientState::makeOwnedCardKey(entry.player_id(), entry.server_card_id()), oid);
        state->exileOidToPlayerId.insert(oid, entry.player_id());
        state->exileOidToServerCardId.insert(oid, entry.server_card_id());
    }
}

void RuledEventDispatcher::applyZoneView(const ruled::v1::ZoneViewSync &view, BatchContext &ctx)
{
    if (!view.battlefields_unchanged()) {
        state->engineOidMarkedDamage.clear();
        state->engineOidBattlefieldPower.clear();
        state->engineOidBattlefieldToughness.clear();
        state->engineOidLoyalty.clear();
        state->engineOidDefense.clear();
        state->engineOidBattleProtector.clear();
        state->engineOidToActivatedAbilityTexts.clear();
        state->engineOidToActivatedAbilityManaCosts.clear();
        state->engineOidToActivatedAbilityManaProduced.clear();
        state->engineOidToActivatedAbilityCostLabels.clear();
        state->engineOidToActivatedAbilityActivatable.clear();
        state->battlefieldGenerationByOid.clear();
    }
    bool anyFirstStrikePending = false;
    for (const auto &p : view.per_player()) {
        if (p.first_strike_step_pending()) {
            anyFirstStrikePending = true;
        }
        if (view.battlefields_unchanged()) {
            continue;
        }
        for (const auto &battlefieldObject : p.battlefield_objects()) {
            const quint32 oid = battlefieldObject.object_id();
            const int damage = static_cast<int>(battlefieldObject.damage());
            if (oid != 0 && damage > 0) {
                state->engineOidMarkedDamage.insert(oid, damage);
            }
            if (oid == 0) {
                continue;
            }
            state->battlefieldGenerationByOid.insert(oid, battlefieldObject.zone_change_generation());
            QStringList texts;
            QStringList manaCosts;
            QStringList manaProduced;
            QStringList costLabels;
            QVector<bool> activatable;
            for (const auto &ability : battlefieldObject.activated_abilities()) {
                texts.append(QString::fromStdString(ability.text()));
                manaCosts.append(QString::fromStdString(ability.mana_cost()));
                manaProduced.append(QString::fromStdString(ability.mana_produced()));
                costLabels.append(QString::fromStdString(ability.cost_label()));
                activatable.append(ability.activatable());
            }
            if (!texts.isEmpty()) {
                state->engineOidToActivatedAbilityTexts.insert(oid, texts);
                state->engineOidToActivatedAbilityManaCosts.insert(oid, manaCosts);
                state->engineOidToActivatedAbilityManaProduced.insert(oid, manaProduced);
                state->engineOidToActivatedAbilityCostLabels.insert(oid, costLabels);
                state->engineOidToActivatedAbilityActivatable.insert(oid, activatable);
            }
            state->engineOidBattlefieldPower.insert(oid, static_cast<int>(battlefieldObject.power()));
            state->engineOidBattlefieldToughness.insert(oid, static_cast<int>(battlefieldObject.toughness()));
            if (battlefieldObject.is_planeswalker()) {
                state->engineOidLoyalty.insert(oid, static_cast<int>(battlefieldObject.loyalty()));
            }
            if (battlefieldObject.is_battle()) {
                state->engineOidDefense.insert(oid, static_cast<int>(battlefieldObject.defense()));
                if (battlefieldObject.has_battle_protector_player_id()) {
                    state->engineOidBattleProtector.insert(oid, battlefieldObject.battle_protector_player_id());
                }
            }
        }
    }
    if (anyFirstStrikePending != state->firstStrikeStepPending) {
        state->firstStrikeStepPending = anyFirstStrikePending;
        emit state->firstStrikeStepPendingChanged(state->firstStrikeStepPending);
    }
    ctx.battlefieldMapDirty = true;
}

void RuledEventDispatcher::applyAttackersDeclared(const ruled::v1::AttackersDeclared &ad, BatchContext &ctx)
{
    state->currentAttackerOids.clear();
    state->currentAttackAssignments.clear();
    for (const auto &assignment : ad.assignments()) {
        const auto oid = static_cast<quint32>(assignment.attacker_object_id());
        state->currentAttackerOids.insert(oid);
        state->currentAttackAssignments.insert(oid, assignment);
    }
    // Active player's pending picks are now committed; clear them.
    state->pendingAttackerOids.clear();
    state->pendingAttackAssignments.clear();
    state->attackerAwaitingDefenderOid = 0;
    state->remoteAttackerPreviewOids.clear();
    state->remoteAttackPreviewAssignments.clear();
    state->attackersSubmittedThisStep = true;
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyAttackersAdded(const ruled::v1::AttackersAdded &added, BatchContext &ctx)
{
    for (const auto &assignment : added.assignments()) {
        const auto oid = static_cast<quint32>(assignment.attacker_object_id());
        state->currentAttackerOids.insert(oid);
        state->currentAttackAssignments.insert(oid, assignment);
    }
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyAttackersPreview(const ruled::v1::AttackersPreview &ap, BatchContext &ctx)
{
    if (static_cast<int>(ap.declaring_player_id()) != host->localPlayerId()) {
        state->remoteAttackerPreviewOids.clear();
        state->remoteAttackPreviewAssignments.clear();
        for (const auto &assignment : ap.assignments()) {
            const auto oid = static_cast<quint32>(assignment.attacker_object_id());
            state->remoteAttackerPreviewOids.insert(oid);
            state->remoteAttackPreviewAssignments.insert(oid, assignment);
        }
    }
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyBlockersDeclared(const ruled::v1::BlockersDeclared &bd, BatchContext &ctx)
{
    state->committedBlocks.clear();
    state->pendingBlocks.clear();
    state->remoteBlockPreviewPairs.clear();
    state->stagedBlockerOids.clear();
    state->committedBlockerGroups.clear();
    for (int bpi = 0; bpi < bd.block_pairs_size(); ++bpi) {
        const auto &bp = bd.block_pairs(bpi);
        const auto attOid = static_cast<quint32>(bp.attacker_id());
        const auto blkOid = static_cast<quint32>(bp.blocker_id());
        state->committedBlocks.insert(blkOid, attOid);
        state->committedBlockerGroups[attOid].append(blkOid);
    }
    // Queue attackers that need explicit combat damage assignment: any attacker with 2+ blockers,
    // or a trample attacker with 1+ blockers (CR 702.19: trample excess goes to defending player).
    state->clearCombatDamageAssignmentState();
    for (auto it = state->committedBlockerGroups.constBegin(); it != state->committedBlockerGroups.constEnd(); ++it) {
        const bool singleWithTrample = it.value().size() == 1 && state->engineOidTrample.value(it.key(), false);
        if (it.value().size() > 1 || singleWithTrample) {
            state->combatDamagePendingAttackers.append(it.key());
        }
    }
    if (!state->combatDamagePendingAttackers.isEmpty()) {
        state->currentCombatDamageAttackerIdx = 0;
        state->seedDefaultCombatDamageForCurrentAttacker();
    }
    state->blockersSubmittedThisStep = true;
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyCombatDamageAssigned(const ruled::v1::CombatDamageAssigned &cda, BatchContext &ctx)
{
    const quint32 doneAtt = cda.attacker_id();
    if (state->currentCombatDamageAttackerIdx >= 0 &&
        state->currentCombatDamageAttackerIdx < state->combatDamagePendingAttackers.size() &&
        state->combatDamagePendingAttackers.at(state->currentCombatDamageAttackerIdx) == doneAtt) {
        state->pendingCombatDamageByBlocker.clear();
        state->currentCombatDamageAttackerIdx++;
        if (state->currentCombatDamageAttackerIdx < state->combatDamagePendingAttackers.size()) {
            state->seedDefaultCombatDamageForCurrentAttacker();
        }
    }
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyBlockersPreview(const ruled::v1::BlockersPreview &bp, BatchContext &ctx)
{
    if (static_cast<int>(bp.declaring_player_id()) != host->localPlayerId()) {
        state->remoteBlockPreviewPairs.clear();
        for (int bpi = 0; bpi < bp.block_pairs_size(); ++bpi) {
            const auto &pair = bp.block_pairs(bpi);
            state->remoteBlockPreviewPairs.insert(static_cast<quint32>(pair.blocker_id()),
                                                  static_cast<quint32>(pair.attacker_id()));
        }
    }
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyRemovedFromCombat(const ruled::v1::CreaturesRemovedFromCombat &rfc, BatchContext &ctx)
{
    for (const auto rawOid : rfc.object_ids()) {
        const auto oid = static_cast<quint32>(rawOid);
        state->currentAttackerOids.remove(oid);
        // Clean up attacker-side of blocker groups.
        state->committedBlockerGroups.remove(oid);
        // Clean up blocker-side: remove this blocker from any group.
        state->committedBlocks.remove(oid);
        for (auto git = state->committedBlockerGroups.begin(); git != state->committedBlockerGroups.end();) {
            git.value().removeAll(oid);
            if (git.value().isEmpty()) {
                git = state->committedBlockerGroups.erase(git);
            } else {
                ++git;
            }
        }
    }
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyLifeChanged(const ruled::v1::LifeChanged &lc, BatchContext &ctx)
{
    ctx.timeline +=
        QStringLiteral("Life: P%1 is now %2 (%3)\n").arg(lc.player_id()).arg(lc.new_total()).arg(lc.delta());
}

void RuledEventDispatcher::applyManaPoolUpdated(const ruled::v1::ManaPoolUpdated &mpu, BatchContext &ctx)
{
    QVector<RuledRestrictedManaGroup> groups;
    groups.reserve(mpu.restricted_groups_size());
    for (const auto &group : mpu.restricted_groups()) {
        groups.append({group.restriction_group_id(), static_cast<int>(group.w()), static_cast<int>(group.u()),
                       static_cast<int>(group.b()), static_cast<int>(group.r()), static_cast<int>(group.g()),
                       static_cast<int>(group.c()), QString::fromStdString(group.display_label())});
    }
    std::sort(groups.begin(), groups.end(),
              [](const auto &left, const auto &right) { return left.groupId < right.groupId; });
    const int playerId = static_cast<int>(mpu.player_id());
    state->restrictedManaByPlayer.insert(playerId, groups);
    ctx.restrictedManaDirtyPlayers.insert(playerId);
}

// ---------------------------------------------------------------------------------------
// Legal actions
// ---------------------------------------------------------------------------------------

void RuledEventDispatcher::applyLegalActions(const ruled::v1::LegalActions &actions, BatchContext &ctx)
{
    state->handActions = copyHandActions(actions);
    for (const auto &action : actions.zone_cast_actions()) {
        const int objectId = static_cast<int>(action.object_id());
        const int faceIndex = static_cast<int>(action.face_index());
        const auto castMethod = action.cast_method();
        const RuledCastSource source = action.source_zone() == ruled::v1::CAST_SOURCE_ZONE_EXILE
                                           ? RuledCastSource::Exile
                                           : RuledCastSource::Graveyard;
        const quint64 castKey = RuledClientState::zoneCastActionKey(objectId, faceIndex, source, castMethod);
        state->zoneCastActions.handIndices.insert(objectId);
        const QString cardName = QString::fromStdString(action.card_name());
        QString displayName = cardName;
        if (castMethod == ruled::v1::CAST_METHOD_FLASHBACK) {
            displayName += tr(" — Flashback");
        } else if (castMethod == ruled::v1::CAST_METHOD_HARMONIZE) {
            displayName += tr(" — Harmonize");
        }
        state->zoneCastActions.indicesByCardName.insert(cardName, objectId);
        state->zoneCastActions.faceOptionsByIndex[objectId].append({faceIndex, displayName,
                                                                    QString::fromStdString(action.cost()),
                                                                    static_cast<int>(action.generic_cost_reduction()),
                                                                    castMethod, action.has_convoke()});
        state->zoneCastSourceByOid.insert(objectId, source);
        if (action.needs_target()) {
            state->zoneCastActions.needsTargetIndices.insert(objectId);
        }
        state->zoneCastCostsByCastKey.insert(castKey, QString::fromStdString(action.cost()));
        if (action.has_cost_choices()) {
            state->zoneCastActions.costDataByCastKey.insert(castKey, parseCostData(action.cost_choices()));
        }
        QSet<quint32> eligibleGroups;
        for (const quint32 groupId : action.eligible_restricted_mana_group_ids()) {
            eligibleGroups.insert(groupId);
        }
        state->zoneCastActions.eligibleRestrictedManaByCastKey.insert(castKey, eligibleGroups);
        if (action.modes_size() > 0) {
            state->zoneCastActions.modalMinModesByCastKey.insert(castKey, static_cast<int>(action.min_modes()));
            state->zoneCastActions.modalMaxModesByCastKey.insert(castKey, static_cast<int>(action.max_modes()));
            QVector<RuledModalSpellOption> modes;
            for (const auto &mode : action.modes()) {
                modes.append(
                    {static_cast<int>(mode.mode_index()), QString::fromStdString(mode.label()), mode.selectable(),
                     mode.needs_target(),
                     mode.has_targets() ? parseSpellTargets(mode.targets()) : RuledClientState::SpellTargetData{}});
            }
            state->zoneCastActions.modalOptionsByCastKey.insert(castKey, modes);
        }
    }

    for (const auto &action : actions.zone_land_actions()) {
        const quint32 objectId = static_cast<quint32>(action.object_id());
        state->zoneLandFacesByOid[objectId].append(
            {static_cast<int>(action.face_index()), QString::fromStdString(action.card_name()), QString(), 0});
    }

    QHash<quint64, RuledExilePlayPermissionGroup> permissionGroups;
    for (const auto &group : actions.exile_play_permission_groups()) {
        RuledExilePlayPermissionGroup parsed;
        parsed.groupId = static_cast<quint64>(group.group_id());
        parsed.sourceLabel = QString::fromStdString(group.source_label());
        parsed.objectIds.reserve(group.object_ids_size());
        for (const quint32 objectId : group.object_ids()) {
            parsed.objectIds.append(objectId);
        }
        permissionGroups.insert(parsed.groupId, parsed);
    }
    if (permissionGroups != state->exilePlayPermissionGroups) {
        state->exilePlayPermissionGroups = permissionGroups;
        emit state->exilePlayPermissionGroupsChanged();
    }

    state->validTargetsByHandSlot.clear();
    for (const auto &entry : actions.valid_targets_by_hand_slot()) {
        // Key is the engine's composite (hand slot << 8 | face index); stored verbatim and
        // matched by RuledClientState::spellTargetKey().
        state->validTargetsByHandSlot.insert(static_cast<int>(entry.first), parseSpellTargets(entry.second));
    }
    state->validTargetsByZoneObject.clear();
    for (const auto &entry : actions.valid_targets_by_zone_object()) {
        state->validTargetsByZoneObject.insert(static_cast<quint64>(entry.first), parseSpellTargets(entry.second));
    }
    state->validTargetsByAbility.clear();
    for (const auto &entry : actions.valid_targets_by_ability()) {
        state->validTargetsByAbility.insert(static_cast<quint64>(entry.first), parseSpellTargets(entry.second));
    }
    state->abilityCostData.clear();
    for (const auto &entry : actions.cost_choices_by_ability()) {
        state->abilityCostData.insert(static_cast<quint64>(entry.first), parseCostData(entry.second));
    }
    state->eligibleRestrictedManaByAbility.clear();
    for (const auto &entry : actions.mana_payment_by_ability()) {
        QSet<quint32> eligibleGroups;
        for (const quint32 groupId : entry.second.eligible_restricted_mana_group_ids()) {
            eligibleGroups.insert(groupId);
        }
        state->eligibleRestrictedManaByAbility.insert(static_cast<quint64>(entry.first), eligibleGroups);
    }
    state->permanentActionsByOid.clear();
    for (const auto &action : actions.permanent_actions()) {
        RuledPermanentAction parsed;
        parsed.kind = action.kind();
        parsed.objectId = action.object_id();
        parsed.zoneChangeGeneration = action.zone_change_generation();
        parsed.label = QString::fromStdString(action.label());
        parsed.manaCost = QString::fromStdString(action.mana_cost());
        if (action.has_face_index()) {
            parsed.faceIndex = action.face_index();
        }
        for (const quint32 groupId : action.eligible_restricted_mana_group_ids()) {
            parsed.eligibleRestrictedManaGroupIds.insert(groupId);
        }
        state->permanentActionsByOid[parsed.objectId].append(parsed);
    }
    for (const auto &action : actions.zone_ability_actions()) {
        const quint32 oid = action.object_id();
        const int abilityIndex = static_cast<int>(action.ability_index());
        if (oid == 0 || abilityIndex < 0 || !action.has_ability()) {
            continue;
        }
        state->zoneAbilitySourceByOid.insert(oid, action.source_zone());
        state->abilitySourceGenerationByOid.insert(oid, action.zone_change_generation());
        state->zoneAbilityIndicesByOid[oid].insert(abilityIndex);
        if (action.source_zone() == ruled::v1::ABILITY_SOURCE_ZONE_HAND && action.has_hand_index()) {
            state->handAbilityOidBySlot.insert(static_cast<int>(action.hand_index()), oid);
        }
        const auto &ability = action.ability();
        auto &texts = state->engineOidToActivatedAbilityTexts[oid];
        auto &manaCosts = state->engineOidToActivatedAbilityManaCosts[oid];
        auto &manaProduced = state->engineOidToActivatedAbilityManaProduced[oid];
        auto &costLabels = state->engineOidToActivatedAbilityCostLabels[oid];
        auto &activatable = state->engineOidToActivatedAbilityActivatable[oid];
        while (texts.size() <= abilityIndex) {
            texts.append(QString{});
            manaCosts.append(QString{});
            manaProduced.append(QString{});
            costLabels.append(QString{});
            activatable.append(false);
        }
        texts[abilityIndex] = QString::fromStdString(ability.text());
        manaCosts[abilityIndex] = QString::fromStdString(ability.mana_cost());
        manaProduced[abilityIndex] = QString::fromStdString(ability.mana_produced());
        costLabels[abilityIndex] = QString::fromStdString(ability.cost_label());
        activatable[abilityIndex] = ability.activatable();
    }

    state->openingUiKind = RuledOpeningUiKind::None;
    state->openingPickSeatIds.clear();
    state->openingBottomSelectedIndices.clear();
    if (!state->handActionSet(ruled::v1::HAND_ACTION_OPENING_BOTTOM).handIndices.isEmpty()) {
        state->openingUiKind = RuledOpeningUiKind::BottomLibrary;
    } else {
        for (const auto &l : actions.labels()) {
            if (QString::fromStdString(l) == QLatin1String("Keep opening hand (opening)")) {
                state->openingUiKind = RuledOpeningUiKind::MulliganChoice;
                break;
            }
        }
    }
    if (state->openingUiKind == RuledOpeningUiKind::None) {
        for (const auto &l : actions.labels()) {
            const QString qs = QString::fromStdString(l);
            if (qs == QLatin1String("You start (opening pick)") ||
                qs == QLatin1String("Opponent starts (opening pick)")) {
                state->openingUiKind = RuledOpeningUiKind::ChooseFirst;
                state->openingMulliganCount = 0;
                break;
            }
        }
    }

    ctx.promptFeed += tr("Legal actions:\n");
    for (const auto &l : actions.labels()) {
        ctx.promptFeed += QStringLiteral(" — %1\n").arg(QString::fromStdString(l));
    }
    // CR 605 float courtesy: surface the engine's undoable-mana count so the client can offer /
    // retract the Undo affordance authoritatively.
    emit state->undoableManaAbilitiesChanged(static_cast<int>(actions.undoable_mana_abilities()));
    // CR 508.1d / 509.1c: engine-reported must-attack / must-block sets that gate the combat
    // confirm controls (see RuledClientState::combatDeclarationSatisfied).
    state->requiredAttackerOids.clear();
    for (const quint32 oid : actions.required_attacker_ids()) {
        state->requiredAttackerOids.insert(oid);
    }
    state->requiredBlockerOids.clear();
    for (const quint32 oid : actions.required_blocker_ids()) {
        state->requiredBlockerOids.insert(oid);
    }
    state->selectableAttackerOids.clear();
    for (const quint32 oid : actions.selectable_attacker_ids()) {
        state->selectableAttackerOids.insert(oid);
    }
    state->legalAttackAssignmentsByAttacker.clear();
    for (const auto &assignment : actions.legal_attack_assignments()) {
        state->legalAttackAssignmentsByAttacker[assignment.attacker_object_id()].append(assignment);
    }
    state->legalBlockAttackerOidsByBlocker.clear();
    for (const auto &pair : actions.legal_block_pairs()) {
        state->legalBlockAttackerOidsByBlocker[pair.blocker_id()].insert(pair.attacker_id());
    }
}

void RuledEventDispatcher::applyNoLegalActions()
{
    state->clearHandActions();
    state->openingBottomSelectedIndices.clear();
    state->openingPickSeatIds.clear();
    state->openingUiKind = RuledOpeningUiKind::None;
    state->permanentActionsByOid.clear();
    // NB: do NOT clear the required or selectable combat sets here. Servatrice-synthesized
    // combat preview batches (AttackersPreview / BlockersPreview, emitted while the local player
    // stages attackers/blocks) carry no legal_by_player entry and land in this branch. The
    // combat sets are engine-authoritative and only change when a real engine batch (with
    // legal_by_player) arrives, so they must survive preview echoes — otherwise deselecting a
    // staged required creature couldn't re-disable OK and legal creatures would become inert.
    emit state->undoableManaAbilitiesChanged(0);
}

// ---------------------------------------------------------------------------------------
// Batch epilogue
// ---------------------------------------------------------------------------------------

void RuledEventDispatcher::finishBatch(BatchContext &ctx)
{
    if (ctx.reconcilePublicReveal && !ctx.publicRevealSeen) {
        state->clearPublicReveal();
    }
    state->pruneCleanupDiscardSelectionAndEmitUi();
    emit state->legalActionsChanged();
    if (ctx.stackTrackingDirty) {
        emit state->stackHasItemsChanged(!state->stackOidOrder.isEmpty());
        emit state->stackOrderChanged(state->stackOidOrder);
    }
    emit state->engineTimeline(ctx.timeline);
    emit state->enginePromptFeed(ctx.promptFeed);
    emit state->openingUiChanged();
    if (ctx.battlefieldMapDirty) {
        emit state->battlefieldMapUpdated();
    }
    if (ctx.combatStateDirty) {
        emit state->combatStateChanged();
    }
    for (const int playerId : ctx.restrictedManaDirtyPlayers) {
        emit state->restrictedManaChanged(playerId);
    }
    // One emit per batch from the settled state (CR 603.3b). A batch that places the picked trigger
    // and immediately offers the rest nets out to "still ordering, here is what's left", so the
    // popup is updated rather than closed and reopened.
    if (ctx.triggerOrderDirty) {
        emit state->triggerOrderUiChanged(state->hasPendingTriggerOrder(), state->triggerOrderCandidates());
    }
    // Which graveyards need to be open: a pending trigger's targets (Gravedigger ETB) unioned
    // with any pending cast's. `validTargetsByAbility` and the graveyard OID map are both
    // populated in this same batch, so recompute after applying it.
    state->emitGraveyardTargetsNeeded();
    // Defer so stack window / zone views finish layout before we resolve CardItem positions.
    host->scheduleSpellTargetArrowSync();
}
