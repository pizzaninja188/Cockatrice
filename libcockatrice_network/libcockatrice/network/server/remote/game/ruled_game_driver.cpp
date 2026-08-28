// Fork-owned. See ruled_game_driver.h.

#include "ruled_game_driver.h"

#include "ruled_batch_synchronizer.h"
#include "ruled_broadcast_router.h"
#include "ruled_game_session.h"
#include "server_game.h"

#include <libcockatrice/protocol/pb/game_replay.pb.h>

RuledGameDriver::RuledGameDriver(Server_Game *_game)
    : game(_game), session(std::make_unique<RuledGameSession>(_game)),
      synchronizer(std::make_unique<RuledBatchSynchronizer>(_game, session.get())),
      broadcaster(std::make_unique<RuledBroadcastRouter>(_game, synchronizer.get()))
{
}

RuledGameDriver::~RuledGameDriver() = default;

int RuledGameDriver::priorityPlayer() const
{
    return synchronizer->priorityPlayer();
}

void RuledGameDriver::setPriorityPlayer(int playerId)
{
    synchronizer->setPriorityPlayer(playerId);
}

QString RuledGameDriver::ruledCardIdForName(const QString &cardName) const
{
    return synchronizer->cardIdForName(cardName);
}

QString RuledGameDriver::ruledCardNameForId(const QString &cardId) const
{
    return synchronizer->cardNameForId(cardId);
}

QString RuledGameDriver::ruledFaceDisplayName(const QString &cardId, int faceIndex) const
{
    return synchronizer->faceDisplayName(cardId, faceIndex);
}

void RuledGameDriver::revealFaceDownPermanentsOnConcede(int concedingPlayerId, GameEventStorage &events)
{
    synchronizer->revealFaceDownPermanentsOnConcede(concedingPlayerId, events);
}

bool RuledGameDriver::cacheAutoPassPolicy(int playerId, const ruled::v1::SetAutoPassPolicy &policy)
{
    return session->cacheAutoPassPolicy(playerId, policy);
}

QByteArray RuledGameDriver::canonicalGameplayCommand(int playerId, const ruled::v1::RuledCommand &command) const
{
    return session->canonicalGameplayCommand(playerId, command);
}

void RuledGameDriver::insertParticipantForTest(int id, Server_AbstractParticipant *participant)
{
    game->participants.insert(id, participant);
}

bool RuledGameDriver::validateDecksForStart()
{
    return session->validateDecksForStart();
}

void RuledGameDriver::resetForNewGame()
{
    // Per-player engine identity maps are rebuilt from the new session's zone views. Carrying
    // them across games republishes dead Server_Card ids (notably in GraveyardObjectMap, which
    // is only re-sent when non-empty) and lets stale oids resolve to cards that no longer exist.
    synchronizer->resetForNewGame();
    session->resetForNewGame();
    broadcaster->resetForNewGame();
}

void RuledGameDriver::endSidecarSession()
{
    session->end();
}

