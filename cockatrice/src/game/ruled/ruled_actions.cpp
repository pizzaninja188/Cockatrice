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

bool gameplayInputLocked(const AbstractGame *game)
{
    const RuledClientState *state = stateFor(game);
    return state && state->isEngineCommandPending();
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

namespace
{
/// The stack half of target resolution: the object's own stack CardItem, or — for a Counterspell
/// target already dropped from the stack order in the same batch as StackResolved — the physical
/// card still sitting in a stack zone.
ArrowTarget *stackTargetItem(AbstractGame *game, RuledClientState *state, quint32 targetOid)
{
    if (state->getStackOidOrder().contains(targetOid)) {
        if (CardItem *onStack = findStackCardItemByEngineOid(game, targetOid)) {
            return onStack;
        }
    }
    const int sidProbe = state->cardIdForEngineOid(targetOid);
    if (sidProbe < 0) {
        return nullptr;
    }
    CardItem *stk = rawStackCardByServerId(game, sidProbe);
    if (!stk || !stk->getZone() ||
        stk->getZone()->getName().compare(QStringLiteral("stack"), Qt::CaseInsensitive) != 0) {
        return nullptr;
    }
    if (TabGame *tab = game->getTab()) {
        if (CardItem *vis = tab->findVisibleStackSpellCardItem(sidProbe)) {
            return vis;
        }
    }
    return stk;
}

/// Seat graphics target for `targetOid` read as a Cockatrice seat id, or null if no such seat.
/// Engine object ids never collide with seat ids, which is what makes the read unambiguous.
ArrowTarget *playerTargetItem(AbstractGame *game, quint32 targetOid)
{
    PlayerManager *pm = game->getPlayerManager();
    const int seatId = static_cast<int>(targetOid);
    if (!pm->getPlayers().contains(seatId)) {
        return nullptr;
    }
    Player *asPlayer = pm->getPlayer(seatId);
    return asPlayer ? asPlayer->getGraphicsItem()->getPlayerTarget() : nullptr;
}
} // namespace

RuledTargetItemKind classifySpellTargetItem(AbstractGame *game, RuledClientState *state, quint32 targetOid)
{
    if (!game || !state) {
        return RuledTargetItemKind::Unknown;
    }
    if (playerTargetItem(game, targetOid)) {
        return RuledTargetItemKind::Player;
    }
    if (stackTargetItem(game, state, targetOid)) {
        return RuledTargetItemKind::Stack;
    }
    // Graveyard before battlefield: a card targeted in a graveyard (Reanimate) is not on the
    // battlefield, so the two can never both match at the moment of classification.
    if (findGraveyardCardItemByEngineOid(game, targetOid)) {
        return RuledTargetItemKind::Graveyard;
    }
    if (findBattlefieldCardItemByEngineOid(game, targetOid)) {
        return RuledTargetItemKind::Battlefield;
    }
    return RuledTargetItemKind::Unknown;
}

ArrowTarget *
resolveSpellTargetItem(AbstractGame *game, RuledClientState *state, quint32 targetOid, RuledTargetItemKind kind)
{
    if (!game || !state) {
        return nullptr;
    }
    switch (kind) {
        case RuledTargetItemKind::Player:
            return playerTargetItem(game, targetOid);
        case RuledTargetItemKind::Stack:
            return stackTargetItem(game, state, targetOid);
        case RuledTargetItemKind::Graveyard:
            return findGraveyardCardItemByEngineOid(game, targetOid);
        case RuledTargetItemKind::Battlefield:
            return findBattlefieldCardItemByEngineOid(game, targetOid);
        case RuledTargetItemKind::Unknown:
            break;
    }
    return nullptr;
}

// ---------------------------------------------------------------------------------------
// Clicked hand card → engine hand slot
// ---------------------------------------------------------------------------------------

int resolveHandActionIndex(const RuledClientState *state, RuledHandActionKind kind, const CardItem *card)
{
    if (!state || !card || !card->getZone()) {
        return -1;
    }
    // HandSlotMap binds the exact physical CardItem to its engine slot. Resolve against every slot
    // the engine offered for this action, not its display name: Adventure cards show only the
    // permanent name while the sole currently castable face may be the differently named spell.
    // Never fall back to visual hand order: a dev-conjured card can become visible during the
    // full-state resync before the matching ruled payload arrives, and a positional guess can
    // turn that click into an action on a different physical card.
    Player *handPlayer = card->getZone()->getPlayer();
    if (!handPlayer || !handPlayer->getPlayerInfo()) {
        return -1;
    }
    return state->legalHandSlotForServerCard(kind, handPlayer->getPlayerInfo()->getId(), card->getId());
}

quint32 resolvePublicZoneObjectId(const RuledClientState *state, const CardItem *card)
{
    if (!state || !card || !card->getZone() || !card->getZone()->getPlayer()) {
        return 0;
    }
    const int playerId = card->getZone()->getPlayer()->getPlayerInfo()->getId();
    if (card->getZone()->getName() == ZoneNames::GRAVE) {
        return state->graveyardEngineOidForOwnedCard(playerId, card->getId());
    }
    if (card->getZone()->getName() == ZoneNames::EXILE) {
        return state->exileEngineOidForOwnedCard(playerId, card->getId());
    }
    return 0;
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
    const RuledPickScaffoldZone scaffoldZone = zone->getName() == ZoneNames::HAND
                                                   ? RuledPickScaffoldZone::Hand
                                               : zone->getName() == ZoneNames::DECK ? RuledPickScaffoldZone::Deck
                                                                                   : RuledPickScaffoldZone::Other;
    return isRuledPickSurfaceCard(state->resolutionHandPickZone(), scaffoldZone, viewZone != nullptr, zoneIsLocal);
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
/// Resolve any ruled battlefield card to its engine ObjectId. Defender selection deliberately
/// accepts noncreatures (planeswalkers and Battles); the attacker/blocker branches apply the
/// creature gate after this lookup.
quint32 combatObjectOid(const CardItem *card, RuledClientState **outState)
{
    RuledClientState *state = stateForCard(card);
    if (!state || state->isEngineCommandPending() || !card->getZone() ||
        card->getZone()->getName() != ZoneNames::TABLE) {
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
    const quint32 oid = combatObjectOid(card, &state);
    if (oid == 0 || !isCombatEligibleCreature(card)) {
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
    const quint32 oid = combatObjectOid(card, &state);
    if (oid == 0) {
        return false;
    }
    using Phase = RuledClientState::RuledCombatPhase;
    const Phase phase = state->getCombatPhase();
    Player *owner = card->getOwner();
    const bool creature = isCombatEligibleCreature(card);
    const bool ownCreature = creature && owner && owner->getPlayerInfo()->getLocal();

    if (state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender)) {
        if (state->isLegalAttackPermanentDefender(oid)) {
            state->chooseAttackPermanentDefender(oid);
        }
        // Consume every battlefield click while the engine is waiting for this mandatory choice;
        // an invalid click must never fall through to freeform movement or selection.
        return true;
    }

    if (phase == Phase::DeclareAttackers && state->localPlayerIsActive() && state->isChoosingAttackDefender() &&
        state->isLegalAttackPermanentDefender(oid)) {
        state->chooseAttackPermanentDefender(oid);
        return true;
    }

    if (phase == Phase::DeclareAttackers && state->localPlayerIsActive() && ownCreature) {
        // The engine's set already accounts for tapping, summoning sickness, haste, and effects
        // such as Pacifism. Consume invalid clicks so ruled combat never falls through to freeform.
        if (!state->isSelectableAttacker(oid)) {
            return true;
        }
        state->togglePendingAttacker(oid);
        return true;
    }

    if (phase == Phase::DeclareBlockers && state->localPlayerIsDefender()) {
        if (ownCreature) {
            if (!state->isSelectableBlocker(oid)) {
                return true;
            }
            // Toggle this creature in/out of the staged blocker set.
            state->toggleStagedBlocker(oid);
            return true;
        }
        // Clicked an enemy attacker — pair all staged blockers to it.
        if (state->isCurrentAttacker(oid)) {
            if (state->hasStagedBlocker()) {
                state->pairStagedBlockerToAttacker(oid);
            }
            // Declared attackers are combat controls during this step. Consume every click so an
            // illegal pair (including an unblockable attacker) cannot fall through to freeform UI.
            return true;
        }
    }

    if (phase == Phase::AssignCombatDamage && state->localPlayerIsActive() && creature) {
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
    const bool inHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool inPublicCastZone = card->getZone()->getName() == ZoneNames::GRAVE ||
                                  card->getZone()->getName() == ZoneNames::EXILE;
    if (!inHand && !inPublicCastZone) {
        return false;
    }
    RuledClientState *state = stateForCard(card);
    if (!state) {
        return false;
    }
    const int zoneIndex = card->getZone()->getCards().indexOf(const_cast<CardItem *>(card));
    if (zoneIndex < 0) {
        return false;
    }
    if (inPublicCastZone) {
        const quint32 objectId = resolvePublicZoneObjectId(state, card);
        const RuledCastSource source = card->getZone()->getName() == ZoneNames::GRAVE
                                           ? RuledCastSource::Graveyard
                                           : RuledCastSource::Exile;
        return objectId != 0 &&
               (state->isZoneActionLegal(objectId, source) || state->isZoneLandActionLegal(objectId));
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
    if (actions && actions->isTargetSelectedForPendingSpell(oid)) {
        return true;
    }
    const RuledClientState *state = stateFor(game);
    return state && state->isPendingTriggerTargetSelected(oid);
}

bool isCombatDefenderPlayerCandidate(const Player *player)
{
    if (!player || !player->getGame()) {
        return false;
    }
    RuledClientState *state = stateFor(player->getGame());
    return state && state->isLegalAttackPlayerDefender(player->getPlayerInfo()->getId());
}

bool tryHandleCombatDefenderPlayerClick(Player *player)
{
    if (!isCombatDefenderPlayerCandidate(player)) {
        return false;
    }
    RuledClientState *state = stateFor(player->getGame());
    return state && state->chooseAttackPlayerDefender(player->getPlayerInfo()->getId());
}

bool isSelectedCastCostPermanent(const AbstractGame *game, quint32 oid)
{
    PlayerActions *actions = localPlayerActions(game);
    return actions && actions->isCastCostPermanentSelected(oid);
}

bool isSelectedGraveyardCostObject(const AbstractGame *game, quint32 oid)
{
    PlayerActions *actions = localPlayerActions(game);
    RuledClientState *state = stateFor(game);
    return (actions && actions->isRuledGraveyardCostObjectSelected(oid)) ||
           (state && state->isResolutionCostObjectSelected(oid));
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
