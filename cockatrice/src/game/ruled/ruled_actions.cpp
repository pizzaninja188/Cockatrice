#include "ruled_actions.h"

#include "../../interface/widgets/tabs/tab_game.h"
#include "../abstract_game.h"
#include "../board/arrow_target.h"
#include "../board/card_item.h"
#include "../board/card_list.h"
#include "../game_event_handler.h"
#include "../game_meta_info.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "../player/player_graphics_item.h"
#include "../player/player_info.h"
#include "../player/player_manager.h"
#include "../player/player_target.h"
#include "../zones/logic/card_zone_logic.h"
#include "../zones/logic/view_zone_logic.h"
#include "ruled_client_host.h"
#include "ruled_client_state.h"

#include <algorithm>
#include <libcockatrice/utility/zone_names.h>

namespace
{

/// Local player's PlayerActions, or nullptr when there is no seated local player.
PlayerActions *localPlayerActions(const AbstractGame *game)
{
    if (!game) {
        return nullptr;
    }
    PlayerManager *pm = const_cast<AbstractGame *>(game)->getPlayerManager();
    if (!pm) {
        return nullptr;
    }
    const int localId = pm->getLocalPlayerId();
    if (localId < 0) {
        return nullptr;
    }
    Player *local = pm->getPlayers().value(localId, nullptr);
    return local ? local->getPlayerActions() : nullptr;
}

/// Stack CardItem with this Server_Card.id in any player's stack zone (no zone-view preference).
CardItem *rawStackCardByServerId(AbstractGame *game, int serverCardId)
{
    if (!game || serverCardId < 0) {
        return nullptr;
    }
    for (Player *p : game->getPlayerManager()->getPlayers()) {
        if (!p) {
            continue;
        }
        if (CardItem *c =
                game->getCard(p->getPlayerInfo()->getId(), QString::fromLatin1(ZoneNames::STACK), serverCardId)) {
            return c;
        }
    }
    return nullptr;
}

} // namespace

