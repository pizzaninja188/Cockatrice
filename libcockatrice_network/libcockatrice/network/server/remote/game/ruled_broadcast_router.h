#ifndef RULED_BROADCAST_ROUTER_H
#define RULED_BROADCAST_ROUTER_H

// Fork-owned recipient routing for ruled responses. This class injects server/engine identity
// maps, applies fail-closed protobuf visibility redaction, and retains only reconnect state.

#include "../server_response_containers.h"

#include <QSet>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <optional>

class RuledBatchSynchronizer;
class Server_AbstractParticipant;
class Server_Game;

class RuledBroadcastRouter
{
    friend class RuledBatchTest;

public:
    RuledBroadcastRouter(Server_Game *game, RuledBatchSynchronizer *synchronizer);

    void sendSpellPaymentPreview(int playerId, const ruled::v1::SpellPaymentPreview &preview);
    void resetForNewGame();
    void broadcast(const ruled::v1::IpcResponse &response, bool authoritative = true);
    void enqueuePendingResolutionChoiceForParticipant(Server_AbstractParticipant *participant, ResponseContainer &rc);

private:
    void appendServerObjectMaps(ruled::v1::IpcResponse &response);
    void updatePendingResolutionChoiceCache(const ruled::v1::IpcResponse &response);
    ruled::v1::RuledEventBatch redactBatchForParticipant(const ruled::v1::RuledEventBatch &batch,
                                                         Server_AbstractParticipant *participant);

    Server_Game *const game;
    RuledBatchSynchronizer *const synchronizer;
    ruled::v1::HandSlotMap lastBroadcastHandSlotMap;
    bool hasLastBroadcastHandSlotMap = false;
    QSet<int> lastBroadcastHandSlotParticipants;
    std::optional<ruled::v1::ResolutionChoiceRequired> pendingResolutionChoice;
};

#endif
