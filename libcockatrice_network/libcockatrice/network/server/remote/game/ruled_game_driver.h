#ifndef RULED_GAME_DRIVER_H
#define RULED_GAME_DRIVER_H

// Fork-owned facade for one server-authoritative ruled game. It validates and canonicalizes a
// command through RuledGameSession, projects accepted batches through RuledBatchSynchronizer,
// and publishes recipient-safe responses through RuledBroadcastRouter. Server_Game keeps only
// the owning pointer and short delegation hooks.
//
// See docs/ARCHITECTURE.md for the identity glossary (engine ObjectId vs tricerules card_id
// vs Oracle name vs Server_Card.id vs hand slot), the end-to-end "life of a command" trace,
// and the authoritative batch-pipeline description in section 4.
//
// See docs/ARCHITECTURE.md section 4 for collaborator ownership and the load-bearing physical
// projection and broadcast pipelines.

#include "../server_response_containers.h"

#include <QByteArray>
#include <QString>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <memory>

class Server_Game;
class Server_AbstractParticipant;
class RuledBatchSynchronizer;
class RuledBroadcastRouter;
class RuledGameSession;

/// One per ruled game, owned by (and friend of) Server_Game; non-null iff the game is ruled.
class RuledGameDriver
{
    // Test-only friend: lets the ruled-batch fixture reach the owned collaborators and register
    // participants without Server_Game::addPlayer's network/userInterface plumbing.
    friend class RuledBatchTest;

public:
    explicit RuledGameDriver(Server_Game *game);
    ~RuledGameDriver();

    Response::ResponseCode processRuledPayload(int playerId, const Command_RuledPayload &cmd, GameEventStorage &ges);
    /// Ruled mode: forward a serialized `ruled.v1.RuledCommand` to tricerules and broadcast the batch
    /// (used for mana-pool sync on land taps — not every payload goes through `Command_RuledPayload`).
    void relayRuledPayloadAndBroadcast(int playerId, const QByteArray &ruledCmdBytes);
    void broadcastRuledResponse(const ruled::v1::IpcResponse &resp, bool authoritative = true);
    /// Returns false when the session was refused because a deck contains unimplemented
    /// cards (the game must not start; players were already notified). Infrastructure
    /// failures keep the legacy casual fallback and return true.
    bool startRuledSidecarSession();
    /// Pregame gate run from doStartGameIfReady: stateless ValidateDeck IPC over every
    /// mainboard. On failure notifies + un-readies the players and returns false (the
    /// start must not proceed; the pregame stays open so players can swap decks).
    bool validateDecksForStart();
    /// Clears per-game stack bookkeeping and the connection-lost flag before a (re)start.
    void resetForNewGame();
    /// Appends the sole current engine-authored resolution choice, redacted for this recipient,
    /// to a join/reconnect response. No-op when resolution is not parked on a choice.
    void enqueuePendingResolutionChoiceForParticipant(Server_AbstractParticipant *participant, ResponseContainer &rc);
    /// Ends the sidecar session and drops the relay (game teardown).
    void endSidecarSession();
    /// CR 708.9: reveal the conceding player's face-down permanents, or every remaining one when
    /// the concession ends the game, before ordinary teardown clears the table zones.
    void revealFaceDownPermanentsOnConcede(int concedingPlayerId, GameEventStorage &ges);

    int priorityPlayer() const;
    void setPriorityPlayer(int playerId);

    /// Engine card id for an Oracle card name via the session catalog; empty when unknown
    /// (no catalog yet / card not in this game's decks).
    QString ruledCardIdForName(const QString &cardName) const;
    /// Oracle card name for an engine card id via the session catalog; empty when unknown.
    QString ruledCardNameForId(const QString &cardId) const;
    /// Exact cards.xml name used to display a multi-face card in the selected face state. This is
    /// a face name for MDFC/Transform/Flip cards and the whole-card name for Adventure/Split.
    QString ruledFaceDisplayName(const QString &cardId, int faceIndex) const;

private:
    /// Handles the rules engine connection dropping during an active ruled game: notifies the
    /// players once (the game is unrecoverable) and tears down the dead relay so subsequent
    /// commands fail fast instead of re-timing-out and re-notifying. Idempotent.
    void handleRuledEngineConnectionLost();
    /// Validate and cache one authenticated player's UI-only phase-stop preferences. The command
    /// carries no player id; `playerId` always comes from the server-side participant binding.
    bool cacheAutoPassPolicy(int playerId, const ruled::v1::SetAutoPassPolicy &policy);
    /// Wrap one ordinary gameplay command with a sorted, complete server-authored policy snapshot.
    /// Returns empty for a UI-only/nested command or an unknown player.
    QByteArray canonicalGameplayCommand(int playerId, const ruled::v1::RuledCommand &command) const;
    /// Test-only: registers a participant directly on the game (bypassing addPlayer's
    /// network/userInterface plumbing) via the driver's Server_Game friendship.
    void insertParticipantForTest(int id, Server_AbstractParticipant *participant);

    Server_Game *const game;
    std::unique_ptr<RuledGameSession> session;
    std::unique_ptr<RuledBatchSynchronizer> synchronizer;
    std::unique_ptr<RuledBroadcastRouter> broadcaster;
};

#endif
