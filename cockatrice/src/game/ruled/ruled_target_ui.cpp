#include "ruled_pending_cast.h"

#include "../abstract_game.h"
#include "../board/card_item.h"
#include "../game_event_handler.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "../player/player_info.h"
#include "../zones/logic/card_zone_logic.h"
#include "ruled_actions.h"

#include <libcockatrice/utility/zone_names.h>

void RuledTargetUi::ensureRefreshConnection(PlayerActions *actions)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return;
    }
    QObject::connect(state, &RuledClientState::legalActionsChanged, actions,
                     &PlayerActions::reconcilePendingRuledTargetSelections, Qt::UniqueConnection);
}

void RuledTargetUi::reconcile(PlayerActions *actions)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return;
    }
    const int localPlayerId = actions->player->getPlayerInfo()->getId();
    const bool spellHadTargets = !actions->pendingRuledSpellCast.selectedTargetOids.isEmpty();
    const bool abilityHadTarget = actions->pendingActivatedAbility.selectedTargetOid != 0;
    if (!reconcileRuledPendingTargets(actions->pendingRuledSpellCast, actions->pendingActivatedAbility, *state,
                                      localPlayerId)) {
        return;
    }

    auto &spell = actions->pendingRuledSpellCast;
    if (spell.valid && spell.waitingForTarget) {
        spell.inDamageAllocationMode = false;
        const auto group = currentRuledSpellTargetGroup(spell, *state);
        const QString prompt = ruledPendingSpellTargetPrompt(spell, *state);
        emit actions->ruledSpellTargetingChanged(true, prompt);
        if (group.has_value()) {
            emit actions->ruledMultiTargetSelectionUpdated(spell.selectedTargetOids.size(), group->minTargets,
                                                           ruledTargetSelectionDisplayMaximum(*group));
        }
        if (spellHadTargets) {
            state->emitLocalLog(actions->tr("A selected target is no longer legal. %1").arg(prompt));
        }
    }
    auto &ability = actions->pendingActivatedAbility;
    if (abilityHadTarget && ability.valid && ability.selectedTargetOid == 0) {
        ability.waitingForTarget = true;
        ability.waitingForMana = false;
        const QString prompt = ruledPendingAbilityTargetPrompt(ability, *state);
        emit actions->ruledActivatedAbilityTargetPendingChanged(true, prompt);
        state->emitLocalLog(actions->tr("The selected target is no longer legal. %1").arg(prompt));
    }
    state->emitSpellTargetSelectionChanged();
    state->emitSpellDamageAllocationUiChanged();
}

RuledTargetClickEligibility RuledTargetUi::cardEligibility(const PlayerActions *actions, CardItem *card)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state || !RuledActions::isRuledGame(actions->player->getGame())) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    if (actions->pendingRuledSpellCast.valid && actions->pendingRuledSpellCast.waitingForCastCostObject) {
        if (!card || !card->getZone()) {
            return RuledTargetClickEligibility::Illegal;
        }
        const QString zoneName = card->getZone()->getName();
        if (zoneName == ZoneNames::HAND) {
            Player *const handPlayer = card->getZone()->getPlayer();
            if (handPlayer != actions->player || !handPlayer->getPlayerInfo()) {
                return RuledTargetClickEligibility::Illegal;
            }
            const int handSlot =
                state->engineHandSlotForServerCard(handPlayer->getPlayerInfo()->getId(), card->getId());
            return handSlot < 0
                       ? RuledTargetClickEligibility::Illegal
                       : ruledCastCostObjectEligibility(actions->pendingRuledSpellCast,
                                                        RuledCastCostCandidateKind::Hand,
                                                        static_cast<quint32>(handSlot));
        }
        if (zoneName == ZoneNames::TABLE) {
            const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
            const quint32 oid = state->engineOidForCardId(ownerPlayerId, card->getId());
            return oid == 0 ? RuledTargetClickEligibility::Illegal
                            : ruledCastCostObjectEligibility(actions->pendingRuledSpellCast,
                                                             RuledCastCostCandidateKind::Permanent, oid);
        }
        return RuledTargetClickEligibility::Illegal;
    }
    RuledTargetCandidateKind kind = RuledTargetCandidateKind::Battlefield;
    quint32 oid = 0;
    if (card && card->getZone()) {
        const QString zoneName = card->getZone()->getName();
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        if (zoneName == ZoneNames::TABLE) {
            kind = RuledTargetCandidateKind::Battlefield;
            oid = state->engineOidForCardId(ownerPlayerId, card->getId());
        } else if (zoneName == ZoneNames::STACK) {
            kind = RuledTargetCandidateKind::Stack;
            oid = state->engineOidForCardId(ownerPlayerId, card->getId());
        } else if (zoneName == ZoneNames::GRAVE) {
            kind = RuledTargetCandidateKind::Graveyard;
            oid = state->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
        }
    }
    const auto eligibility = ::ruledTargetClickEligibility(
        actions->pendingRuledSpellCast, actions->pendingActivatedAbility, *state, kind, oid,
        actions->player->getPlayerInfo()->getId());
    return oid == 0 && eligibility != RuledTargetClickEligibility::NotTargeting
               ? RuledTargetClickEligibility::Illegal
               : eligibility;
}

RuledTargetClickEligibility RuledTargetUi::playerEligibility(const PlayerActions *actions, Player *target)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state || !RuledActions::isRuledGame(actions->player->getGame())) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    const int targetPlayerId = target ? target->getPlayerInfo()->getId() : -1;
    const auto eligibility = ::ruledTargetClickEligibility(
        actions->pendingRuledSpellCast, actions->pendingActivatedAbility, *state, RuledTargetCandidateKind::Player,
        targetPlayerId >= 0 ? static_cast<quint32>(targetPlayerId) : 0,
        actions->player->getPlayerInfo()->getId());
    return targetPlayerId < 0 && eligibility != RuledTargetClickEligibility::NotTargeting
               ? RuledTargetClickEligibility::Illegal
               : eligibility;
}
