// Fork-owned. See ruled_game_driver.h. This code was extracted verbatim from
// server_game.cpp (roadmap Step 4); Server_Game state is reached via its public
// interface plus the `friend class RuledGameDriver` grant (participants, currentReplay).

#include "ruled_game_driver.h"

#include "../server_abstractuserinterface.h"
#include "ruled_utils.h"
#include "server_abstract_player.h"
#include "server_card.h"
#include "server_cardzone.h"
#include "server_counter.h"
#include "server_game.h"
#include "server_player.h"

#include <QDebug>
#include <QRandomGenerator>
#include <QSet>
#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/deck_list/deck_list.h>
#include <libcockatrice/deck_list/tree/deck_list_card_node.h>
#include <libcockatrice/protocol/pb/command_move_card.pb.h>
#include <libcockatrice/protocol/pb/event_attach_card.pb.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_notify_user.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_set_card_attr.pb.h>
#include <libcockatrice/protocol/pb/event_set_counter.pb.h>
#include <libcockatrice/protocol/pb/game_replay.pb.h>
#include <libcockatrice/protocol/pb/session_event.pb.h>
#include <libcockatrice/utility/zone_names.h>

namespace {

QString normalizeRuledCardName(const QString &name)
{
    return name.trimmed().toLower().replace(QLatin1Char('_'), QLatin1Char(' '));
}

/// Ruled mode routes every cast onto the lowest player-id stack zone (see `processRuledPayload`).
Server_CardZone *ruledCanonicalStackZone(Server_Game *game)
{
    if (!game) {
        return nullptr;
    }
    Server_AbstractPlayer *canonicalStackOwner = nullptr;
    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
        if (!ab) {
            continue;
        }
        if (!canonicalStackOwner || ab->getPlayerId() < canonicalStackOwner->getPlayerId()) {
            canonicalStackOwner = ab;
        }
    }
    return canonicalStackOwner ? canonicalStackOwner->getZones().value(ZoneNames::STACK) : nullptr;
}

/// Bind `StackPushed` to the cockatrice `Server_Card` that actually sits on the shared stack.
/// Walk from the end of the zone list toward the bottom so the most recently moved spell wins
/// when multiple copies share a name. Returns nullptr when no name match is found — callers
/// must not fall back to an unrelated card, since abilities have no physical card on the stack.
Server_Card *ruledPhysicalSpellOnCanonicalStack(Server_CardZone *stackZone, const QString &normalizedPushedName)
{
    if (!stackZone) {
        return nullptr;
    }
    const QList<Server_Card *> &cards = stackZone->getCards();
    for (int i = cards.size() - 1; i >= 0; --i) {
        Server_Card *c = cards.at(i);
        if (!c) {
            continue;
        }
        if (normalizeRuledCardName(c->getName()) == normalizedPushedName) {
            return c;
        }
    }
    return nullptr;
}

/// Legacy casual fallback: the engine refused the session for a non-card reason, so the
/// physical decks (left unshuffled for engine-ordered play) get a normal shuffle instead.
void shuffleMainDeckForRuledFallback(Server_AbstractPlayer *player)
{
    if (Server_CardZone *deckZone = player->getZones().value(ZoneNames::DECK)) {
        deckZone->shuffle();
    }
}

int expectedMainboardSizeForStartupSync(Server_Game *game,
                                        int playerId,
                                        const QList<QPair<int, QStringList>> &deckByPlayer)
{
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        if (row.first == playerId) {
            return static_cast<int>(row.second.size());
        }
    }
    if (Server_AbstractPlayer *player = game->getPlayer(playerId)) {
        if (const Server_CardZone *deckZone = player->getZones().value(ZoneNames::DECK)) {
            return static_cast<int>(deckZone->getCards().size());
        }
    }
    return 60;
}

} // namespace

RuledGameDriver::RuledGameDriver(Server_Game *_game) : game(_game), ruledSeed(0), ruledPriorityPlayer(-1)
{
}

RuledGameDriver::~RuledGameDriver() = default;

void RuledGameDriver::insertParticipantForTest(int id, Server_AbstractParticipant *participant)
{
    game->participants.insert(id, participant);
}

bool RuledGameDriver::validateDecksForStart()
{
    // A ruled game must not start while any mainboard contains cards the rules engine
    // doesn't implement — no silent casual fallback. Validation is stateless (no engine
    // session); on failure the pregame continues so players can swap decks.
    const QList<QPair<int, QStringList>> deckByPlayer = ruledMainboardNamesByPlayer();
    QStringList allNames;
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        allNames += row.second;
    }
    if (allNames.isEmpty()) {
        return true;
    }
    RulesRelay validationRelay;
    ruled::v1::IpcResponse validateResp;
    if (!validationRelay.validateDeck(allNames, validateResp)) {
        // Sidecar unreachable (or validation failed): we cannot start a ruled game
        // without the engine, and the client has already committed to ruled UI at join
        // time, so we cannot silently downgrade to casual mid-life. Block the start the
        // same way unimplemented cards do — notify, un-ready, keep the pregame open so
        // players can retry once the engine is back.
        notifyRuledEngineUnreachable();
    } else if (!validateResp.missing_card_names().empty()) {
        QStringList missing;
        for (const std::string &name : validateResp.missing_card_names()) {
            missing.append(QString::fromStdString(name));
        }
        notifyRuledUnimplementedCards(deckByPlayer, missing);
    } else {
        return true;
    }
    for (auto *player : game->getPlayers().values()) {
        player->setReadyStart(false);
    }
    game->sendGameStateToPlayers();
    return false;
}

void RuledGameDriver::resetForNewGame()
{
    // Per-player engine identity maps are rebuilt from the new session's zone views. Carrying
    // them across games republishes dead Server_Card ids (notably in GraveyardObjectMap, which
    // is only re-sent when non-empty) and lets stale oids resolve to cards that no longer exist.
    playerBindings.clear();
    ruledEngineStackPushDescriptionsByObjectId.clear();
    ruledStackObjectIdToServerCardId.clear();
    ruledStackObjectIdToCasterPlayerId.clear();
    ruledStackTargetsByObjectId.clear();
    ruledStackCopyObjectIds.clear();
    ruledPendingCastVisualQueue.clear();
    // Reset the connection-lost flag so that a back-to-back second game can report and
    // handle a fresh engine disconnect correctly. Without this reset, if game 1 lost the
    // engine connection the flag stays true, handleRuledEngineConnectionLost() returns early
    // for game 2, no notification is sent, and rulesRelay is never dropped — causing every
    // subsequent ruled command in game 2 to time out rather than fail fast.
    ruledEngineConnectionLost = false;
}

void RuledGameDriver::endSidecarSession()
{
    if (rulesRelay) {
        rulesRelay->sessionEnd();
        rulesRelay.reset();
    }
}

