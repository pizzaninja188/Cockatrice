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

    void sendPaymentPreview(int playerId, const ruled::v1::PaymentPreview &preview);
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
    // State accompanying a parked cast: replayable views and legality, never one-shot actions.
    ruled::v1::RuledEventBatch pendingResolutionState;
    // Caster-private engine transaction plus the public views required to rebuild payment after
    // reconnect. The fail-closed redactor removes the transaction for every other participant.
    ruled::v1::RuledEventBatch pendingSpellCastState;
    std::optional<ruled::v1::ZoneViewSync> currentPublicZoneView;
    // Opening legality/progress only; never replay one-shot logs or movement events.
    ruled::v1::RuledEventBatch pendingOpeningState;
};

#endif