namespace RuledActions
{

// ---------------------------------------------------------------------------------------
// Mode predicate
// ---------------------------------------------------------------------------------------

bool isRuledGame(const AbstractGame *game)
{
    if (!game) {
        return false;
    }
    GameMetaInfo *meta = const_cast<AbstractGame *>(game)->getGameMetaInfo();
    return meta && meta->proto().ruled_game();
}

bool isRuledGameForPlayer(const Player *player)
{
    return player && isRuledGame(const_cast<Player *>(player)->getGame());
}

bool isRuledGameForCard(const CardItem *card)
{
    if (!card || !card->getOwner()) {
        return false;
    }
    return isRuledGame(card->getOwner()->getGame());
}

RuledClientState *stateFor(const AbstractGame *game)
{
    if (!isRuledGame(game)) {
        return nullptr;
    }
    GameEventHandler *handler = const_cast<AbstractGame *>(game)->getGameEventHandler();
    return handler ? handler->ruled() : nullptr;
}

void updateGraveyardTargetHint(const Player *player, int handSlot, int faceIndex)
{
    if (!player) {
        return;
    }
    RuledClientState *state = stateFor(player->getGame());
    if (!state) {
        return;
    }
    QSet<quint32> oids;
    if (handSlot >= 0) {
        oids = state->validTargetsByHandSlot.value(RuledClientState::spellTargetKey(handSlot, faceIndex))
                   .validGraveyardIds;
    }
    state->setPendingCastGraveyardTargets(oids);
}

void sendRuledCommand(const AbstractGame *game, const ruled::v1::RuledCommand &command)
{
    if (!isRuledGame(game)) {
        return;
    }
    GameEventHandler *handler = const_cast<AbstractGame *>(game)->getGameEventHandler();
    if (!handler) {
        return;
    }
    // Through the host interface, where the method is public. GameEventHandler keeps its
    // RuledClientHost overrides private so the view model is the only thing that normally sends;
    // this is the one documented exception, rather than widening that class's interface.
    static_cast<RuledClientHost *>(handler)->sendRuledCommand(command);
}

void sendRuledCommandExpectingAck(const AbstractGame *game,
                                  const ruled::v1::RuledCommand &command,
                                  std::function<void(bool accepted)> onFinished)
{
    if (!isRuledGame(game)) {
        return;
    }
    GameEventHandler *handler = const_cast<AbstractGame *>(game)->getGameEventHandler();
    if (!handler) {
        return;
    }
    static_cast<RuledClientHost *>(handler)->sendRuledCommandExpectingAck(command, std::move(onFinished));
}

RuledClientState *stateForCard(const CardItem *card)
{
    if (!card || !card->getOwner()) {
        return nullptr;
    }
    return stateFor(card->getOwner()->getGame());
}

// ---------------------------------------------------------------------------------------
// CardItem lookup
// ---------------------------------------------------------------------------------------

bool parseCreaturePt(const QString &pt, int *outPower, int *outToughness)
{
    if (!outPower || !outToughness) {
        return false;
    }
    *outPower = *outToughness = 0;
    const QString s = pt.trimmed();
    if (s.isEmpty()) {
        return false;
    }
    const int slash = s.indexOf(QLatin1Char('/'));
    if (slash < 0) {
        return false;
    }
    const QString left = s.left(slash).trimmed();
    const QString right = s.mid(slash + 1).trimmed();
    if (left.contains(QLatin1Char('*')) || right.contains(QLatin1Char('*'))) {
        return false;
    }
    bool okP = false;
    bool okT = false;
    *outPower = left.toInt(&okP);
    *outToughness = right.toInt(&okT);
    return okP && okT;
}

CardItem *findBattlefieldCardItemByEngineOid(AbstractGame *game, quint32 engineOid)
{
    RuledClientState *state = stateFor(game);
    if (!game || !state || engineOid == 0) {
        return nullptr;
    }
    const int sid = state->cardIdForEngineOid(engineOid);
    const int owner = state->playerIdForEngineOid(engineOid);
    if (sid >= 0 && owner >= 0) {
        if (CardItem *c = game->getCard(owner, QString::fromLatin1(ZoneNames::TABLE), sid)) {
            return c;
        }
    }
    PlayerManager *pm = game->getPlayerManager();
    for (Player *p : pm->getPlayers()) {
        if (!p) {
            continue;
        }
        CardZoneLogic *zt = p->getZones().value(QString::fromLatin1(ZoneNames::TABLE), nullptr);
        if (!zt) {
            continue;
        }
        for (CardItem *c : zt->getCards()) {
            if (!c) {
                continue;
            }
            const int cid = c->getId();
            // BattlefieldObjectMap keys (player_id, server_card_id) use the server zone controller;
            // CardItem ownership can disagree, so try every seat id that appears in the oid map.
            for (Player *op : pm->getPlayers()) {
                if (!op || !op->getPlayerInfo()) {
                    continue;
                }
                if (state->engineOidForCardId(op->getPlayerInfo()->getId(), cid) == engineOid) {
                    return c;
                }
            }
        }
    }
    return nullptr;
}

CardItem *findStackCardItemByServerCardId(AbstractGame *game, int serverCardId)
{
    if (!game || serverCardId < 0) {
        return nullptr;
    }
    if (TabGame *tab = game->getTab()) {
        if (CardItem *c = tab->findVisibleStackSpellCardItem(serverCardId)) {
            return c;
        }
    }
    return rawStackCardByServerId(game, serverCardId);
}

CardItem *findStackCardItemByEngineOid(AbstractGame *game, quint32 stackOid)
{
    RuledClientState *state = stateFor(game);
    if (!game || !state) {
        return nullptr;
    }
    const int sid = state->cardIdForEngineOid(stackOid);
    if (sid >= 0) {
        if (CardItem *c = findStackCardItemByServerCardId(game, sid)) {
            return c;
        }
    }
    PlayerManager *pm = game->getPlayerManager();
    for (Player *p : pm->getPlayers()) {
        if (!p) {
            continue;
        }
        CardZoneLogic *sz = p->getZones().value(QString::fromLatin1(ZoneNames::STACK), nullptr);
        if (!sz) {
            continue;
        }
        for (CardItem *c : sz->getCards()) {
            if (!c) {
                continue;
            }
            const int cid = c->getId();
            for (Player *op : pm->getPlayers()) {
                if (!op || !op->getPlayerInfo()) {
                    continue;
                }
                if (state->engineOidForCardId(op->getPlayerInfo()->getId(), cid) == stackOid) {
                    return c;
                }
            }
        }
    }
    return nullptr;
}

CardItem *findGraveyardCardItemByEngineOid(AbstractGame *game, quint32 engineOid)
{
    RuledClientState *state = stateFor(game);
    if (!game || !state) {
        return nullptr;
    }
    const auto pidIt = state->graveyardOidToPlayerId.constFind(engineOid);
    if (pidIt == state->graveyardOidToPlayerId.constEnd()) {
        return nullptr;
    }
    const int playerId = pidIt.value();
    const int serverCardId = state->graveyardOidToServerCardId.value(engineOid, -1);
    if (serverCardId < 0) {
        return nullptr;
    }
    // Prefer the copy in an open zone view: a graveyard pile paints only its front card, so that
    // is the only place the *targeted* card has a position of its own to point at.
    if (TabGame *tab = game->getTab()) {
        if (CardItem *visible = tab->findVisibleGraveyardCardItem(playerId, serverCardId)) {
            return visible;
        }
    }
    // Pile closed: fall back to the card in the pile zone. `PileZone::reorganizeCards` never lays
    // its cards out, so they all sit at the pile's own position — an arrow to any of them points
    // at the graveyard, which is exactly what should happen when the card itself is not visible.
    return game->getCard(playerId, QString::fromLatin1(ZoneNames::GRAVE), serverCardId);
}

ArrowTarget *resolveSpellTargetItem(AbstractGame *game, RuledClientState *state, quint32 targetOid)
{
    if (!game || !state) {
        return nullptr;
    }
    PlayerManager *pm = game->getPlayerManager();
    // Only Cockatrice seat ids count as player targets (engine object ids never collide with seats).
    const int seatId = static_cast<int>(targetOid);
    if (pm->getPlayers().contains(seatId)) {
        if (Player *asPlayer = pm->getPlayer(seatId)) {
            return asPlayer->getGraphicsItem()->getPlayerTarget();
        }
    }
    if (state->getStackOidOrder().contains(targetOid)) {
        return findStackCardItemByEngineOid(game, targetOid);
    }
    // Counterspell target: the countered spell's oid may already be removed from the stack order in
    // the same batch as StackResolved, while StackPushed.targets still reference it — resolve via
    // the id map plus the physical stack zone.
    const int sidProbe = state->cardIdForEngineOid(targetOid);
    if (sidProbe >= 0) {
        if (CardItem *stk = rawStackCardByServerId(game, sidProbe)) {
            if (stk->getZone() &&
                stk->getZone()->getName().compare(QStringLiteral("stack"), Qt::CaseInsensitive) == 0) {
                if (TabGame *tab = game->getTab()) {
                    if (CardItem *vis = tab->findVisibleStackSpellCardItem(sidProbe)) {
                        return vis;
                    }
                }
                return stk;
            }
        }
    }
    if (CardItem *grave = findGraveyardCardItemByEngineOid(game, targetOid)) {
        return grave;
    }
    return findBattlefieldCardItemByEngineOid(game, targetOid);
}

// ---------------------------------------------------------------------------------------
// Clicked hand card → engine hand slot
// ---------------------------------------------------------------------------------------

int engineHandIndexFromLegalSlots(const RuledClientState *state,
                                  const CardItem *card,
                                  const QList<int> &sortedLegalHandIndices)
{
    if (!state || !card || !card->getZone()) {
        return -1;
    }
    const CardZoneLogic *zone = card->getZone();
    Player *handPlayer = zone->getPlayer();
    const int clickedId = card->getId();
    if (handPlayer && handPlayer->getPlayerInfo()) {
        const int pid = handPlayer->getPlayerInfo()->getId();
        const int mapped = state->engineHandSlotForServerCard(pid, clickedId);
        if (mapped >= 0 && sortedLegalHandIndices.contains(mapped)) {
            return mapped;
        }
    }
    for (int h : sortedLegalHandIndices) {
        if (h < 0 || h >= zone->getCards().size()) {
            continue;
        }
        const CardItem *slot = zone->getCards().at(h);
        if (slot && slot->getId() == clickedId) {
            return h;
        }
    }
    const int handIndex = zone->getCards().indexOf(const_cast<CardItem *>(card));
    if (handIndex >= 0 && sortedLegalHandIndices.contains(handIndex)) {
        return handIndex;
    }
    return -1;
}

int resolveHandActionIndex(const RuledClientState *state, RuledHandActionKind kind, const CardItem *card)
{
    if (!state || !card) {
        return -1;
    }
    // A land's client CardItem carries its front-face Oracle name (an MDFC's back face never becomes
    // a separate hand card), which the engine also uses to label each playable face of that slot —
    // so a single lookup resolves the shared hand slot for both faces of a pathway.
    // Cleanup and opening-bottom deliberately return every legal slot here: their CardItem is
    // identified authoritatively by HandSlotMap, and a display-name mismatch must not disable a
    // required game action.
    int resolved = engineHandIndexFromLegalSlots(state, card, state->handActionClickCandidates(kind, card->getName()));
    if (resolved >= 0 || kind != ruled::v1::HAND_ACTION_CAST_SPELL) {
        return resolved;
    }
    // CR 709/712/715: the engine labels each castable face of a multi-face card (split half / MDFC
    // face) by that face's own name, not the combined "A // B" Oracle name a CardItem carries. Try
    // each face so a click on a multi-face card resolves to its shared hand slot.
    const QStringList faceNames = card->getName().split(QStringLiteral(" // "), Qt::SkipEmptyParts);
    if (faceNames.size() > 1) {
        for (const QString &faceName : faceNames) {
            resolved = engineHandIndexFromLegalSlots(state, card, state->handActionIndicesForCardName(kind, faceName));
            if (resolved >= 0) {
                return resolved;
            }
        }
    }
    return resolved;
}

bool isResolutionPickZoneCard(const RuledClientState *state, const CardItem *card)
{
    if (!state || !card || !state->isResolutionHandPickActive()) {
        return false;
    }
    CardZoneLogic *zone = card->getZone();
    if (!zone) {
        return false;
    }
    const auto *viewZone = qobject_cast<const ZoneViewZoneLogic *>(zone);
    const Player *zonePlayer = zone->getPlayer();
    const bool zoneIsLocal = zonePlayer && zonePlayer->getPlayerInfo()->getLocal();
    switch (state->resolutionHandPickZone()) {
        case RuledClientState::PickZone::Hand:
            // Brainstorm: real cards in the local hand, carrying genuine Server_Card ids.
            return zone->getName() == ZoneNames::HAND && zoneIsLocal;
        case RuledClientState::PickZone::Deck:
            // Tutor / Gifts Ungiven search: the deck zone-*view* popup, never the face-down pile.
            return viewZone != nullptr && zone->getName() == ZoneNames::DECK && zoneIsLocal;
        case RuledClientState::PickZone::Revealed:
            // Revealed set / a looked-at hand: a popup built on the deck zone as a scaffold.
            return viewZone != nullptr && zone->getName() == ZoneNames::DECK;
    }
    return false;
}

// ---------------------------------------------------------------------------------------
// Click interpretation
// ---------------------------------------------------------------------------------------

bool isCombatEligibleCreature(const CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::TABLE) {
        return false;
    }
    if (card->getFaceDown()) {
        return false;
    }
    // Creature-ness is a ruled/mechanical decision: ask the engine (via the battlefield object
    // map) rather than the Oracle display DB. The Oracle path has no entry for engine-minted
    // tokens, so deciding from getCardType() would wrongly make tokens combat-ineligible.
    if (RuledClientState *state = stateForCard(card)) {
        Player *owner = card->getOwner();
        const int ownerPlayerId = owner ? owner->getPlayerInfo()->getId() : -1;
        const quint32 oid = state->engineOidForCardId(ownerPlayerId, card->getId());
        if (oid != 0) {
            return state->isEngineOidCreature(oid);
        }
    }
    return card->getCardInfo().getCardType().contains("Creature", Qt::CaseInsensitive);
}