Response::ResponseCode
RuledGameDriver::processRuledPayload(int playerId, const Command_RuledPayload &cmd, GameEventStorage & /*ges*/)
{
    if (!rulesRelay) {
        return Response::RespInvalidCommand;
    }

    ruled::v1::RuledCommand uiOnlyCmd;
    if (uiOnlyCmd.ParseFromString(cmd.payload())) {
        if (uiOnlyCmd.has_preview_declare_blockers()) {
            // Cockatrice-only: show tentative blocks to the opponent. Never touch the engine or replay log.
            constexpr int declareBlockersToolbarPhase = 6;
            if (game->getActivePhase() != declareBlockersToolbarPhase || game->getActivePlayer() < 0 ||
                playerId == game->getActivePlayer()) {
                return Response::RespContextError;
            }
            ruled::v1::IpcResponse previewResp;
            previewResp.set_ok(true);
            auto *bpMsg = previewResp.mutable_batch()->add_events()->mutable_blockers_preview();
            bpMsg->set_declaring_player_id(playerId);
            const auto &pairs = uiOnlyCmd.preview_declare_blockers();
            for (int pi = 0; pi < pairs.block_pairs_size(); ++pi) {
                const auto &pr = pairs.block_pairs(pi);
                auto *out = bpMsg->add_block_pairs();
                out->set_attacker_id(pr.attacker_id());
                out->set_blocker_id(pr.blocker_id());
            }
            broadcastRuledResponse(previewResp);
            return Response::RespOk;
        }
        if (uiOnlyCmd.has_preview_declare_attackers()) {
            constexpr int declareAttackersToolbarPhase = 5;
            if (game->getActivePhase() != declareAttackersToolbarPhase || game->getActivePlayer() < 0 ||
                playerId != game->getActivePlayer()) {
                return Response::RespContextError;
            }
            ruled::v1::IpcResponse previewResp;
            previewResp.set_ok(true);
            auto *apMsg = previewResp.mutable_batch()->add_events()->mutable_attackers_preview();
            apMsg->set_declaring_player_id(playerId);
            const auto &ids = uiOnlyCmd.preview_declare_attackers();
            for (int ai = 0; ai < ids.creature_ids_size(); ++ai) {
                apMsg->add_attacker_object_ids(static_cast<uint32_t>(ids.creature_ids(ai)));
            }
            broadcastRuledResponse(previewResp);
            return Response::RespOk;
        }
    }

    ruled::v1::IpcResponse resp;
    QByteArray payload = QByteArray::fromStdString(cmd.payload());
    if (!rulesRelay->playerCommand(playerId, payload, resp)) {
        // Relay (not the engine) failed: the sidecar connection dropped mid-game. Tell the
        // players why the game has frozen rather than returning a silent internal error.
        handleRuledEngineConnectionLost();
        return Response::RespInternalError;
    }
    if (!resp.ok()) {
        return Response::RespContextError;
    }
    ruled::v1::RuledCommand ruledCmd;
    if (ruledCmd.ParseFromString(cmd.payload())) {
        if (Server_AbstractPlayer *cmdPlayer = game->getPlayer(playerId)) {
            Server_CardZone *handZone = cmdPlayer->getZones().value(ZoneNames::HAND);
            if (ruledCmd.has_play_land()) {
                Server_CardZone *tableZone = cmdPlayer->getZones().value(ZoneNames::TABLE);
                const int handIndex = static_cast<int>(ruledCmd.play_land().hand_card_index());
                if (handZone && tableZone && handIndex >= 0 && handIndex < handZone->getCards().size()) {
                    Server_Card *card = handZone->getCards().at(handIndex);
                    // CR 712: an MDFC land (a pathway) enters as the chosen face. Rename the physical
                    // card to that face's Oracle name before it moves to the battlefield, so the
                    // move event reveals the active face and the client shows its art (cards.xml has
                    // a separate entry per face). The catalog maps both face names to the same engine
                    // id, so later zone-view reconciliation still resolves this permanent.
                    const int faceIndex = static_cast<int>(ruledCmd.play_land().face_index());
                    if (faceIndex > 0) {
                        const QString activeName = ruledActiveFaceName(ruledCardIdForName(card->getName()), faceIndex);
                        if (!activeName.isEmpty() && activeName != card->getName()) {
                            card->setCardRef(CardRef{activeName});
                        }
                    }
                    CardToMove cardToMove;
                    cardToMove.set_card_id(card->getId());
                    GameEventStorage moveGes;
                    // Cockatrice table uses 3 rows; lands belong on the bottom row (grid y = 2).
                    static constexpr int RULED_LAND_GRID_Y = 2;
                    if (cmdPlayer->moveCard(moveGes, handZone, QList<const CardToMove *>() << &cardToMove, tableZone,
                                            -1, RULED_LAND_GRID_Y, true) == Response::RespOk) {
                        moveGes.sendToGame(game);
                    }
                }
            } else if (ruledCmd.has_cast_spell()) {
                // Route all spells to the canonical (lowest player-id) stack zone so every
                // client's stack window shows the complete stack without a split view.
                // Resolution uses ruledStackObjectIdToCasterPlayerId to send the card to the
                // correct destination zone regardless of which physical zone it sat in.
                Server_CardZone *stackZone = ruledCanonicalStackZone(game);
                const int handIndex = static_cast<int>(ruledCmd.cast_spell().hand_card_index());
                if (handZone && stackZone && handIndex >= 0 && handIndex < handZone->getCards().size()) {
                    Server_Card *card = handZone->getCards().at(handIndex);
                    PendingRuledCastVisual pending;
                    pending.cardName = card ? card->getName() : QString();
                    pending.serverCardId = card ? card->getId() : -1;
                    pending.casterPlayerId = playerId;
                    for (int ti = 0; ti < ruledCmd.cast_spell().targets_size(); ++ti) {
                        pending.targetOids.append(static_cast<quint32>(ruledCmd.cast_spell().targets(ti).object_id()));
                    }
                    ruledPendingCastVisualQueue.append(pending);
                    CardToMove cardToMove;
                    cardToMove.set_card_id(card->getId());
                    GameEventStorage moveGes;
                    if (cmdPlayer->moveCard(moveGes, handZone, QList<const CardToMove *>() << &cardToMove, stackZone,
                                            -1, 0, true) == Response::RespOk) {
                        moveGes.sendToGame(game);
                    }
                }
            }
        }
    }

    // CR 605 float courtesy: an UndoManaAbility untaps the source mid-turn, so let that untap reach
    // the commanding player's clients (the normal guard only releases taps during the untap step).
    const int forceUntapForPlayerId = ruledCmd.has_undo_mana_ability() ? playerId : -1;
    const RuledBatchApplyResult batchResult = applyRuledBatch(resp, forceUntapForPlayerId);
    if (batchResult.zoneViewApplied && (batchResult.handOrLibraryChanged || batchResult.battlefieldOrderChanged)) {
        game->sendGameStateToPlayers();
    }
    // Append to deterministic replay log (concatenated RuledCommand bytes)
    if (game->currentReplay) {
        game->currentReplay->mutable_ruled_command_log()->append(payload.constData(),
                                                                 static_cast<size_t>(payload.size()));
    }
    broadcastRuledResponse(resp);
    return Response::RespOk;
}

void RuledGameDriver::relayRuledPayloadAndBroadcast(int playerId, const QByteArray &ruledCmdBytes)
{
    if (!rulesRelay || ruledCmdBytes.isEmpty()) {
        return;
    }
    ruled::v1::IpcResponse resp;
    if (!rulesRelay->playerCommand(playerId, ruledCmdBytes, resp)) {
        // Relay (not the engine) failed: the sidecar connection dropped mid-game.
        handleRuledEngineConnectionLost();
        return;
    }
    if (!resp.ok()) {
        return;
    }
    const RuledBatchApplyResult batchResult = applyRuledBatch(resp);
    if (batchResult.zoneViewApplied && (batchResult.handOrLibraryChanged || batchResult.battlefieldOrderChanged)) {
        game->sendGameStateToPlayers();
    }
    if (game->currentReplay) {
        game->currentReplay->mutable_ruled_command_log()->append(ruledCmdBytes.constData(),
                                                                 static_cast<size_t>(ruledCmdBytes.size()));
    }
    broadcastRuledResponse(resp);
}

void RuledGameDriver::applyRuledStackResolvedEvent(const ruled::v1::StackResolved &stackResolved)
{
    const quint32 resolvedOid = static_cast<quint32>(stackResolved.object_id());
    // CR 707.10d: a spell copy ceases to exist on resolution and has no physical card to move.
    // Returning here also stops the name-matching fallback from resolving the original spell's
    // still-on-stack card (the copy shares its card_id/name).
    if (ruledStackCopyObjectIds.remove(resolvedOid)) {
        ruledEngineStackPushDescriptionsByObjectId.remove(resolvedOid);
        return;
    }
    const QString engineStackDescription = ruledEngineStackPushDescriptionsByObjectId.value(resolvedOid);

    auto tryResolveCardOnStack = [this, &stackResolved](Server_AbstractPlayer *ab, Server_CardZone *stackZone,
                                                        Server_Card *card) -> bool {
        if (!ab || !stackZone || !card) {
            return false;
        }
        // The engine sets a destination on every resolve; an unspecified value means
        // engine/server skew. Default to graveyard (CR 608.3: only permanent spells
        // go to the battlefield).
        const ruled::v1::StackResolveDestination dest = stackResolved.destination();
        if (dest != ruled::v1::STACK_RESOLVE_DESTINATION_BATTLEFIELD &&
            dest != ruled::v1::STACK_RESOLVE_DESTINATION_GRAVEYARD) {
            qWarning() << "Ruled: StackResolved for object" << stackResolved.object_id()
                       << "has no destination; defaulting to graveyard";
        }
        const bool goesToBattlefield = (dest == ruled::v1::STACK_RESOLVE_DESTINATION_BATTLEFIELD);
        const quint32 resolvedOidLocal = static_cast<quint32>(stackResolved.object_id());
        const int casterPid = ruledStackObjectIdToCasterPlayerId.value(resolvedOidLocal, -1);
        Server_AbstractPlayer *destPlayer = ab;
        if (casterPid >= 0) {
            if (Server_AbstractPlayer *cp = game->getPlayer(casterPid)) {
                destPlayer = cp;
            }
        }
        Server_CardZone *targetZone =
            destPlayer->getZones().value(goesToBattlefield ? ZoneNames::TABLE : ZoneNames::GRAVE);
        if (!targetZone) {
            return false;
        }

        CardToMove cardToMove;
        cardToMove.set_card_id(card->getId());
        GameEventStorage moveGes;
        int targetY = 0;
        if (goesToBattlefield) {
            targetY = 1; // default: middle row (noncreature nonland)
            if (const CardDatabaseQuerier *q = CardDatabaseManager::query()) {
                if (const CardInfoPtr info = q->getCardInfo(card->getName())) {
                    if (info->getUiAttributes().tableRow == 2) {
                        targetY = 0; // creature → top row
                    }
                }
            }
        }
        // Battlefield: -1 means "find a free grid column". Graveyard is a pile that renders its
        // front card, so a resolved spell goes to position 0 (matching the freeform client, which
        // sends x=0 for stack->graveyard) rather than being appended behind everything already there.
        const int targetX = goesToBattlefield ? -1 : 0;
        if (ab->moveCard(moveGes, stackZone, QList<const CardToMove *>() << &cardToMove, targetZone, targetX, targetY,
                         true) == Response::RespOk) {
            moveGes.sendToGame(game);
            return true;
        }
        return false;
    };

    // Multiplayer: each player has their own Cockatrice stack zone. Prefer the physical card that was mapped when
    // this object was pushed (cast_spell → stack_pushed); never pop "first non-empty stack in player iteration order".
    const auto mappedIdIt = ruledStackObjectIdToServerCardId.constFind(resolvedOid);
    if (mappedIdIt != ruledStackObjectIdToServerCardId.constEnd()) {
        const int serverCardId = mappedIdIt.value();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK);
            if (!stackZone) {
                continue;
            }
            if (Server_Card *card = stackZone->getCard(serverCardId, nullptr, false)) {
                if (tryResolveCardOnStack(ab, stackZone, card)) {
                    return;
                }
            }
        }
    }

    // Fallback when no cast mapping exists (tokens / older batches): top of first non-empty stack.
    // Only fires when the top card's name matches the engine description — abilities that have no
    // physical card on the stack must not accidentally resolve an unrelated card here.
    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
        if (!ab) {
            continue;
        }
        Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK);
        if (!stackZone || stackZone->getCards().isEmpty()) {
            continue;
        }
        Server_Card *card = stackZone->getCards().last();
        if (!card) {
            continue;
        }
        if (!engineStackDescription.isEmpty() &&
            normalizeRuledCardName(card->getName()) != normalizeRuledCardName(engineStackDescription)) {
            break;
        }
        if (tryResolveCardOnStack(ab, stackZone, card)) {
            return;
        }
        break;
    }
}

