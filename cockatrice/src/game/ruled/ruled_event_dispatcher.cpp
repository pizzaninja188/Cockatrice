#include "ruled_event_dispatcher.h"

#include "ruled_client_host.h"
#include "ruled_client_state.h"

#include <QRegularExpression>
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

/// How a label spec's optional third capture group is interpreted.
enum class ThirdCapture
{
    Unused,     ///< the pattern has no third group
    FaceIndex,  ///< CR 712: ", face N" — one entry per playable face of the slot
    NeedsTarget ///< ", target" — the action needs a cast-time target
};

/// One engine legal-action label form. Capture 1 is the card name, capture 2 the engine hand slot,
/// capture 3 whatever `third` says. A new hand mechanic is one more row here plus a
/// RuledHandActionKind value — nothing else in the client parses labels.
struct HandActionLabelSpec
{
    RuledHandActionKind kind;
    QRegularExpression pattern;
    ThirdCapture third;
};

const QVector<HandActionLabelSpec> &handActionLabelSpecs()
{
    static const QVector<HandActionLabelSpec> specs{
        // CR 712: multi-face (MDFC) land labels append ", face N"; single-face lands omit it (face 0).
        {RuledHandActionKind::PlayLand,
         QRegularExpression(QStringLiteral(R"(^Play land (.*) \(hand idx (\d+)(?:, face (\d+))?\)$)")),
         ThirdCapture::FaceIndex},
        // Optional ", target" suffix is emitted by the engine when the spell needs a cast-time target.
        {RuledHandActionKind::CastSpell,
         QRegularExpression(QStringLiteral(R"(^Cast (.*) \(hand idx (\d+)(, target)?\)$)")),
         ThirdCapture::NeedsTarget},
        {RuledHandActionKind::CleanupDiscard,
         QRegularExpression(QStringLiteral(R"(^Discard (.*) \(cleanup, hand idx (\d+)\)$)")), ThirdCapture::Unused},
        {RuledHandActionKind::OpeningBottom,
         QRegularExpression(QStringLiteral(R"(^Put (.+) on bottom \(opening, hand idx (\d+)\)$)")),
         ThirdCapture::Unused},
    };
    return specs;
}

/// The one legal-action label parser: every hand-action kind at once, in a single pass over labels.
QHash<RuledHandActionKind, RuledHandActionSet> parseHandActions(const ruled::v1::LegalActions &actions)
{
    QHash<RuledHandActionKind, RuledHandActionSet> parsed;
    for (const auto &label : actions.labels()) {
        const QString text = QString::fromStdString(label);
        for (const HandActionLabelSpec &spec : handActionLabelSpecs()) {
            const QRegularExpressionMatch match = spec.pattern.match(text);
            if (!match.hasMatch()) {
                continue;
            }
            bool ok = false;
            const int handIndex = match.captured(2).toInt(&ok);
            if (!ok) {
                break; // label matched this kind; a malformed slot is not another kind's label
            }
            RuledHandActionSet &set = parsed[spec.kind];
            set.handIndices.insert(handIndex);
            set.indicesByCardName.insert(match.captured(1), handIndex);
            if (spec.third == ThirdCapture::FaceIndex) {
                bool faceOk = false;
                const int faceIndex = match.captured(3).toInt(&faceOk);
                set.faceOptionsByIndex[handIndex].append({faceOk ? faceIndex : 0, match.captured(1)});
            } else if (spec.third == ThirdCapture::NeedsTarget && !match.captured(3).isEmpty()) {
                set.needsTargetIndices.insert(handIndex);
            }
            break;
        }
    }
    return parsed;
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
            state->hasPendingTrigger = false;
            // Record the source permanent so the targeting arrow starts from it.
            if (state->pendingTriggerSourceOid != 0) {
                state->stackSourceOidByStackOid.insert(sp.object_id(), state->pendingTriggerSourceOid);
            }
            host->createSyntheticStackCard(sp.object_id(), QString::fromStdString(sp.description()),
                                           state->pendingTriggerControllerPlayerId, {});
        } else if (sp.is_copy()) {
            // The copy is being placed on the stack — any pending copy-target choice has been
            // accepted by the engine.
            state->pendingCopyTargetChoice = {};
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
    state->pendingTriggerSourceOid = tnt.source_permanent_id();
    state->pendingTriggerAbilityIndex = tnt.ability_index();
    state->pendingTriggerAbilityText = QString::fromStdString(tnt.ability_text());
    state->pendingTriggerControllerPlayerId = static_cast<int>(tnt.controller_player_id());
    // Only the controller sees hasPendingTrigger = true so that only they can send
    // ChooseTriggerTarget commands.
    state->hasPendingTrigger = (state->pendingTriggerControllerPlayerId == host->localPlayerId());
    if (state->hasPendingTrigger) {
        ctx.promptFeed += QStringLiteral("Choose a target for: %1\n").arg(state->pendingTriggerAbilityText);
    }
    emit state->triggerNeedsTarget(state->pendingTriggerAbilityText);
}