namespace
{
/// Shared preamble for the two combat click handlers: ruled game, combat-eligible creature on the
/// battlefield, with a known engine ObjectId. Returns 0 when the click is not ours to interpret.
quint32 combatClickOid(const CardItem *card, RuledClientState **outState)
{
    RuledClientState *state = stateForCard(card);
    if (!state || !isCombatEligibleCreature(card)) {
        return 0;
    }
    Player *owner = card->getOwner();
    const int ownerPlayerId = owner ? owner->getPlayerInfo()->getId() : -1;
    const quint32 oid = state->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0) {
        return 0;
    }
    *outState = state;
    return oid;
}
} // namespace

bool tryHandleCombatRightClick(CardItem *card)
{
    RuledClientState *state = nullptr;
    const quint32 oid = combatClickOid(card, &state);
    if (oid == 0) {
        return false;
    }
    using Phase = RuledClientState::RuledCombatPhase;
    if (state->getCombatPhase() == Phase::AssignCombatDamage && state->localPlayerIsActive()) {
        const quint32 curAtt = state->currentCombatDamageAttackerOid();
        if (curAtt == 0) {
            return false;
        }
        if (state->getCommittedBlocks().value(oid, 0) != curAtt) {
            return false;
        }
        state->bumpBlockerCombatDamage(oid, -1);
        return true;
    }
    return false;
}