RuledGameDriver::RuledBatchApplyResult RuledGameDriver::applyRuledBatch(const ruled::v1::IpcResponse &resp,
                                                                        int forceUntapForPlayerId)
{
    RuledBatchApplyResult result;
    if (!resp.has_batch()) {
        return result;
    }
    const ruled::v1::RuledEventBatch &batch = resp.batch();

    // One named method per pass. The pass order is load-bearing — never merge or reorder:
    // the catalog must be indexed before anything resolves a card name through it, the
    // pre-batch oid capture feeds PermanentMoved translation, tokens must exist before
    // the zone-view sync binds battlefield slots, PermanentMoved must run before zone views
    // reconcile hand/library counts, and attachment restore plus life/mana/combat
    // translation need the fresh post-zone-view oid maps.

    // Mid-game catalog refresh. Almost every batch carries no CardCatalog and leaves the index
    // untouched; a batch that does carries the whole catalog and replaces it.
    indexCardCatalogEvents(batch);
    applyDevCardConjures(batch, result);

    // Capture the pre-batch engine_oid -> Server_Card map per player. The engine has
    // already removed dead permanents from its battlefield, so the upcoming zone-view
    // sync will rebuild the map without them. We need the *prior* mapping to translate
    // PermanentMoved events into moveCard(...) calls.
    QHash<int, QHash<quint32, int>> preBatchOidMaps;
    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
        if (!ab) {
            continue;
        }
        preBatchOidMaps.insert(ab->getPlayerId(), playerBinding(ab->getPlayerId()).engineOidToServerCardId);
    }

    applyTokenCreations(batch);
    applyPermanentMoves(batch, preBatchOidMaps);
    applyPhaseStackAndZoneViews(batch, forceUntapForPlayerId, result);
    applyAttachmentRestores(batch);
    applyLifeManaAndCombatEvents(batch);
    return result;
}

// A dev command conjured a card that was in no decklist (see DevCardConjured), so there is no
// physical Server_Card behind the engine's new object. Mint one before the zone-view sync below,
// for the same reason applyTokenCreations runs early: the reconcile matches engine slots to
// physical cards and abandons the whole sync if it cannot.
//
// Runs before applyTokenCreations and applyPermanentMoves because a card conjured by this batch
// may also be moved by it.
void RuledGameDriver::applyDevCardConjures(const ruled::v1::RuledEventBatch &batch, RuledBatchApplyResult &result)
{
    GameEventStorage conjureGes;
    bool conjureGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_dev_card_conjured()) {
            continue;
        }
        const auto &dc = e.dev_card_conjured();
        Server_AbstractPlayer *owner = game->getPlayer(dc.owner_player_id());
        if (!owner) {
            continue;
        }
        const bool toBattlefield = dc.zone() == ruled::v1::DEV_ZONE_BATTLEFIELD;
        const bool created = playerBinding(dc.owner_player_id())
                                 .createRuledDevCard(static_cast<Server_Player *>(owner), dc.object_id(),
                                                     QString::fromStdString(dc.card_name()), dc.is_creature(),
                                                     toBattlefield, conjureGes);
        if (!created) {
            continue;
        }
        if (toBattlefield) {
            conjureGesHasEvents = true;
        } else {
            // A hand conjure broadcasts no creation event of its own — that would reveal the card
            // to the opponent. Flagging the hand as changed makes processRuledPayload issue the
            // ordinary full-state resync, which redacts private zones per recipient.
            result.handOrLibraryChanged = true;
        }
    }
    if (conjureGesHasEvents) {
        conjureGes.sendToGame(game);
    }
}

// Tokens (CR 111) appear on the engine battlefield with no physical card behind them. Mint
// one on the controller's table for each TokenCreated event before the zone-view sync,
// so that sync can bind the engine battlefield slot to a real Server_Card by ObjectId. Runs
// before PermanentMoved: a token created this batch cannot also have died this batch.
void RuledGameDriver::applyTokenCreations(const ruled::v1::RuledEventBatch &batch)
{
    GameEventStorage tokenCreateGes;
    bool tokenCreateGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_token_created()) {
            continue;
        }
        const auto &tc = e.token_created();
        if (!tc.has_identity()) {
            continue;
        }
        if (Server_AbstractPlayer *controller = game->getPlayer(tc.controller_player_id())) {
            playerBinding(tc.controller_player_id())
                .createRuledToken(static_cast<Server_Player *>(controller), static_cast<quint32>(tc.object_id()),
                                  tc.identity(), tokenCreateGes);
            tokenCreateGesHasEvents = true;
        }
    }
    if (tokenCreateGesHasEvents) {
        tokenCreateGes.sendToGame(game);
    }
}

// Apply every PermanentMoved before zone_view. Hand discards are already absent from the
// engine hand list in the sync that follows, so the server must move the physical card
// first or applyRuledEngineZoneView's deck+hand pool counts disagree with the engine.
void RuledGameDriver::applyPermanentMoves(const ruled::v1::RuledEventBatch &batch,
                                          const QHash<int, QHash<quint32, int>> &preBatchOidMaps)
{
    GameEventStorage permanentMoveGes;
    bool permanentMoveGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_permanent_moved()) {
            continue;
        }
        const auto &pm = e.permanent_moved();
        const int ownerId = pm.owner_player_id();
        const quint32 oid = static_cast<quint32>(pm.object_id());
        Server_AbstractPlayer *owner = game->getPlayer(ownerId);
        if (!owner) {
            continue;
        }
        Server_Card *card = nullptr;
        const auto preIt = preBatchOidMaps.constFind(ownerId);
        if (preIt != preBatchOidMaps.constEnd()) {
            const auto cardIdIt = preIt->constFind(oid);
            if (cardIdIt != preIt->constEnd()) {
                for (const char *zn : {ZoneNames::TABLE, ZoneNames::HAND, ZoneNames::STACK, ZoneNames::DECK}) {
                    Server_CardZone *z = owner->getZones().value(zn);
                    if (!z) {
                        continue;
                    }
                    if (Server_Card *c = z->getCard(*cardIdIt, nullptr, false)) {
                        card = c;
                        break;
                    }
                }
            }
        }
        // ReturnFromGraveyard: the card may be in the graveyard zone, not in the battlefield/hand
        // OID map. Try the graveyard map maintained by the player's binding.
        if (!card) {
            if (auto *sp = qobject_cast<Server_Player *>(owner)) {
                if (Server_Card *c = playerBinding(ownerId).findGraveyardCardByEngineOid(sp, oid)) {
                    card = c;
                }
            }
        }
        if (!card) {
            // A spell leaving the stack (e.g. countered) physically lives on the single shared
            // canonical stack zone (owned by the lowest player-id), not in this owner's own zones,
            // and stack cards are never in the per-player engine-oid map (only battlefield + hand
            // are — see RuledPlayerBinding::applyRuledEngineZoneView). Resolve it through the global
            // stack-object map and search every player's stack zone; the move below routes it to
            // the owner's destination. This makes countered spells generic — no name special-case.
            //
            // MUST run before the mill fallback: a countered spell's card_id (e.g. "lightning_bolt")
            // would otherwise name-match a *different* copy sitting in the owner's library, moving
            // that to the graveyard and leaving the real stacked card stranded as a ghost.
            const auto stackCardIdIt = ruledStackObjectIdToServerCardId.constFind(oid);
            if (stackCardIdIt != ruledStackObjectIdToServerCardId.constEnd()) {
                for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
                    if (!ab) {
                        continue;
                    }
                    if (Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK)) {
                        if (Server_Card *c = stackZone->getCard(*stackCardIdIt, nullptr, false)) {
                            card = c;
                            break;
                        }
                    }
                }
            }
        }
        if (!card) {
            // Library cards (mill: library -> graveyard) are never registered in the engine-oid
            // map (the library is synced by name list, not object ids), so the lookup above
            // misses. Resolve by tricerules card_id within the owner's deck instead; physical
            // instances of a given printing are fungible, so any matching-named card works. Each
            // PermanentMoved is a separate iteration and moveCard removes the card, so repeated
            // mills of the same card name consume distinct deck cards.
            const QString wantCardId = QString::fromStdString(pm.card_id());
            if (!wantCardId.isEmpty()) {
                if (Server_CardZone *deck = owner->getZones().value(ZoneNames::DECK)) {
                    for (Server_Card *c : deck->getCards()) {
                        if (ruledCardIdForName(c->getName()) == wantCardId) {
                            card = c;
                            break;
                        }
                    }
                }
            }
        }
        if (!card) {
            qWarning().noquote() << "ruled PermanentMoved unresolved: oid" << oid << "card_id"
                                 << QString::fromStdString(pm.card_id()) << "owner" << ownerId;
            continue;
        }
        Server_CardZone *startZone = card->getZone();
        if (!startZone) {
            continue;
        }
        // `destX` is the insert position, not a grid column, for the zones without coords.
        // A pile zone renders only its *front* card (PileZone::paint draws index 0), so the most
        // recently added card must go to position 0 — that is what the freeform client sends for
        // GRAVE/EXILE. Passing -1 appends to the far end (server_abstract_player.cpp turns it into
        // `getCards().size()`), which leaves the oldest card showing forever no matter how much
        // is milled. TABLE uses -1 to mean "find a free grid column"; hand/library order is not
        // rendered as a pile, so they keep the append behaviour.
        const char *destZone = ZoneNames::GRAVE;
        int destX = 0;
        switch (pm.destination()) {
            case ruled::v1::PermanentMoved::DESTINATION_HAND:
                destZone = ZoneNames::HAND;
                destX = -1;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_LIBRARY:
                destZone = ZoneNames::DECK;
                destX = -1;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_EXILE:
                destZone = ZoneNames::EXILE;
                destX = 0;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD:
                destZone = ZoneNames::TABLE;
                destX = -1;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD:
            default:
                destZone = ZoneNames::GRAVE;
                destX = 0;
                break;
        }
        Server_CardZone *targetZone = owner->getZones().value(destZone);
        if (!targetZone) {
            continue;
        }
        // Hidden zones (the library) address cards by position index in moveCard/getCard, not by
        // intrinsic card id. Mill moves come out of the deck, so pass the card's current position;
        // public/private zones (table, hand) use the card id. Position is recomputed per event
        // because each successful move re-indexes the hidden zone.
        int moveCardId = card->getId();
        if (startZone->getType() == ServerInfo_Zone::HiddenZone) {
            moveCardId = startZone->getCards().indexOf(card);
            if (moveCardId < 0) {
                continue;
            }
        }
        CardToMove cardToMove;
        cardToMove.set_card_id(moveCardId);
        if (owner->moveCard(permanentMoveGes, startZone, QList<const CardToMove *>() << &cardToMove, targetZone, destX,
                            0, true) == Response::RespOk) {
            permanentMoveGesHasEvents = true;
        }
    }
    if (permanentMoveGesHasEvents) {
        permanentMoveGes.sendToGame(game);
    }
}