Response::ResponseCode
RuledGameDriver::processRuledPayload(int playerId, const Command_RuledPayload &cmd, GameEventStorage & /*ges*/)
{
    ruled::v1::RuledCommand ruledCmd;
    if (!ruledCmd.ParseFromString(cmd.payload())) {
        return Response::RespInvalidCommand;
    }
    if (ruledCmd.has_set_auto_pass_policy()) {
        return cacheAutoPassPolicy(playerId, ruledCmd.set_auto_pass_policy()) ? Response::RespOk
                                                                              : Response::RespContextError;
    }
    if (ruledCmd.has_canonical_gameplay()) {
        // The canonical envelope is a trusted Servatrice->engine/replay shape, never client input.
        return Response::RespInvalidCommand;
    }
    if (ruledCmd.has_preview_declare_blockers()) {
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
        const auto &pairs = ruledCmd.preview_declare_blockers();
        for (int pi = 0; pi < pairs.block_pairs_size(); ++pi) {
            const auto &pr = pairs.block_pairs(pi);
            auto *out = bpMsg->add_block_pairs();
            out->set_attacker_id(pr.attacker_id());
            out->set_blocker_id(pr.blocker_id());
        }
        broadcastRuledResponse(previewResp, false);
        return Response::RespOk;
    }
    if (ruledCmd.has_preview_declare_attackers()) {
        constexpr int declareAttackersToolbarPhase = 5;
        if (game->getActivePhase() != declareAttackersToolbarPhase || game->getActivePlayer() < 0 ||
            playerId != game->getActivePlayer()) {
            return Response::RespContextError;
        }
        ruled::v1::IpcResponse previewResp;
        previewResp.set_ok(true);
        auto *apMsg = previewResp.mutable_batch()->add_events()->mutable_attackers_preview();
        apMsg->set_declaring_player_id(playerId);
        const auto &preview = ruledCmd.preview_declare_attackers();
        for (const auto &assignment : preview.assignments()) {
            *apMsg->add_assignments() = assignment;
        }
        broadcastRuledResponse(previewResp, false);
        return Response::RespOk;
    }
    if (!session->isActive()) {
        return Response::RespInvalidCommand;
    }

    if (ruledCmd.has_preview_spell_payment()) {
        ruled::v1::IpcResponse response;
        if (!session->previewSpellPayment(playerId, ruledCmd.preview_spell_payment(), response)) {
            handleRuledEngineConnectionLost();
            return Response::RespInternalError;
        }
        if (!response.ok() || !response.has_batch() || !response.batch().has_spell_payment_preview())
            return Response::RespContextError;
        broadcaster->sendSpellPaymentPreview(playerId, response.batch().spell_payment_preview());
        return Response::RespOk;
    }
    const QByteArray payload = canonicalGameplayCommand(playerId, ruledCmd);
    if (payload.isEmpty()) {
        return Response::RespInvalidCommand;
    }
    ruled::v1::IpcResponse resp;
    if (!session->playerCommand(playerId, payload, resp)) {
        // Relay (not the engine) failed: the sidecar connection dropped mid-game. Tell the
        // players why the game has frozen rather than returning a silent internal error.
        handleRuledEngineConnectionLost();
        return Response::RespInternalError;
    }
    if (!resp.ok()) {
        return Response::RespContextError;
    }
    synchronizer->applyAcceptedCommandVisuals(playerId, ruledCmd);
    const RuledBatchSynchronizer::BatchApplyResult batchResult = synchronizer->applyBatch(resp);
    if ((batchResult.zoneViewApplied && (batchResult.handOrLibraryChanged || batchResult.battlefieldOrderChanged || batchResult.publicZoneOrderChanged)) ||
        batchResult.battlefieldDisplayChanged) {
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
    if (!session->isActive() || ruledCmdBytes.isEmpty()) {
        return;
    }
    ruled::v1::RuledCommand command;
    if (!command.ParseFromArray(ruledCmdBytes.constData(), ruledCmdBytes.size())) {
        return;
    }
    const QByteArray canonicalBytes = canonicalGameplayCommand(playerId, command);
    if (canonicalBytes.isEmpty()) {
        return;
    }
    ruled::v1::IpcResponse resp;
    if (!session->playerCommand(playerId, canonicalBytes, resp)) {
        // Relay (not the engine) failed: the sidecar connection dropped mid-game.
        handleRuledEngineConnectionLost();
        return;
    }
    if (!resp.ok()) {
        return;
    }
    const RuledBatchSynchronizer::BatchApplyResult batchResult = synchronizer->applyBatch(resp);
    if ((batchResult.zoneViewApplied && (batchResult.handOrLibraryChanged || batchResult.battlefieldOrderChanged || batchResult.publicZoneOrderChanged)) ||
        batchResult.battlefieldDisplayChanged) {
        game->sendGameStateToPlayers();
    }
    if (game->currentReplay) {
        game->currentReplay->mutable_ruled_command_log()->append(canonicalBytes.constData(),
                                                                 static_cast<size_t>(canonicalBytes.size()));
    }
    broadcastRuledResponse(resp);
}

void RuledGameDriver::broadcastRuledResponse(const ruled::v1::IpcResponse &response, bool authoritative)
{
    broadcaster->broadcast(response, authoritative);
}

void RuledGameDriver::enqueuePendingResolutionChoiceForParticipant(Server_AbstractParticipant *participant,
                                                                   ResponseContainer &response)
{
    broadcaster->enqueuePendingResolutionChoiceForParticipant(participant, response);
}

void RuledGameDriver::handleRuledEngineConnectionLost()
{
    session->handleConnectionLost();
}

bool RuledGameDriver::startRuledSidecarSession()
{
    const RuledGameSession::StartResult result = session->start();
    if (result.disposition == RuledGameSession::StartDisposition::Blocked) {
        return false;
    }
    if (result.disposition == RuledGameSession::StartDisposition::Fallback) {
        return true;
    }
    synchronizer->applyStartupBatch(result.response, result.deckByPlayer);
    if (!session->isActive()) {
        return true;
    }
    if (game->currentReplay) {
        game->currentReplay->set_ruled_seed(result.seed);
        // Stamp the card-data hash beside the seed so (seed, command log, hash) reproduces the replay.
        if (!result.cardDataHash.isEmpty()) {
            game->currentReplay->set_ruled_card_data_hash(result.cardDataHash.toStdString());
        }
    }
    broadcastRuledResponse(result.response);
    return true;
}