bool tryHandleCombatClick(CardItem *card)
{
    RuledClientState *state = nullptr;
    const quint32 oid = combatClickOid(card, &state);
    if (oid == 0) {
        return false;
    }
    using Phase = RuledClientState::RuledCombatPhase;
    const Phase phase = state->getCombatPhase();
    Player *owner = card->getOwner();
    const bool ownCreature = owner && owner->getPlayerInfo()->getLocal();

    if (phase == Phase::DeclareAttackers && state->localPlayerIsActive() && ownCreature) {
        if (card->getTapped()) {
            return false;
        }
        // CR 702.10b: Haste bypasses summoning sickness — a creature with Haste
        // may be selected as an attacker even on the turn it entered the battlefield.
        if (state->isEngineOidSummoningSick(oid) && !state->isEngineOidHaste(oid)) {
            return false;
        }
        state->togglePendingAttacker(oid);
        return true;
    }

    if (phase == Phase::DeclareBlockers && state->localPlayerIsDefender()) {
        if (ownCreature) {
            if (card->getTapped()) {
                return false;
            }
            // Toggle this creature in/out of the staged blocker set.
            state->toggleStagedBlocker(oid);
            return true;
        }
        // Clicked an enemy attacker — pair all staged blockers to it.
        if (state->hasStagedBlocker() && state->isCurrentAttacker(oid)) {
            state->pairStagedBlockerToAttacker(oid);
            return true;
        }
    }

    if (phase == Phase::AssignCombatDamage && state->localPlayerIsActive()) {
        const quint32 curAtt = state->currentCombatDamageAttackerOid();
        if (curAtt == 0) {
            return false;
        }
        // Only accept blockers assigned to the current attacker.
        if (state->getCommittedBlocks().value(oid, 0) != curAtt) {
            return false;
        }
        state->bumpBlockerCombatDamage(oid, +1);
        return true;
    }

    return false;
}