// Phase / priority / stack push+resolve / zone view + tap sync.
// Tap state propagates from the engine on every batch — declare attackers, mana
// payment, and untap all use this path (no longer gated on an explicit untap event).
void RuledGameDriver::applyPhaseStackAndZoneViews(const ruled::v1::RuledEventBatch &batch,
                                                  int forceUntapForPlayerId,
                                                  RuledBatchApplyResult &result)
{
    GameEventStorage tapSyncGes;
    bool batchHasUntapPhase = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (e.has_phase_changed() && e.phase_changed().phase_id() == ruled::v1::PHASE_ID_UNTAP) {
            batchHasUntapPhase = true;
            break;
        }
    }
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (e.has_phase_changed()) {
            const int newActive = e.phase_changed().active_player_id();
            if (newActive >= 0 && game->getActivePlayer() != newActive) {
                game->setActivePlayer(newActive);
            }
            const int mappedPhase = ruledPhaseToCockatricePhase(e.phase_changed().phase_id());
            if (mappedPhase >= 0 && game->getActivePhase() != mappedPhase) {
                game->setActivePhase(mappedPhase);
            }
            result.phaseChanged = true;
        }
        if (e.has_priority_changed()) {
            const int newPriority = e.priority_changed().player_id();
            if (newPriority >= 0 && newPriority != ruledPriorityPlayer) {
                ruledPriorityPlayer = newPriority;
                if (Server_AbstractPlayer *prioPlayer = game->getPlayer(newPriority)) {
                    Event_GameSay priorityEvent;
                    const QString playerName = QString::fromStdString(prioPlayer->getUserInfo()->name());
                    priorityEvent.set_message(QStringLiteral("%1 gains priority.").arg(playerName).toStdString());
                    game->sendGameEventContainer(game->prepareGameEvent(priorityEvent, -1));
                }
            }
        }
        if (!e.has_zone_view()) {
            if (e.has_stack_pushed()) {
                const auto &sp = e.stack_pushed();
                const quint32 pushedOid = static_cast<quint32>(sp.object_id());
                // CR 707.10: a spell copy (Twincast/Fork) has no physical card on the shared stack.
                // Record its display name (for the prompt/log) but never bind it to a Server_Card —
                // its card_id matches the original, so binding would steal the original's physical
                // card. The resolve handler treats it as a no-op move (see ruledStackCopyObjectIds).
                if (sp.is_copy()) {
                    ruledStackCopyObjectIds.insert(pushedOid);
                    QString copyName = QString::fromStdString(sp.description());
                    if (!sp.card_id().empty()) {
                        const QString catalogName = ruledCardNameForId(QString::fromStdString(sp.card_id()));
                        if (!catalogName.isEmpty()) {
                            copyName = catalogName;
                        }
                    }
                    ruledEngineStackPushDescriptionsByObjectId.insert(pushedOid, copyName);
                    continue;
                }
                // Physical binding matches on display names. For spells, resolve the engine
                // card id through the catalog (authoritative Oracle name) rather than trusting
                // the free-form description; abilities carry no card_id and keep the description.
                QString pushedName = QString::fromStdString(sp.description());
                if (!sp.card_id().empty()) {
                    const QString catalogName = ruledCardNameForId(QString::fromStdString(sp.card_id()));
                    if (!catalogName.isEmpty()) {
                        pushedName = catalogName;
                    }
                }
                ruledEngineStackPushDescriptionsByObjectId.insert(pushedOid, pushedName);
                const QString normalizedPushedName = normalizeRuledCardName(pushedName);
                QList<PendingRuledCastVisual>::iterator bindIt = ruledPendingCastVisualQueue.end();
                for (auto it = ruledPendingCastVisualQueue.begin(); it != ruledPendingCastVisualQueue.end(); ++it) {
                    if (normalizeRuledCardName(it->cardName) == normalizedPushedName) {
                        bindIt = it;
                        break;
                    }
                }
                // CastSpell always enqueues one pending entry before applyRuledBatch; if the engine
                // stack description and Cockatrice printing names differ, name match fails and clients
                // never get BattlefieldObjectMap rows for stack oids (spell targeting arrows break).
                if (bindIt == ruledPendingCastVisualQueue.end() && !ruledPendingCastVisualQueue.isEmpty()) {
                    bindIt = ruledPendingCastVisualQueue.begin();
                }
                if (bindIt != ruledPendingCastVisualQueue.end()) {
                    ruledStackTargetsByObjectId.insert(pushedOid, bindIt->targetOids);
                    if (bindIt->serverCardId >= 0) {
                        ruledStackObjectIdToServerCardId.insert(pushedOid, bindIt->serverCardId);
                    }
                    if (bindIt->casterPlayerId >= 0) {
                        ruledStackObjectIdToCasterPlayerId.insert(pushedOid, bindIt->casterPlayerId);
                    }
                    ruledPendingCastVisualQueue.erase(bindIt);
                } else {
                    QVector<quint32> tlist;
                    tlist.reserve(sp.targets_size());
                    for (int ti = 0; ti < sp.targets_size(); ++ti) {
                        tlist.append(static_cast<quint32>(sp.targets(ti).object_id()));
                    }
                    if (!tlist.isEmpty()) {
                        ruledStackTargetsByObjectId.insert(pushedOid, tlist);
                    }
                }
                // Authoritative: map engine stack oid to the physical card.
                // All spells now go into the canonical zone so always look there.
                {
                    Server_CardZone *spellZone = ruledCanonicalStackZone(game);
                    if (spellZone) {
                        if (Server_Card *phys = ruledPhysicalSpellOnCanonicalStack(spellZone, normalizedPushedName)) {
                            ruledStackObjectIdToServerCardId.insert(pushedOid, phys->getId());
                        }
                    }
                }
            }
            if (e.has_stack_resolved()) {
                applyRuledStackResolvedEvent(e.stack_resolved());
            }
            continue;
        }
        for (const auto &p : e.zone_view().per_player()) {
            // Untap-step "reset" applies only to the active player's view; NAP may stay tapped.
            // UndoManaAbility (CR 605) also untaps mid-turn, but only for the player who undid it.
            const bool perPlayerAllowUntap = (batchHasUntapPhase && p.player_id() == game->getActivePlayer()) ||
                                             p.player_id() == forceUntapForPlayerId;
            if (Server_AbstractPlayer *ab = game->getPlayer(p.player_id())) {
                const RuledPlayerBinding::RuledZoneSyncResult sync =
                    playerBinding(p.player_id())
                        .applyRuledEngineZoneView(static_cast<Server_Player *>(ab), p, &tapSyncGes,
                                                  perPlayerAllowUntap);
                result.handOrLibraryChanged = result.handOrLibraryChanged || sync.handOrLibraryChanged;
                result.battlefieldOrderChanged = result.battlefieldOrderChanged || sync.battlefieldOrderChanged;
                result.tapStateEventsQueued = result.tapStateEventsQueued || sync.tapStateChanged;
                result.zoneViewApplied = true;
            }
        }
    }
    if (result.tapStateEventsQueued) {
        tapSyncGes.sendToGame(game);
    }
}

// Post-zone-view pass: restore attach state from battlefield_attached_to_oid for both
// Auras and Equipment. The engine OID maps are fresh after the zone-view pass, so
// cross-player lookups work. A non-zero attached_to_oid means the card at that slot is
// currently attached to that target — issue Event_AttachCard to bring client visual state
// into sync. This handles reconnect (initial_response_batch) and any batch with a zone_view.
void RuledGameDriver::applyAttachmentRestores(const ruled::v1::RuledEventBatch &batch)
{
    {
        GameEventStorage attachRestoreGes;
        bool attachRestoreGesHasEvents = false;
        for (int ei = 0; ei < batch.events_size(); ++ei) {
            const auto &e = batch.events(ei);
            if (!e.has_zone_view()) {
                continue;
            }
            for (const auto &p : e.zone_view().per_player()) {
                if (p.battlefield_objects_size() == 0) {
                    continue;
                }
                Server_AbstractPlayer *ownerAb = game->getPlayer(p.player_id());
                if (!ownerAb) {
                    continue;
                }
                auto *ownerPlayer = static_cast<Server_Player *>(ownerAb);
                for (const auto &battlefieldObject : p.battlefield_objects()) {
                    const quint32 targetOid = static_cast<quint32>(battlefieldObject.attached_to_oid());
                    if (targetOid == 0) {
                        continue;
                    }
                    const quint32 attachedOid = static_cast<quint32>(battlefieldObject.object_id());
                    Server_Card *attachedCard = playerBinding(p.player_id()).findCardByEngineOid(ownerPlayer, attachedOid);
                    if (!attachedCard || !attachedCard->getZone()) {
                        continue;
                    }
                    Server_Card *targetCard = nullptr;
                    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
                        if (!ab) {
                            continue;
                        }
                        targetCard = playerBinding(ab->getPlayerId())
                                         .findCardByEngineOid(static_cast<Server_Player *>(ab), targetOid);
                        if (targetCard) {
                            break;
                        }
                    }
                    if (!targetCard || !targetCard->getZone()) {
                        continue;
                    }
                    // Avoid redundant events when the server already knows about this attachment.
                    if (attachedCard->getParentCard() == targetCard) {
                        continue;
                    }
                    attachedCard->setParentCard(targetCard);
                    // Match cmdAttachCard: an attached card leaves the grid (x = -1) and is drawn
                    // against its parent.
                    const int attachedOldX = attachedCard->getX();
                    attachedCard->setCoords(-1, attachedCard->getY());
                    attachedCard->getZone()->updateCardCoordinates(attachedCard, attachedOldX,
                                                                   attachedCard->getY());
                    Event_AttachCard attachEv;
                    attachEv.set_start_zone(attachedCard->getZone()->getName().toStdString());
                    attachEv.set_card_id(attachedCard->getId());
                    attachEv.set_target_player_id(targetCard->getZone()->getPlayer()->getPlayerId());
                    attachEv.set_target_zone(targetCard->getZone()->getName().toStdString());
                    attachEv.set_target_card_id(targetCard->getId());
                    attachRestoreGes.enqueueGameEvent(attachEv, attachedCard->getZone()->getPlayer()->getPlayerId());
                    attachRestoreGesHasEvents = true;
                }
            }
        }
        if (attachRestoreGesHasEvents) {
            attachRestoreGes.sendToGame(game);
        }
    }
}