void RuledEventDispatcher::applyResolutionChoiceRequired(const ruled::v1::ResolutionChoiceRequired &rcr,
                                                         BatchContext &ctx)
{
    // Tier-3 custom resolution paused for a player choice (CR 608).
    ctx.promptFeed += QString::fromStdString(rcr.prompt_text()) + QStringLiteral("\n");
    // Clear any stale hand-pick state from a previous resolution step.
    if (state->resolutionHandPick.has_value() && state->resolutionHandPick->pickZone == PickZone::Revealed) {
        emit state->revealedPickChanged(false, {}, {}, 0, 0);
    }
    state->resolutionHandPick.reset();
    if (static_cast<int>(rcr.deciding_player_id()) != host->localPlayerId() || rcr.candidate_object_ids_size() <= 0) {
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_TARGET_OBJECTS) {
        // Target objects (CR 707.10c copy retarget): click-to-target rather than a modal list.
        state->pendingCopyTargetChoice.valid = true;
        state->pendingCopyTargetChoice.promptText = QString::fromStdString(rcr.prompt_text());
        state->pendingCopyTargetChoice.candidateOids.clear();
        for (int i = 0; i < rcr.candidate_object_ids_size(); ++i) {
            state->pendingCopyTargetChoice.candidateOids.append(rcr.candidate_object_ids(i));
        }
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LEGEND_KEEP) {
        // Legend rule keep (CR 704.5j): the controller clicks the legendary permanent to KEEP
        // directly on the battlefield; the rest are sacrificed. Click-to-select mode, like copy
        // retarget, instead of a modal list dialog.
        state->pendingLegendKeepChoice.valid = true;
        state->pendingLegendKeepChoice.promptText = QString::fromStdString(rcr.prompt_text());
        state->pendingLegendKeepChoice.candidateOids.clear();
        for (int i = 0; i < rcr.candidate_object_ids_size(); ++i) {
            state->pendingLegendKeepChoice.candidateOids.append(rcr.candidate_object_ids(i));
        }
        emit state->combatStateChanged();
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS &&
        rcr.candidate_server_card_ids_size() == rcr.candidate_object_ids_size()) {
        // HandCards with server card ids: use the hand-click UI.
        RuledClientState::ResolutionHandPick pick;
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
        state->resolutionHandPick = std::move(pick);
        emit state->resolutionHandPickUiChanged(state->resolutionHandPick->min, 0);
        emit state->combatStateChanged();
        return;
    }

    if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH &&
        rcr.candidate_server_card_ids_size() == rcr.candidate_names_size() && rcr.candidate_names_size() > 0) {
        // LibrarySearch with server card ids: deck zone-view pick.
        // unique_names is always true for Gifts Ungiven step 1.
        RuledClientState::ResolutionHandPick pick;
        pick.min = static_cast<int>(rcr.min());
        pick.max = static_cast<int>(rcr.max());
        pick.uniqueNames = rcr.unique_names();
        pick.promptText = QString::fromStdString(rcr.prompt_text());
        pick.pickZone = PickZone::Deck;
        pick.viewTitle = tr("Search your library");
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
        state->resolutionHandPick = std::move(pick);
        emit state->resolutionHandPickUiChanged(state->resolutionHandPick->min, 0);
        emit state->librarySearchPickStarted(state->resolutionHandPick->candidateNames, libScids);
        emit state->combatStateChanged();
        return;
    }

    if ((rcr.choice_kind() == ruled::v1::CHOICE_KIND_REVEALED ||
         rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND) &&
        rcr.candidate_server_card_ids_size() == rcr.candidate_names_size() && rcr.candidate_names_size() > 0) {
        // RevealedCards or PrivateRevealedHand with server card ids: zone popup pick. The deciding
        // player chooses from the revealed cards (OpponentHand = a target player's hand shown only
        // to the caster; the relay redacted it from everyone else).
        RuledClientState::ResolutionHandPick pick;
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
        state->resolutionHandPick = std::move(pick);
        emit state->resolutionHandPickUiChanged(state->resolutionHandPick->min, 0);
        emit state->revealedPickChanged(true, names, scids, state->resolutionHandPick->min,
                                        state->resolutionHandPick->max);
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
        state->engineOidHaste.insert(entry.engine_object_id(), entry.haste());
        state->engineOidTrample.insert(entry.engine_object_id(), entry.trample());
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
    state->graveyardEngineOidToServerCardId.clear();
    for (int gi = 0; gi < map.entries_size(); ++gi) {
        const auto &ent = map.entries(gi);
        state->graveyardEngineOidToServerCardId.insert(static_cast<quint32>(ent.engine_object_id()),
                                                       ent.server_card_id());
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
        const int count = std::min(p.battlefield_object_id_size(), p.battlefield_damage_size());
        for (int zdi = 0; zdi < count; ++zdi) {
            const quint32 oid = p.battlefield_object_id(zdi);
            const int damage = static_cast<int>(p.battlefield_damage(zdi));
            if (oid != 0 && damage > 0) {
                state->engineOidMarkedDamage.insert(oid, damage);
            }
        }
        // Parse activated ability texts and mana costs (pipe-delimited per permanent).
        const int nAbil = std::min(p.battlefield_object_id_size(), p.battlefield_activated_ability_texts_size());
        for (int ai = 0; ai < nAbil; ++ai) {
            const quint32 oid = p.battlefield_object_id(ai);
            if (oid == 0) {
                continue;
            }
            const QString textsStr = QString::fromStdString(p.battlefield_activated_ability_texts(ai));
            if (!textsStr.isEmpty()) {
                state->engineOidToActivatedAbilityTexts.insert(oid, textsStr.split(QChar('|'), Qt::SkipEmptyParts));
            }
            if (ai < p.battlefield_activated_ability_mana_costs_size()) {
                // Split on '|'; an empty entry means no mana cost for that ability.
                state->engineOidToActivatedAbilityManaCosts.insert(
                    oid, QString::fromStdString(p.battlefield_activated_ability_mana_costs(ai)).split(QChar('|')));
            }
            if (ai < p.battlefield_activated_ability_mana_produced_size()) {
                // Split on '|'; empty entry = non-mana ability (CR 605).
                state->engineOidToActivatedAbilityManaProduced.insert(
                    oid, QString::fromStdString(p.battlefield_activated_ability_mana_produced(ai)).split(QChar('|')));
            }
            if (ai < p.battlefield_activated_ability_cost_labels_size()) {
                // Split on '|'; each entry is a display cost string.
                state->engineOidToActivatedAbilityCostLabels.insert(
                    oid, QString::fromStdString(p.battlefield_activated_ability_cost_labels(ai)).split(QChar('|')));
            }
        }
        const int nPow = std::min(p.battlefield_object_id_size(), p.battlefield_power_size());
        for (int pi = 0; pi < nPow; ++pi) {
            const quint32 oid = p.battlefield_object_id(pi);
            if (oid != 0) {
                state->engineOidBattlefieldPower.insert(oid, static_cast<int>(p.battlefield_power(pi)));
            }
        }
        const int nTough = std::min(p.battlefield_object_id_size(), p.battlefield_toughness_size());
        for (int ti = 0; ti < nTough; ++ti) {
            const quint32 oid = p.battlefield_object_id(ti);
            if (oid != 0) {
                state->engineOidBattlefieldToughness.insert(oid, static_cast<int>(p.battlefield_toughness(ti)));
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
    state->handActions = parseHandActions(actions);

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
    if (!state->handActionSet(RuledHandActionKind::OpeningBottom).handIndices.isEmpty()) {
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
    // Emit graveyard-open signal for triggers whose valid targets are in the graveyard (e.g.
    // Gravedigger ETB). validTargetsByAbility is populated in this same batch.
    const quint64 abilityKey = RuledClientState::abilityTargetKey(state->pendingTriggerSourceOid,
                                                                  static_cast<int>(state->pendingTriggerAbilityIndex));
    const bool graveyardNeeded =
        state->hasPendingTrigger && !state->validTargetsByAbility.value(abilityKey).validGraveyardIds.isEmpty();
    emit state->triggerGraveyardNeedsTarget(graveyardNeeded);
    // Defer so stack window / zone views finish layout before we resolve CardItem positions.
    host->scheduleSpellTargetArrowSync();
}
