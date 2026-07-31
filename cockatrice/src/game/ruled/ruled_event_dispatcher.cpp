#include "ruled_event_dispatcher.h"

#include "ruled_client_host.h"
#include "ruled_client_state.h"

#include <algorithm>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

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
    for (const quint32 oid : src.valid_permanent_ids()) {
        data.validPermanentIds.insert(oid);
    }
    for (const quint32 oid : src.valid_stack_ids()) {
        data.validStackIds.insert(oid);
    }
    for (const quint32 oid : src.valid_graveyard_ids()) {
        data.validGraveyardIds.insert(oid);
    }
    data.canTargetSelf = src.can_target_self();
    data.canTargetOpponent = src.can_target_opponent();
    data.maxTargets = static_cast<int>(src.max_targets());
    data.fixedDamage = static_cast<int>(src.fixed_damage());
    data.isDamageTargets = src.is_damage_targets();
    data.extraManaPerTarget = static_cast<int>(src.extra_mana_per_target());
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
        set.faceOptionsByIndex[handIndex].append({faceIndex, cardName});
        if (action.needs_target()) {
            set.needsTargetIndices.insert(handIndex);
        }
        if (action.modes_size() > 0) {
            set.modalMinModesByCastKey.insert(castKey, static_cast<int>(action.min_modes()));
            set.modalMaxModesByCastKey.insert(castKey, static_cast<int>(action.max_modes()));
            QVector<RuledModalSpellOption> modes;
            modes.reserve(action.modes_size());
            for (const auto &mode : action.modes()) {
                modes.append({static_cast<int>(mode.mode_index()), QString::fromStdString(mode.label()),
                              mode.selectable(), mode.needs_target(),
                              mode.has_targets() ? parseSpellTargets(mode.targets())
                                                 : RuledClientState::SpellTargetData{}});
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
    resetPerBatchLegalActions();
    ruled::v1::RuledEventBatch batch;
    if (!batch.ParseFromString(payload)) {
        return false;
    }
    processBatch(batch);
    return true;
}

void RuledEventDispatcher::resetPerBatchLegalActions()
{
    state->clearHandActions();
    state->openingBottomSelectedIndices.clear();
    state->openingPickSeatIds.clear();
    state->openingUiKind = RuledOpeningUiKind::None;
    state->ownedCardToEngineHandSlot.clear();
}

void RuledEventDispatcher::processBatch(const ruled::v1::RuledEventBatch &batch)
{
    BatchContext ctx;

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
        if (e.has_trigger_needs_target()) {
            applyTriggerNeedsTarget(e.trigger_needs_target(), ctx);
        }
        if (e.has_resolution_choice_required()) {
            applyResolutionChoiceRequired(e.resolution_choice_required(), ctx);
        }
        if (e.has_battlefield_object_map()) {
            applyBattlefieldObjectMap(e.battlefield_object_map(), ctx);
        }
        if (e.has_hand_slot_map()) {
            applyHandSlotMap(e.hand_slot_map());
        }
        if (e.has_graveyard_object_map()) {
            applyGraveyardObjectMap(e.graveyard_object_map());
        }
        if (e.has_zone_view()) {
            applyZoneView(e.zone_view(), ctx);
        }
        if (e.has_attackers_declared()) {
            applyAttackersDeclared(e.attackers_declared(), ctx);
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
    }

    const auto lit = batch.legal_by_player().find(host->localPlayerId());
    if (lit != batch.legal_by_player().end()) {
        applyLegalActions(lit->second, ctx);
    } else {
        applyNoLegalActions();
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
    state->pendingBlocks.clear();
    state->remoteBlockPreviewPairs.clear();
    state->remoteAttackerPreviewOids.clear();
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
    QVector<quint32> tlist;
    tlist.reserve(sp.targets_size());
    for (int ti = 0; ti < sp.targets_size(); ++ti) {
        tlist.append(static_cast<quint32>(sp.targets(ti).object_id()));
    }
    state->stackTargetsByStackOid.insert(sp.object_id(), tlist);
    // Overlay the annotation on the stack card: an ability's text, an X value for an X spell
    // ("X = N", CR 107.3), or the cast face of a multi-face spell (e.g. "Fire" / "Ice").
    if (!sp.ability_annotation().empty()) {
        state->stackAnnotationByOid.insert(sp.object_id(), QString::fromStdString(sp.ability_annotation()));
        // Abilities (no card_id) and spell copies (is_copy) both lack a physical CardItem on the
        // stack and need a synthetic one. A normal spell (non-empty card_id, not a copy) already
        // has a real card.
        if (sp.card_id().empty()) {
            // A triggered ability was just placed on the stack — the pending trigger target has
            // been chosen and is no longer pending.
            state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerTarget);
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
    // Countered spells leave the engine stack without their own StackResolved; remove this
    // spell's stack targets (e.g. the countered object id) so stackOidOrder matches the real
    // stack (pass button + stack window).
    const QVector<quint32> spellTargets = state->stackTargetsByStackOid.value(rid);
    state->stackOidOrder.removeOne(rid);
    state->stackTargetsByStackOid.remove(rid);
    state->stackAnnotationByOid.remove(rid);
    state->stackSourceOidByStackOid.remove(rid);
    host->removeSyntheticStackCard(rid);
    for (quint32 t : spellTargets) {
        state->stackOidOrder.removeOne(t);
    }
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
        choice.kind = RuledClientState::ChoiceKind::TriggerTarget;
        choice.promptText = abilityText;
        state->setPendingChoice(std::move(choice));
        ctx.promptFeed += QStringLiteral("Choose a target for “%1”.\n").arg(abilityText);
    } else {
        state->clearPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerTarget);
    }
    emit state->triggerNeedsTarget(abilityText);
}

void RuledEventDispatcher::applyResolutionChoiceRequired(const ruled::v1::ResolutionChoiceRequired &rcr,
                                                         BatchContext &ctx)
{
    using PendingChoice = RuledClientState::RuledPendingChoice;
    using ChoiceKind = RuledClientState::ChoiceKind;

    // Tier-3 custom resolution paused for a player choice (CR 608).
    ctx.promptFeed += QString::fromStdString(rcr.prompt_text()) + QStringLiteral("\n");
    // Drop any stale pick from a previous resolution step. Deliberately not a full
    // clearPendingChoice(): a parked trigger/copy/legend choice belongs to a different flow.
    state->clearPendingChoiceOfKind(ChoiceKind::ResolutionPick);
    if (static_cast<int>(rcr.deciding_player_id()) != host->localPlayerId() || rcr.candidate_object_ids_size() <= 0) {
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_TARGET_OBJECTS ||
        rcr.choice_kind() == ruled::v1::CHOICE_KIND_LEGEND_KEEP) {
        // Click-to-select on the battlefield rather than a modal list:
        //   TARGET_OBJECTS — CR 707.10c, the controller of a spell copy may redirect its targets.
        //   LEGEND_KEEP    — CR 704.5j, the controller clicks the legend to KEEP; the rest are
        //                    sacrificed.
        PendingChoice choice;
        choice.kind = rcr.choice_kind() == ruled::v1::CHOICE_KIND_LEGEND_KEEP ? ChoiceKind::LegendKeep
                                                                             : ChoiceKind::CopyTarget;
        choice.promptText = QString::fromStdString(rcr.prompt_text());
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

    if ((rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH ||
         rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_TOP) &&
        rcr.candidate_server_card_ids_size() == rcr.candidate_names_size() && rcr.candidate_names_size() > 0) {
        // LibrarySearch or LibraryTop with server card ids: deck zone-view pick. Both show cards
        // out of the local library, so they share the popup — only the title differs.
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
        pick.viewTitle =
            rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_TOP ? tr("Scry") : tr("Search your library");
        QVector<int> libScids;
        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
            const quint32 oid = (i < rcr.candidate_object_ids_size()) ? rcr.candidate_object_ids(i) : 0;
            const int scid = rcr.candidate_server_card_ids(i);
            const QString name = QString::fromStdString(rcr.candidate_names(i));
            if (scid >= 0) {
                pick.serverCardIdToOid.insert(scid, oid);
                pick.serverCardIdToName.insert(scid, name);
            }
            pick.candidateNames.append(name);
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
        // RevealedCards or PrivateRevealedHand with server card ids: zone popup pick. The deciding
        // player chooses from the revealed cards (OpponentHand = a target player's hand shown only
        // to the caster; the relay redacted it from everyone else).
        PendingChoice pick;
        pick.kind = ChoiceKind::ResolutionPick;
        pick.min = static_cast<int>(rcr.min());
        pick.max = static_cast<int>(rcr.max());
        pick.promptText = QString::fromStdString(rcr.prompt_text());
        pick.pickZone = PickZone::Revealed;
        // Both kinds render identically, but they are not the same thing: REVEALED was shown to
        // the whole table, OPPONENT_HAND is a hand only the decider may look at (CR 701.7). The
        // proto does not carry whose hand it is, so the title stays seat-agnostic — if a future
        // card picks a player without targeting one, that owner id has to come across the wire.
        pick.viewTitle = rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND
                             ? tr("Target player's hand")
                             : tr("Revealed cards");
        QStringList names;
        QVector<int> scids;
        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
            const quint32 oid = (i < rcr.candidate_object_ids_size()) ? rcr.candidate_object_ids(i) : 0;
            const int scid = rcr.candidate_server_card_ids(i);
            if (scid >= 0) {
                pick.serverCardIdToOid.insert(scid, oid);
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
        emit state->revealedPickChanged(true, names, scids, required, maximum);
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
    for (int hi = 0; hi < map.entries_size(); ++hi) {
        const auto &ent = map.entries(hi);
        state->ownedCardToEngineHandSlot.insert(
            RuledClientState::makeOwnedCardKey(ent.player_id(), ent.server_card_id()),
            static_cast<int>(ent.hand_index()));
    }
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

void RuledEventDispatcher::applyZoneView(const ruled::v1::ZoneViewSync &view, BatchContext &ctx)
{
    state->engineOidMarkedDamage.clear();
    state->engineOidBattlefieldPower.clear();
    state->engineOidBattlefieldToughness.clear();
    state->engineOidToActivatedAbilityTexts.clear();
    state->engineOidToActivatedAbilityManaCosts.clear();
    state->engineOidToActivatedAbilityManaProduced.clear();
    bool anyFirstStrikePending = false;
    for (const auto &p : view.per_player()) {
        if (p.first_strike_step_pending()) {
            anyFirstStrikePending = true;
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
            QStringList texts;
            QStringList manaCosts;
            QStringList manaProduced;
            QStringList costLabels;
            for (const auto &ability : battlefieldObject.activated_abilities()) {
                texts.append(QString::fromStdString(ability.text()));
                manaCosts.append(QString::fromStdString(ability.mana_cost()));
                manaProduced.append(QString::fromStdString(ability.mana_produced()));
                costLabels.append(QString::fromStdString(ability.cost_label()));
            }
            if (!texts.isEmpty()) {
                state->engineOidToActivatedAbilityTexts.insert(oid, texts);
                state->engineOidToActivatedAbilityManaCosts.insert(oid, manaCosts);
                state->engineOidToActivatedAbilityManaProduced.insert(oid, manaProduced);
                state->engineOidToActivatedAbilityCostLabels.insert(oid, costLabels);
            }
            state->engineOidBattlefieldPower.insert(oid, static_cast<int>(battlefieldObject.power()));
            state->engineOidBattlefieldToughness.insert(oid, static_cast<int>(battlefieldObject.toughness()));
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
    for (const auto oid : ad.attacker_object_ids()) {
        state->currentAttackerOids.insert(oid);
    }
    // Active player's pending picks are now committed; clear them.
    state->pendingAttackerOids.clear();
    state->remoteAttackerPreviewOids.clear();
    state->attackersSubmittedThisStep = true;
    ctx.combatStateDirty = true;
}

void RuledEventDispatcher::applyAttackersPreview(const ruled::v1::AttackersPreview &ap, BatchContext &ctx)
{
    if (static_cast<int>(ap.declaring_player_id()) != host->localPlayerId()) {
        state->remoteAttackerPreviewOids.clear();
        for (int ai = 0; ai < ap.attacker_object_ids_size(); ++ai) {
            state->remoteAttackerPreviewOids.insert(static_cast<quint32>(ap.attacker_object_ids(ai)));
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

// ---------------------------------------------------------------------------------------
// Legal actions
// ---------------------------------------------------------------------------------------

void RuledEventDispatcher::applyLegalActions(const ruled::v1::LegalActions &actions, BatchContext &ctx)
{
    state->handActions = copyHandActions(actions);

    state->validTargetsByHandSlot.clear();
    for (const auto &entry : actions.valid_targets_by_hand_slot()) {
        // Key is the engine's composite (hand slot << 8 | face index); stored verbatim and
        // matched by RuledClientState::spellTargetKey().
        state->validTargetsByHandSlot.insert(static_cast<int>(entry.first), parseSpellTargets(entry.second));
    }
    state->validTargetsByAbility.clear();
    for (const auto &entry : actions.valid_targets_by_ability()) {
        state->validTargetsByAbility.insert(static_cast<quint64>(entry.first), parseSpellTargets(entry.second));
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
}

void RuledEventDispatcher::applyNoLegalActions()
{
    state->clearHandActions();
    state->openingBottomSelectedIndices.clear();
    state->openingPickSeatIds.clear();
    state->openingUiKind = RuledOpeningUiKind::None;
    // NB: do NOT clear requiredAttackerOids / requiredBlockerOids here. Servatrice-synthesized
    // combat preview batches (AttackersPreview / BlockersPreview, emitted while the local player
    // stages attackers/blocks) carry no legal_by_player entry and land in this branch. The
    // must-attack / must-block sets are engine-authoritative and only change when a real engine
    // batch (with legal_by_player) arrives, so they must survive preview echoes — otherwise
    // deselecting a staged required creature couldn't re-disable OK.
    emit state->undoableManaAbilitiesChanged(0);
}

// ---------------------------------------------------------------------------------------
// Batch epilogue
// ---------------------------------------------------------------------------------------

void RuledEventDispatcher::finishBatch(BatchContext &ctx)
{
    state->pruneCleanupDiscardSelectionAndEmitUi();
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
    // Which graveyards need to be open: a pending trigger's targets (Gravedigger ETB) unioned
    // with any pending cast's. `validTargetsByAbility` and the graveyard OID map are both
    // populated in this same batch, so recompute after applying it.
    state->emitGraveyardTargetsNeeded();
    // Defer so stack window / zone views finish layout before we resolve CardItem positions.
    host->scheduleSpellTargetArrowSync();
}