// Combat-related events that depend on the engine OID map (LifeChanged,
// AttackersDeclared) and stack resolution side effects that synthesize standard
// Cockatrice events for clients. PermanentMoved is handled earlier (before zone_view).
void RuledGameDriver::applyLifeManaAndCombatEvents(const ruled::v1::RuledEventBatch &batch)
{
    GameEventStorage combatGes;
    bool combatGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (e.has_life_changed()) {
            const auto &lc = e.life_changed();
            Server_AbstractPlayer *target = game->getPlayer(lc.player_id());
            if (!target) {
                continue;
            }
            auto *targetPlayer = static_cast<Server_Player *>(target);
            // Life is stored on the per-player counter id 0 ("life"). Update both server
            // state and broadcast a SetCounter event so clients render the change.
            const auto &allCounters = targetPlayer->getCounters();
            Server_Counter *lifeCounter = allCounters.value(0, nullptr);
            if (!lifeCounter || lifeCounter->getName() != QStringLiteral("life")) {
                // Fall back: search by name. Counter ids are stable in practice but be defensive.
                for (Server_Counter *c : allCounters) {
                    if (c && c->getName() == QStringLiteral("life")) {
                        lifeCounter = c;
                        break;
                    }
                }
            }
            if (!lifeCounter) {
                continue;
            }
            lifeCounter->setCount(lc.new_total());
            Event_SetCounter ev;
            ev.set_counter_id(lifeCounter->getId());
            ev.set_value(lifeCounter->getCount());
            combatGes.enqueueGameEvent(ev, target->getPlayerId());
            combatGesHasEvents = true;
        }
        if (e.has_mana_pool_updated()) {
            const auto &mp = e.mana_pool_updated();
            Server_AbstractPlayer *target = game->getPlayer(mp.player_id());
            if (!target) {
                continue;
            }
            auto *targetPlayer = static_cast<Server_Player *>(target);
            // CR 106: the engine is the sole owner of the mana pool. Mirror its absolute snapshot
            // onto the player's single-letter mana counters (w/u/b/r/g/c). Because it is absolute,
            // this one handler covers production (mana abilities), payment (pay_mana), and the
            // empty-on-step/phase-change case — so no separate server-side pool clear is needed.
            // Colorless ("c") has no display counter yet; it is harmlessly skipped until a
            // colorless-producing card (and counter) is added.
            const QHash<QString, int> desired = {
                {QStringLiteral("w"), static_cast<int>(mp.w())}, {QStringLiteral("u"), static_cast<int>(mp.u())},
                {QStringLiteral("b"), static_cast<int>(mp.b())}, {QStringLiteral("r"), static_cast<int>(mp.r())},
                {QStringLiteral("g"), static_cast<int>(mp.g())}, {QStringLiteral("c"), static_cast<int>(mp.c())},
            };
            for (Server_Counter *counter : targetPlayer->getCounters()) {
                if (!counter) {
                    continue;
                }
                const auto it = desired.constFind(counter->getName().trimmed().toLower());
                if (it == desired.constEnd() || counter->getCount() == it.value()) {
                    continue;
                }
                counter->setCount(it.value());
                Event_SetCounter ev;
                ev.set_counter_id(counter->getId());
                ev.set_value(it.value());
                combatGes.enqueueGameEvent(ev, target->getPlayerId());
                combatGesHasEvents = true;
            }
        }
        if (e.has_attackers_declared()) {
            const auto &ad = e.attackers_declared();
            Server_AbstractPlayer *attacker = game->getPlayer(ad.attacking_player_id());
            if (!attacker) {
                continue;
            }
            auto *attackerPlayer = static_cast<Server_Player *>(attacker);
            Server_CardZone *tableZone = attackerPlayer->getZones().value(ZoneNames::TABLE);
            if (tableZone) {
                for (Server_Card *card : tableZone->getCards()) {
                    if (!card || !card->getAttacking()) {
                        continue;
                    }
                    card->setAttacking(false);
                    Event_SetCardAttr clearEv;
                    clearEv.set_zone_name(std::string(ZoneNames::TABLE));
                    clearEv.set_card_id(card->getId());
                    clearEv.set_attribute(AttrAttacking);
                    clearEv.set_attr_value("0");
                    combatGes.enqueueGameEvent(clearEv, attacker->getPlayerId());
                    combatGesHasEvents = true;
                }
            }
            for (int i = 0; i < ad.attacker_object_ids_size(); ++i) {
                const quint32 oid = static_cast<quint32>(ad.attacker_object_ids(i));
                Server_Card *card = playerBinding(ad.attacking_player_id()).findCardByEngineOid(attackerPlayer, oid);
                if (!card) {
                    continue;
                }
                card->setAttacking(true);
                Event_SetCardAttr attEv;
                attEv.set_zone_name(std::string(ZoneNames::TABLE));
                attEv.set_card_id(card->getId());
                attEv.set_attribute(AttrAttacking);
                attEv.set_attr_value("1");
                combatGes.enqueueGameEvent(attEv, attacker->getPlayerId());
                combatGesHasEvents = true;
            }
        }
        if (e.has_stack_resolved()) {
            const quint32 resolvedOid = static_cast<quint32>(e.stack_resolved().object_id());
            // A countered spell leaves the stack for its owner's graveyard via a PermanentMoved
            // event the engine emits (handled generically in the first pass, above) — no per-card
            // special-case here. Just retire this stack object's bookkeeping.
            ruledStackTargetsByObjectId.remove(resolvedOid);
            ruledStackObjectIdToServerCardId.remove(resolvedOid);
            ruledStackObjectIdToCasterPlayerId.remove(resolvedOid);
            ruledEngineStackPushDescriptionsByObjectId.remove(resolvedOid);
        }
        // CR 303.4: an aura entering the battlefield attaches to its enchant target. Translate the
        // engine AuraAttached event into Event_AttachCard so the Cockatrice client stacks the aura
        // underneath the enchanted permanent (the visual layout used in freeform too).
        if (e.has_aura_attached()) {
            const auto &aa = e.aura_attached();
            const quint32 auraOid = static_cast<quint32>(aa.aura_object_id());
            const quint32 enchantedOid = static_cast<quint32>(aa.enchanted_object_id());
            Server_Card *auraCard = nullptr;
            Server_Card *enchantedCard = nullptr;
            for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
                if (!ab) {
                    continue;
                }
                auto *pl = static_cast<Server_Player *>(ab);
                RuledPlayerBinding &binding = playerBinding(pl->getPlayerId());
                if (!auraCard) {
                    auraCard = binding.findCardByEngineOid(pl, auraOid);
                }
                if (!enchantedCard) {
                    enchantedCard = binding.findCardByEngineOid(pl, enchantedOid);
                }
            }
            // Skip when the server already has this attachment: a repeated AuraAttached (e.g. in a
            // reconnect's initial batch) would otherwise re-broadcast Event_AttachCard and make the
            // client replay the attach.
            if (auraCard && enchantedCard && auraCard->getZone() &&
                auraCard->getParentCard() != enchantedCard) {
                auraCard->setParentCard(enchantedCard);
                // Match cmdAttachCard: an attached card leaves the grid (x = -1) and is drawn
                // against its parent. Without this it keeps a grid column of its own.
                const int auraOldX = auraCard->getX();
                auraCard->setCoords(-1, auraCard->getY());
                auraCard->getZone()->updateCardCoordinates(auraCard, auraOldX, auraCard->getY());
                Event_AttachCard attachEv;
                attachEv.set_start_zone(auraCard->getZone()->getName().toStdString());
                attachEv.set_card_id(auraCard->getId());
                if (enchantedCard->getZone()) {
                    attachEv.set_target_player_id(enchantedCard->getZone()->getPlayer()->getPlayerId());
                    attachEv.set_target_zone(enchantedCard->getZone()->getName().toStdString());
                    attachEv.set_target_card_id(enchantedCard->getId());
                }
                combatGes.enqueueGameEvent(attachEv, auraCard->getZone()->getPlayer()->getPlayerId());
                combatGesHasEvents = true;
            }
        }
    }
    if (combatGesHasEvents) {
        combatGes.sendToGame(game);
    }
}

