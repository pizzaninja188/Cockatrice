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

    bool spellNeedsNewTarget = false;
    auto &spell = actions->pendingRuledSpellCast;
    if (spell.valid && !spell.selectedModes.isEmpty()) {
        for (int i = 0; i < spell.selectedModes.size(); ++i) {
            auto &mode = spell.selectedModes[i];
            if (!mode.needsTarget || !mode.selectedTargetOids.isEmpty()) {
                continue;
            }
            spell.activeModePosition = i;
            spell.selectedTargetOids = mode.selectedTargetOids;
            spell.selectedTargetDamages = mode.selectedTargetDamages;
            spell.targetDamageAllocations.clear();
            if (const auto live =
                    state->modalSpellTargetData(spell.handIndex, spell.faceIndex, mode.modeIndex, spell.source)) {
                mode.targets = *live;
                spell.isDamageTargets = live->isDamageTargets;
                spell.damageDividedEvenly = live->damageDividedEvenly;
                spell.maxTargets = live->maxTargets;
                spell.fixedDamage = live->fixedDamage;
                spell.extraManaPerTarget = live->extraManaPerTarget;
            }
            spellNeedsNewTarget = true;
            break;
        }
    } else if (spell.valid && spellHadTargets && spell.selectedTargetOids.isEmpty()) {
        spellNeedsNewTarget = true;
    }

    if (spellNeedsNewTarget) {
        spell.waitingForTarget = true;
        spell.inDamageAllocationMode = false;
        emit actions->ruledSpellTargetingChanged(true, spell.cardName);
        state->emitLocalLog(actions->tr("A selected target is no longer legal. Choose a new target for %1.")
                                .arg(spell.cardName));
    }
    auto &ability = actions->pendingActivatedAbility;
    if (abilityHadTarget && ability.valid && ability.selectedTargetOid == 0) {
        ability.waitingForTarget = true;
        ability.waitingForMana = false;
        emit actions->ruledActivatedAbilityTargetPendingChanged(true, ability.abilityText);
        state->emitLocalLog(actions->tr("The selected target is no longer legal. Choose a new target for: %1")
                                .arg(ability.abilityText));
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
