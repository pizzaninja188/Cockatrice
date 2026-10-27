#ifndef RULED_BROADCAST_ROUTER_H
#define RULED_BROADCAST_ROUTER_H

// Fork-owned recipient routing for ruled responses. This class injects server/engine identity
// maps, applies fail-closed protobuf visibility redaction, and retains only reconnect state.

#include "../server_response_containers.h"
#include "ruled_presentation_identity.h"

#include <QHash>
#include <QSet>
#include <QVector>
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

    bool sendPaymentPreview(int playerId, const ruled::v1::PaymentPreview &preview);
    bool canonicalizeClientCommand(int playerId, ruled::v1::RuledCommand &command);
    void resetForNewGame();
    void broadcast(const ruled::v1::IpcResponse &response, bool authoritative = true);
    void enqueuePendingResolutionChoiceForParticipant(Server_AbstractParticipant *participant, ResponseContainer &rc);

private:
    void appendServerObjectMaps(ruled::v1::IpcResponse &response);
    void appendFaceDownObjectMap(ruled::v1::IpcResponse &response);
    void updatePendingResolutionChoiceCache(const ruled::v1::IpcResponse &response);
    ruled::v1::RuledEventBatch redactBatchForParticipant(const ruled::v1::RuledEventBatch &batch,
                                                         Server_AbstractParticipant *participant);

    Server_Game *const game;
    RuledBatchSynchronizer *const synchronizer;
    RuledPresentationIdentity presentationIdentity;
    ruled::v1::HandSlotMap lastBroadcastHandSlotMap;
    bool hasLastBroadcastHandSlotMap = false;
    QSet<int> lastBroadcastHandSlotParticipants;
    std::optional<ruled::v1::ResolutionChoiceRequired> pendingResolutionChoice;
    // Authoritative prompt generation travels independently of the legacy RCR event. Retain its
    // final state for reconnects, including the inactive snapshot that retires a prior prompt.
    std::optional<ruled::v1::ResolutionPromptState> currentResolutionPromptState;
    // State accompanying a parked choice: replayable views and legality, never one-shot actions.
    ruled::v1::RuledEventBatch pendingResolutionState;
    std::optional<ruled::v1::ActivePublicRevealSnapshot> activePublicRevealSnapshot;
    // Player-private engine payment transaction plus the public views required to rebuild it after
    // reconnect. The fail-closed redactor removes each transaction for every other participant.
    ruled::v1::RuledEventBatch pendingAnnouncementState;
    std::optional<ruled::v1::ZoneViewSync> currentPublicZoneView;
    // The last public priority event is retained independently of per-seat legal offers.
    std::optional<ruled::v1::PriorityChanged> currentPriority;
    // Engine-published live stack presentations, in bottom-to-top order. They carry no private
    // legal actions or physical card bindings and are redacted/projected for each reconnect.
    QVector<ruled::v1::StackPushed> currentStackPresentations;
    // Latest engine-authorized top identity per player. It is never copied into the shared
    // public reconnect cache; redactBatchForParticipant restores only the matching recipient.
    QHash<int, ruled::v1::PrivateLibraryTopView> privateLibraryTopByPlayer;
    // A reconnecting seat may still have the last private top cached locally after its live
    // permission-loss batch was missed. Keep a participant-scoped full-replacement clear pending
    // until that seat receives an authoritative zone view.
    QSet<int> privateLibraryTopClearPendingByPlayer;
    // Fresh per-player actions are retained so an idle reconnect can restore library-top offers
    // together with the private top view; redaction still sends only the reconnecting seat's copy.
    QHash<int, ruled::v1::LegalActions> currentLegalActionsByPlayer;
    // Opening legality/progress only; never replay one-shot logs or movement events.
    ruled::v1::RuledEventBatch pendingOpeningState;
};

#endif