void RuledGameDriver::broadcastRuledResponse(const ruled::v1::IpcResponse &resp)
{
    if (!resp.has_batch()) {
        return;
    }
    ruled::v1::IpcResponse toSend;
    toSend.set_ok(resp.ok());
    toSend.set_error(resp.error());
    toSend.mutable_batch()->CopyFrom(resp.batch());
    appendServerObjectMaps(toSend);
    const ruled::v1::RuledEventBatch &batch = toSend.batch();
    for (auto *participant : game->getParticipants()) {
        GameEventStorage ges;
        const ruled::v1::RuledEventBatch filtered = redactBatchForParticipant(batch, participant);
        Event_RuledPayload ev;
        std::string bytes;
        filtered.SerializeToString(&bytes);
        ev.set_payload(bytes);
        ges.enqueueGameEvent(ev, -1, GameEventStorageItem::SendToPrivate, participant->getPlayerId());
        ges.sendToGame(game);
    }
}

// Appends the server-built identity-map events to the outgoing batch: a
// BattlefieldObjectMap so clients can map their visible CardItem (Server_Card.id)
// back to the engine ObjectId that DeclareAttackers / DeclareBlockers expects, a
// HandSlotMap (zone_view hand/lib fields are cleared before broadcast), and a
// GraveyardObjectMap for graveyard spell targets. Rebuilt every batch from the latest sync.
void RuledGameDriver::appendServerObjectMaps(ruled::v1::IpcResponse &toSend)
{
    {
        ruled::v1::RuledEvent mapEvent;
        auto *map = mapEvent.mutable_battlefield_object_map();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            const RuledPlayerBinding &binding = playerBinding(pl->getPlayerId());
            const QHash<quint32, int> oidMap = binding.engineOidToServerCardId;
            Server_CardZone *tableZone = pl->getZones().value(ZoneNames::TABLE);
            int ordinal = 0;
            // Iterate the table zone in current order so `ordinal` matches the
            // controller-order seen in RuledPerPlayerView.battlefield.
            if (tableZone) {
                for (Server_Card *card : tableZone->getCards()) {
                    if (!card) {
                        continue;
                    }
                    QString tr = ruledCardIdForName(card->getName());
                    quint32 engineOid = 0;
                    bool found = false;
                    for (auto it = oidMap.constBegin(); it != oidMap.constEnd(); ++it) {
                        if (it.value() == card->getId()) {
                            engineOid = it.key();
                            found = true;
                            break;
                        }
                    }
                    if (!found) {
                        ++ordinal;
                        continue;
                    }
                    auto *entry = map->add_entries();
                    entry->set_player_id(pl->getPlayerId());
                    entry->set_engine_object_id(engineOid);
                    entry->set_card_id(tr.toStdString());
                    entry->set_ordinal(static_cast<uint32_t>(ordinal));
                    entry->set_server_card_id(card->getId());
                    entry->set_summoning_sick(binding.isEngineOidSummoningSick(engineOid));
                    if (binding.isEngineOidHaste(engineOid)) {
                        entry->add_keywords("Haste");
                    }
                    if (binding.isEngineOidTrample(engineOid)) {
                        entry->add_keywords("Trample");
                    }
                    entry->set_is_creature(binding.isEngineOidCreature(engineOid));
                    ++ordinal;
                }
            }
            Server_CardZone *stackZone = pl->getZones().value(ZoneNames::STACK);
            if (stackZone) {
                int stackOrdinal = 0;
                for (Server_Card *stackCard : stackZone->getCards()) {
                    if (!stackCard) {
                        continue;
                    }
                    quint32 stackOid = 0;
                    bool foundStackOid = false;
                    for (auto it = ruledStackObjectIdToServerCardId.constBegin();
                         it != ruledStackObjectIdToServerCardId.constEnd(); ++it) {
                        if (it.value() == stackCard->getId()) {
                            stackOid = it.key();
                            foundStackOid = true;
                            break;
                        }
                    }
                    if (!foundStackOid) {
                        ++stackOrdinal;
                        continue;
                    }
                    QString tr = ruledCardIdForName(stackCard->getName());
                    auto *entry = map->add_entries();
                    entry->set_player_id(pl->getPlayerId());
                    entry->set_engine_object_id(stackOid);
                    entry->set_card_id(tr.toStdString());
                    entry->set_ordinal(static_cast<uint32_t>(stackOrdinal));
                    entry->set_server_card_id(stackCard->getId());
                    entry->set_summoning_sick(false);
                    ++stackOrdinal;
                }
            }
        }
        // Only inject when we have something useful so trivial batches stay small.
        if (map->entries_size() > 0) {
            *toSend.mutable_batch()->add_events() = mapEvent;
        }
    }
    // zone_view hand/lib fields are cleared before broadcast; publish hand index <-> Server_Card.id separately for
    // ruled UI intents.
    {
        ruled::v1::RuledEvent handEv;
        auto *hm = handEv.mutable_hand_slot_map();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            Server_CardZone *handZone = pl->getZones().value(ZoneNames::HAND);
            if (!handZone) {
                continue;
            }
            const int pid = pl->getPlayerId();
            for (int i = 0; i < handZone->getCards().size(); ++i) {
                Server_Card *c = handZone->getCards().at(i);
                if (!c) {
                    continue;
                }
                auto *ent = hm->add_entries();
                ent->set_player_id(pid);
                ent->set_hand_index(static_cast<uint32_t>(i));
                ent->set_server_card_id(c->getId());
            }
        }
        *toSend.mutable_batch()->add_events() = handEv;
    }
    // Graveyard OID map: lets the client map engine OIDs in valid_graveyard_ids to
    // server card ids so graveyard cards can be clicked as spell targets.
    {
        ruled::v1::RuledEvent graveyardEv;
        auto *gm = graveyardEv.mutable_graveyard_object_map();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            const QHash<quint32, int> gravOidMap = playerBinding(pl->getPlayerId()).graveyardEngineOidToServerCardId;
            for (auto it = gravOidMap.constBegin(); it != gravOidMap.constEnd(); ++it) {
                auto *entry = gm->add_entries();
                entry->set_player_id(pl->getPlayerId());
                entry->set_engine_object_id(it.key());
                entry->set_server_card_id(it.value());
            }
        }
        if (gm->entries_size() > 0) {
            *toSend.mutable_batch()->add_events() = graveyardEv;
        }
    }
}