bool isSingleClickPlayLegal(const CardItem *card)
{
    if (!card || !card->getOwner() || !card->getZone()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    RuledClientState *state = stateForCard(card);
    if (!state) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(const_cast<CardItem *>(card)) < 0) {
        return false;
    }
    const bool isLand = card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive);
    const RuledHandActionKind kind =
        isLand ? ruled::v1::HAND_ACTION_PLAY_LAND : ruled::v1::HAND_ACTION_CAST_SPELL;
    return resolveHandActionIndex(state, kind, card) >= 0;
}

// ---------------------------------------------------------------------------------------
// PlayerActions passthroughs
// ---------------------------------------------------------------------------------------

bool isSelectedSpellTarget(const AbstractGame *game, quint32 oid)
{
    PlayerActions *actions = localPlayerActions(game);
    return actions && actions->isTargetSelectedForPendingSpell(oid);
}

bool isPlayerSelectedAsSpellTarget(const AbstractGame *game, int playerId)
{
    PlayerActions *actions = localPlayerActions(game);
    return actions && actions->isPlayerSelectedAsPendingSpellTarget(playerId);
}

bool isSpellDamageAllocationMode(const AbstractGame *game)
{
    PlayerActions *actions = localPlayerActions(game);
    return actions && actions->isInSpellDamageAllocationMode();
}

bool isSpellDamageAllocationDisplayActive(const AbstractGame *game)
{
    PlayerActions *actions = localPlayerActions(game);
    return actions && actions->isSpellDamageAllocationDisplayActive();
}

int spellDamageAllocationForOid(const AbstractGame *game, quint32 oid)
{
    PlayerActions *actions = localPlayerActions(game);
    return actions ? actions->spellDamageAllocationForOid(oid) : 0;
}

int spellDamageAllocationForPlayerId(const AbstractGame *game, int playerId)
{
    PlayerActions *actions = localPlayerActions(game);
    return actions ? actions->spellDamageAllocationForPlayerId(playerId) : 0;
}

} // namespace RuledActions