// Per-participant hidden-info redaction: keeps only the participant's own legal actions,
// drops LogMessage events not meant for them, and redacts/augments tier-3 resolution
// choice candidates by choice kind.
ruled::v1::RuledEventBatch RuledGameDriver::redactBatchForParticipant(const ruled::v1::RuledEventBatch &batch,
                                                                     Server_AbstractParticipant *participant)
{
    ruled::v1::RuledEventBatch filtered;
    filtered.CopyFrom(batch);
    filtered.clear_legal_by_player();
    const auto it = batch.legal_by_player().find(participant->getPlayerId());
    if (it != batch.legal_by_player().end()) {
        (*filtered.mutable_legal_by_player())[participant->getPlayerId()] = it->second;
    }
    // Drop LogMessage events directed at a different player or explicitly hidden from this one.
    for (int ei = filtered.events_size() - 1; ei >= 0; --ei) {
        const auto &ev = filtered.events(ei);
        if (!ev.has_log()) {
            continue;
        }
        const auto &log = ev.log();
        if (log.has_visible_to_player_id() && log.visible_to_player_id() != participant->getPlayerId()) {
            filtered.mutable_events()->DeleteSubrange(ei, 1);
        } else if (log.has_hidden_from_player_id() &&
                   log.hidden_from_player_id() == participant->getPlayerId()) {
            filtered.mutable_events()->DeleteSubrange(ei, 1);
        }
    }
    {
        // Redact private candidates of a tier-3 resolution choice (CR 608) from everyone but the
        // deciding player. Private kinds expose a concealed zone (see isPrivateChoiceKind):
        // HAND_CARDS reveals a player's hand, LIBRARY_SEARCH their library, OPPONENT_HAND another
        // player's hand, so only the decider sees the candidate object ids / names; the public
        // kinds (REVEALED, TARGET_OBJECTS, LEGEND_KEEP) pass through.
        // For HAND_CARDS, inject candidate_server_card_ids for the deciding player
        // so the client can map engine OIDs to physical hand CardItems for the hand-click UI.
        // For LIBRARY_SEARCH, inject by name-matching from the decider's deck zone
        // so the client can open the deck zone view and use deck-card click-to-pick (like Gifts Ungiven
        // search step). For REVEALED, inject from the non-deciding player's deck
        // so the client can render the revealed cards in a zone popup for the opponent's pick step.
        for (int ei = 0; ei < filtered.events_size(); ++ei) {
            if (!filtered.events(ei).has_resolution_choice_required()) {
                continue;
            }
            auto *rcr = filtered.mutable_events(ei)->mutable_resolution_choice_required();
            if (isPrivateChoiceKind(rcr->choice_kind()) && rcr->deciding_player_id() != participant->getPlayerId()) {
                rcr->clear_candidate_object_ids();
                rcr->clear_candidate_card_ids();
                rcr->clear_candidate_names();
                rcr->clear_candidate_server_card_ids();
                rcr->set_prompt_text("Opponent is making a resolution choice.");
            } else if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS) {
                // HandCards: populate server card ids so client hand-click UI can match engine OIDs.
                const int deciderId = rcr->deciding_player_id();
                auto *deciderPlayer = static_cast<Server_Player *>(game->getPlayers().value(deciderId));
                if (deciderPlayer) {
                    for (int ci = 0; ci < rcr->candidate_object_ids_size(); ++ci) {
                        const quint32 oid = static_cast<quint32>(rcr->candidate_object_ids(ci));
                        Server_Card *sc = playerBinding(deciderId).findCardByEngineOid(deciderPlayer, oid);
                        rcr->add_candidate_server_card_ids(sc ? sc->getId() : -1);
                    }
                }
            } else if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH) {
                // LibrarySearch: assign each candidate a sequential index as its server card ID.
                // Deck cards are not in engineOidToServerCardId (only battlefield/hand/stack are),
                // so there is no server-side lookup available. Sequential indices give every
                // physical card (including duplicate-named ones) a unique client-side ID.
                // The client maps index i → engine OID via candidate_object_ids[i].
                // NB: unique *within this candidate list only* — 0, 1, 2 … collide head-on with
                // the real Server_Card ids of cards in hand and on the battlefield. Any client
                // lookup keyed on these must first confirm the card is in the pick's own zone
                // (RuledActions::isResolutionPickZoneCard).
                for (int ci = 0; ci < rcr->candidate_names_size(); ++ci) {
                    rcr->add_candidate_server_card_ids(ci);
                }
            } else if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_REVEALED &&
                       rcr->candidate_server_card_ids_size() == 0) {
                // RevealedCards: same sequential-index scheme for the same reason.
                for (int ci = 0; ci < rcr->candidate_names_size(); ++ci) {
                    rcr->add_candidate_server_card_ids(ci);
                }
            } else if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND &&
                       rcr->candidate_server_card_ids_size() == 0) {
                // OpponentHand: reaches here only for the deciding player (non-deciders were
                // redacted above). The candidates live in another player's hidden hand, so — like
                // RevealedCards/LibrarySearch — use sequential indices the client maps back to OIDs.
                for (int ci = 0; ci < rcr->candidate_names_size(); ++ci) {
                    rcr->add_candidate_server_card_ids(ci);
                }
            }
        }
    }

    // HandSlotMap is recipient-private: retain only the recipient's physical hand ids.
    for (int ei = 0; ei < filtered.events_size(); ++ei) {
        if (!filtered.events(ei).has_hand_slot_map()) {
            continue;
        }
        auto *entries = filtered.mutable_events(ei)->mutable_hand_slot_map()->mutable_entries();
        for (int i = entries->size() - 1; i >= 0; --i) {
            if (entries->Get(i).player_id() != participant->getPlayerId()) {
                entries->DeleteSubrange(i, 1);
            }
        }
    }

    // Capture the explicitly authorized per-player values, clear every PER_PLAYER field
    // recursively by reflection (future classified fields therefore fail closed), then restore
    // only these reviewed cases.
    bool hasOwnLegalActions = false;
    ruled::v1::LegalActions ownLegalActions;
    const auto ownLegalIt = filtered.legal_by_player().find(participant->getPlayerId());
    if (ownLegalIt != filtered.legal_by_player().end()) {
        ownLegalActions.CopyFrom(ownLegalIt->second);
        hasOwnLegalActions = true;
    }
    QHash<int, QString> routedLogText;
    QHash<int, ruled::v1::ResolutionChoiceRequired> routedChoices;
    QHash<int, ruled::v1::HandSlotMap> ownHandSlotMaps;
    for (int ei = 0; ei < filtered.events_size(); ++ei) {
        const auto &event = filtered.events(ei);
        if (event.has_log()) {
            routedLogText.insert(ei, QString::fromStdString(event.log().text()));
        } else if (event.has_resolution_choice_required()) {
            routedChoices.insert(ei, event.resolution_choice_required());
        } else if (event.has_hand_slot_map()) {
            ownHandSlotMaps.insert(ei, event.hand_slot_map());
        }
    }

    clearRuledFieldsByVisibility(&filtered, ruled::v1::FIELD_VISIBILITY_PER_PLAYER);
    if (hasOwnLegalActions) {
        (*filtered.mutable_legal_by_player())[participant->getPlayerId()] = ownLegalActions;
    }
    for (auto logIt = routedLogText.constBegin(); logIt != routedLogText.constEnd(); ++logIt) {
        filtered.mutable_events(logIt.key())->mutable_log()->set_text(logIt.value().toStdString());
    }
    for (auto choiceIt = routedChoices.constBegin(); choiceIt != routedChoices.constEnd(); ++choiceIt) {
        auto *choice = filtered.mutable_events(choiceIt.key())->mutable_resolution_choice_required();
        choice->set_prompt_text(choiceIt.value().prompt_text());
        choice->mutable_candidate_object_ids()->CopyFrom(choiceIt.value().candidate_object_ids());
        choice->mutable_candidate_card_ids()->CopyFrom(choiceIt.value().candidate_card_ids());
        choice->mutable_candidate_names()->CopyFrom(choiceIt.value().candidate_names());
        choice->mutable_candidate_server_card_ids()->CopyFrom(choiceIt.value().candidate_server_card_ids());
    }
    for (auto handMapIt = ownHandSlotMaps.constBegin(); handMapIt != ownHandSlotMaps.constEnd(); ++handMapIt) {
        filtered.mutable_events(handMapIt.key())->mutable_hand_slot_map()->CopyFrom(handMapIt.value());
    }

    clearRuledFieldsByVisibility(&filtered, ruled::v1::FIELD_VISIBILITY_SERVER_ONLY);
    for (int ei = filtered.events_size() - 1; ei >= 0; --ei) {
        if (filtered.events(ei).ev_case() == ruled::v1::RuledEvent::EV_NOT_SET) {
            filtered.mutable_events()->DeleteSubrange(ei, 1);
        }
    }
    return filtered;
}

QList<QPair<int, QStringList>> RuledGameDriver::ruledMainboardNamesByPlayer() const
{
    QList<QPair<int, QStringList>> deckByPlayer;
    for (Server_AbstractPlayer *pl : game->getPlayers().values()) {
        QStringList mainboardNames;
        if (const DeckList *dl = pl->getDeckList()) {
            const QSet<QString> mainOnly = QSet<QString>() << QStringLiteral("main");
            for (const DecklistCardNode *node : dl->getCardNodes(mainOnly)) {
                if (!node) {
                    continue;
                }
                // Oracle names cross the IPC boundary; the engine owns name->id resolution.
                const QString name = node->getName().trimmed();
                for (int k = 0; k < node->getNumber(); ++k) {
                    mainboardNames.append(name);
                }
            }
        }
        deckByPlayer.append(qMakePair(pl->getPlayerId(), mainboardNames));
    }
    return deckByPlayer;
}

void RuledGameDriver::notifyRuledUnimplementedCards(const QList<QPair<int, QStringList>> &deckByPlayer,
                                                    const QStringList &missingNames)
{
    QSet<QString> missingLower;
    for (const QString &name : missingNames) {
        missingLower.insert(name.trimmed().toLower());
    }

    QStringList perPlayerParts;
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        QMap<QString, int> copiesByName; // QMap: alphabetical card order in the message
        for (const QString &name : row.second) {
            const QString trimmed = name.trimmed();
            if (missingLower.contains(trimmed.toLower())) {
                ++copiesByName[trimmed];
            }
        }
        if (copiesByName.isEmpty()) {
            continue;
        }
        Server_AbstractPlayer *pl = game->getPlayer(row.first);
        const QString playerName = pl ? QString::fromStdString(pl->getUserInfo()->name()) : QString::number(row.first);
        QStringList cardParts;
        for (auto it = copiesByName.constBegin(); it != copiesByName.constEnd(); ++it) {
            cardParts.append(it.value() > 1 ? QStringLiteral("%1 x%2").arg(it.key()).arg(it.value()) : it.key());
        }
        perPlayerParts.append(QStringLiteral("%1 (%2)").arg(cardParts.join(QStringLiteral(", ")), playerName));
    }
    if (perPlayerParts.isEmpty()) {
        // The engine reported names that match no gathered mainboard (shouldn't happen);
        // still surface them rather than blocking silently.
        perPlayerParts.append(missingNames.join(QStringLiteral(", ")));
    }

    const QString summary = QStringLiteral("Cannot start ruled game — unimplemented cards: %1. "
                                           "Swap to a fully implemented deck and ready up again.")
                                .arg(perPlayerParts.join(QStringLiteral("; ")));

    Event_GameSay say;
    say.set_message(summary.toStdString());
    game->sendGameEventContainer(game->prepareGameEvent(say, -1));

    Event_NotifyUser notify;
    notify.set_type(Event_NotifyUser::CUSTOM);
    notify.set_custom_title("Cannot start ruled game");
    notify.set_custom_content(summary.toStdString());
    for (Server_AbstractPlayer *pl : game->getPlayers().values()) {
        if (Server_AbstractUserInterface *ui = pl->getUserInterface()) {
            SessionEvent *se = Server_AbstractUserInterface::prepareSessionEvent(notify);
            ui->sendProtocolItem(*se);
            delete se;
        }
    }
}

void RuledGameDriver::sendRuledEngineNotice(const QString &title, const QString &message)
{
    Event_GameSay say;
    say.set_message(message.toStdString());
    game->sendGameEventContainer(game->prepareGameEvent(say, -1));

    Event_NotifyUser notify;
    notify.set_type(Event_NotifyUser::CUSTOM);
    notify.set_custom_title(title.toStdString());
    notify.set_custom_content(message.toStdString());
    for (Server_AbstractPlayer *pl : game->getPlayers().values()) {
        if (Server_AbstractUserInterface *ui = pl->getUserInterface()) {
            SessionEvent *se = Server_AbstractUserInterface::prepareSessionEvent(notify);
            ui->sendProtocolItem(*se);
            delete se;
        }
    }
}

void RuledGameDriver::notifyRuledEngineUnreachable()
{
    sendRuledEngineNotice(
        QStringLiteral("Cannot start ruled game"),
        QStringLiteral("Cannot start ruled game — the rules engine is unreachable. "
                       "Make sure the rules engine (tricerules) is running, then ready up again."));
}

void RuledGameDriver::handleRuledEngineConnectionLost()
{
    if (ruledEngineConnectionLost) {
        return;
    }
    ruledEngineConnectionLost = true;
    sendRuledEngineNotice(
        QStringLiteral("Rules engine disconnected"),
        QStringLiteral("The connection to the rules engine was lost — this ruled game can no longer "
                       "continue. The engine state cannot be recovered; please concede or leave the game."));
    // Drop the dead relay so further ruled commands fail fast (the !rulesRelay guards) instead of
    // re-timing-out on every command and re-notifying. A restarted sidecar is a fresh session.
    if (rulesRelay) {
        rulesRelay.reset();
    }
}

bool RuledGameDriver::startRuledSidecarSession()
{
    rulesRelay = std::make_unique<RulesRelay>(game);
    ruledSeed = QRandomGenerator::global()->generate64();
    // Test-only determinism hook: COCKATRICE_RULED_SEED pins the session seed so the whole
    // (seed, command log) event stream is reproducible for the E2E smoke test. Unset in production.
    {
        bool forcedOk = false;
        const quint64 forcedSeed = qEnvironmentVariable("COCKATRICE_RULED_SEED").toULongLong(&forcedOk);
        if (forcedOk) {
            ruledSeed = forcedSeed;
            qWarning() << "startRuledSidecarSession: using forced seed from COCKATRICE_RULED_SEED:" << ruledSeed;
        }
    }
    // Dev gate, half 1 of 2: ask the sidecar to accept debug cheat commands. It grants them only
    // if its own TRICERULES_DEV_COMMANDS is also set, so this flag alone cannot enable cheats —
    // that takes access to the machine running the engine. Unset in production.
    const QString devEnv = qEnvironmentVariable("COCKATRICE_RULED_DEV");
    const bool devCommandsRequested = devEnv == QLatin1String("1") || devEnv == QLatin1String("true");
    if (devCommandsRequested) {
        qWarning() << "startRuledSidecarSession: COCKATRICE_RULED_DEV set — requesting dev commands";
    }
    QList<int> ids;
    for (auto *p : game->getPlayers().values()) {
        ids.append(p->getPlayerId());
    }
    ruled::v1::IpcResponse resp;
    const QList<QPair<int, QStringList>> deckByPlayer = ruledMainboardNamesByPlayer();
    bool anyMainboard = false;
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        if (!row.second.isEmpty()) {
            anyMainboard = true;
            break;
        }
    }
    const QList<QPair<int, QStringList>> *deckPtr = anyMainboard ? &deckByPlayer : nullptr;
    if (!rulesRelay->sessionStart(static_cast<quint64>(game->getGameId()), ruledSeed, ids, deckPtr,
                                  devCommandsRequested, resp)) {
        qWarning() << "startRuledSidecarSession: tricerules connection failed";
        // The sidecar went away between pregame validation and SessionStart. We cannot run a
        // ruled game without it and cannot downgrade the already-ruled clients to casual, so
        // block the start (returning false unwinds it) and notify, matching the pregame gate.
        notifyRuledEngineUnreachable();
        rulesRelay.reset();
        return false;
    }
    if (!resp.ok()) {
        qWarning() << "startRuledSidecarSession: tricerules:" << QString::fromStdString(resp.error());
        if (!resp.missing_card_names().empty()) {
            // A deck changed between pregame validation and SessionStart: same block
            // path as the gate — never a silent casual fallback for unimplemented cards.
            QStringList missing;
            for (const std::string &name : resp.missing_card_names()) {
                missing.append(QString::fromStdString(name));
            }
            notifyRuledUnimplementedCards(deckByPlayer, missing);
            rulesRelay.reset();
            return false;
        }
        for (Server_AbstractPlayer *p : game->getPlayers().values()) {
            shuffleMainDeckForRuledFallback(p);
        }
        rulesRelay.reset();
        return true;
    }
    // Version handshake (Phase 5): log the sidecar's build + card-data hash, and warn if the
    // sidecar predates the handshake (empty fields) so build skew is visible. No refusal —
    // same-tree deploys are the norm and a mismatch is advisory.
    const QString engineBuild = QString::fromStdString(resp.engine_build());
    const QString cardDataHash = QString::fromStdString(resp.card_data_hash());
    if (engineBuild.isEmpty()) {
        qWarning() << "startRuledSidecarSession: sidecar reported no engine build / card-data hash"
                   << "— it predates the version handshake; rebuild servatrice and tricerules from the same tree";
    } else {
        qInfo() << "startRuledSidecarSession: tricerules engine" << engineBuild << "card data" << cardDataHash;
    }
    applyRuledStartupBatch(resp, deckByPlayer);
    if (!rulesRelay) {
        return true;
    }
    if (game->currentReplay) {
        game->currentReplay->set_ruled_seed(ruledSeed);
        // Stamp the card-data hash beside the seed so (seed, command log, hash) reproduces the replay.
        if (!cardDataHash.isEmpty()) {
            game->currentReplay->set_ruled_card_data_hash(cardDataHash.toStdString());
        }
    }
    broadcastRuledResponse(resp);
    return true;
}

QString RuledGameDriver::ruledCardIdForName(const QString &cardName) const
{
    return ruledCardIdByLowerName.value(cardName.trimmed().toLower());
}

QString RuledGameDriver::ruledCardNameForId(const QString &cardId) const
{
    const auto it = ruledCardCatalogById.constFind(cardId);
    return it == ruledCardCatalogById.constEnd() ? QString() : QString::fromStdString(it->name());
}

QString RuledGameDriver::ruledActiveFaceName(const QString &cardId, int faceIndex) const
{
    const auto it = ruledCardCatalogById.constFind(cardId);
    if (it == ruledCardCatalogById.constEnd()) {
        return QString();
    }
    // face_names is empty for single-face cards; the front face (0) uses the combined/base name.
    if (faceIndex > 0 && faceIndex < it->face_names_size()) {
        return QString::fromStdString(it->face_names(faceIndex));
    }
    return QString::fromStdString(it->name());
}

// Index every CardCatalog event in `batch` into the name/id lookups the zone reconcile resolves
// physical cards through. Returns true if the batch carried a catalog at all.
//
// A catalog event always carries the whole catalog, so a batch that has one fully replaces the
// index; a batch with none leaves it untouched. That distinction is why the clear is inside the
// loop rather than above it — most batches carry no catalog and must not wipe the index.
bool RuledGameDriver::indexCardCatalogEvents(const ruled::v1::RuledEventBatch &batch)
{
    bool sawCatalog = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_card_catalog()) {
            continue;
        }
        if (!sawCatalog) {
            ruledCardCatalogById.clear();
            ruledCardIdByLowerName.clear();
            sawCatalog = true;
        }
        for (const auto &entry : e.card_catalog().entries()) {
            const QString cardId = QString::fromStdString(entry.card_id());
            ruledCardCatalogById.insert(cardId, entry);
            ruledCardIdByLowerName.insert(QString::fromStdString(entry.name()).trimmed().toLower(), cardId);
            // CR 709/712/715: a multi-face card also resolves to its id by either face's Oracle
            // name (e.g. "Fire" / "Ice" -> "fire_ice"), mirroring the engine's name index.
            for (const auto &faceName : entry.face_names()) {
                ruledCardIdByLowerName.insert(QString::fromStdString(faceName).trimmed().toLower(), cardId);
            }
        }
    }
    return sawCatalog;
}

void RuledGameDriver::applyRuledStartupBatch(const ruled::v1::IpcResponse &resp,
                                             const QList<QPair<int, QStringList>> &deckByPlayer)
{
    if (!resp.has_batch()) {
        return;
    }

    // The catalog must be indexed before any zone-view application below: syncing
    // physical zones resolves card names through it.
    indexCardCatalogEvents(resp.batch());
    if (ruledCardCatalogById.isEmpty()) {
        qWarning() << "applyRuledStartupBatch: no CardCatalog in startup batch — "
                      "is tricerules-server rebuilt from this tree? Zone sync will not resolve names.";
    }

    int startupActivePlayer = -1;
    int startupMappedPhase = -1;
    int startupPriorityPlayer = -1;
    bool startupZoneViewApplied = false;
    for (int ei = 0; ei < resp.batch().events_size(); ++ei) {
        const auto &e = resp.batch().events(ei);
        if (e.has_phase_changed()) {
            startupActivePlayer = e.phase_changed().active_player_id();
            startupMappedPhase = ruledPhaseToCockatricePhase(e.phase_changed().phase_id());
        }
        if (e.has_priority_changed()) {
            startupPriorityPlayer = e.priority_changed().player_id();
        }
        if (e.has_zone_view() && !startupZoneViewApplied) {
            const auto &z = e.zone_view();
            for (int pi = 0; pi < z.per_player_size(); ++pi) {
                const auto &p = z.per_player(pi);
                const int mainN = expectedMainboardSizeForStartupSync(game, p.player_id(), deckByPlayer);
                const int needLib = mainN - p.hand_cards_size();
                const int libCount = p.library_card_ids_size();
                if (libCount != needLib) {
                    qWarning() << "Ruled zone sync: player" << p.player_id() << "expected" << needLib
                               << "library card ids, library_card_ids has" << libCount
                               << "entries — is tricerules-server up to date? "
                                  "(RulesRelay read was fixed; rebuild + restart the Rust side from this repo.)";
                    for (Server_AbstractPlayer *pl : game->getPlayers().values()) {
                        shuffleMainDeckForRuledFallback(pl);
                    }
                    rulesRelay.reset();
                    return;
                }
            }
            for (const auto &p : e.zone_view().per_player()) {
                if (Server_AbstractPlayer *ab = game->getPlayer(p.player_id())) {
                    playerBinding(p.player_id()).applyRuledEngineZoneView(static_cast<Server_Player *>(ab), p);
                }
            }
            startupZoneViewApplied = true;
        }
    }
    if (startupActivePlayer >= 0 && game->getActivePlayer() != startupActivePlayer) {
        game->setActivePlayer(startupActivePlayer);
    }
    if (startupMappedPhase >= 0 && game->getActivePhase() != startupMappedPhase) {
        game->setActivePhase(startupMappedPhase);
    }
    if (startupPriorityPlayer >= 0) {
        ruledPriorityPlayer = startupPriorityPlayer;
    }
}
