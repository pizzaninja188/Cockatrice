#ifndef RULED_GAME_DRIVER_H
#define RULED_GAME_DRIVER_H

// Fork-owned. All ruled-mode (server-authoritative rules engine) integration for one
// Server_Game lives here: the tricerules relay lifecycle, the engine card catalog,
// stack-object bookkeeping, batch application (engine events -> physical Cockatrice
// zones/events), and the two-stage broadcast redaction. Server_Game keeps only a
// `ruledGame` flag, the owning unique_ptr, and 1-line delegation hooks.
//
// See docs/ARCHITECTURE.md for the identity glossary (engine ObjectId vs tricerules card_id
// vs Oracle name vs Server_Card.id vs hand slot) and the end-to-end "life of a command" trace.
//
// --- The batch pipeline -------------------------------------------------------------------
//
// A command's IpcResponse becomes client events in two stages. Both are ordered; neither may
// be merged or reordered (see the comments on the individual methods for why each dependency
// exists).
//
// applyRuledBatch — engine events onto the physical Cockatrice game, six passes after a
// pre-pass that snapshots each player's engine_oid -> Server_Card.id map (the engine has
// already dropped dead permanents, so PermanentMoved needs the *prior* mapping):
//   0. indexCardCatalogEvents     refresh the name/id index if this batch carries a catalog
//                                 (dev conjuring does); everything below resolves names through it.
//   1. applyTokenCreations        CR 111: mint token Server_Cards, so the zone-view sync below
//                                 has something to bind the engine's battlefield slots to.
//   2. applyPermanentMoves        PermanentMoved -> moveCard, resolved through the pre-batch map.
//   3. applyPhaseStackAndZoneViews  phase/priority, stack push+resolve, and the per-player
//                                 RuledPerPlayerView reconciliation that rebuilds the identity
//                                 maps (must run after moves, before anything reading the maps).
//   4. applyAttachmentRestores    AuraAttached -> Event_AttachCard (CR 303.4).
//   5. applyLifeManaAndCombatEvents  life totals, mana-pool counters, combat declarations,
//                                 combat damage, removal-from-combat.
//
// broadcastRuledResponse — the same batch out to each participant, in two stages:
//   a. appendServerObjectMaps     inject the server-built BattlefieldObjectMap (battlefield +
//                                 stack), HandSlotMap and GraveyardObjectMap.
//   b. redactBatchForParticipant  per recipient: keep their own legal actions / log lines /
//                                 hand slots / choice candidates, then clear every PER_PLAYER
//                                 and SERVER_ONLY field by protobuf reflection and restore only
//                                 those reviewed values. Fail-closed: an unclassified field
//                                 fails the ruled_batch_test coverage check.

#include "../server_response_containers.h"
#include "ruled_player_binding.h"
#include "rules_relay.h"

#include <QByteArray>
#include <QHash>
#include <QList>
#include <QPair>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QVector>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

#include <memory>

class Server_Game;
class Server_AbstractParticipant;

/// One per ruled game, owned by (and friend of) Server_Game; non-null iff the game is ruled.
class RuledGameDriver
{
    // Test-only friend: lets the ruled-batch unit test reach the otherwise-private
    // catalog maps and applyRuledBatch entry point without going through the
    // network/userInterface plumbing required by Server_Game::addPlayer().
    friend class RuledBatchTest;

public:
    explicit RuledGameDriver(Server_Game *game);
    ~RuledGameDriver();

    Response::ResponseCode processRuledPayload(int playerId, const Command_RuledPayload &cmd, GameEventStorage &ges);
    /// Ruled mode: forward a serialized `ruled.v1.RuledCommand` to tricerules and broadcast the batch
    /// (used for mana-pool sync on land taps — not every payload goes through `Command_RuledPayload`).
    void relayRuledPayloadAndBroadcast(int playerId, const QByteArray &ruledCmdBytes);
    void broadcastRuledResponse(const ruled::v1::IpcResponse &resp);
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
    /// Ends the sidecar session and drops the relay (game teardown).
    void endSidecarSession();

    int priorityPlayer() const
    {
        return ruledPriorityPlayer;
    }
    void setPriorityPlayer(int playerId)
    {
        ruledPriorityPlayer = playerId;
    }

    /// Engine card id for an Oracle card name via the session catalog; empty when unknown
    /// (no catalog yet / card not in this game's decks).
    QString ruledCardIdForName(const QString &cardName) const;
    /// Oracle card name for an engine card id via the session catalog; empty when unknown.
    QString ruledCardNameForId(const QString &cardId) const;
    /// CR 712: Oracle name of a multi-face card's active face (0 = front/combined name). Used to
    /// display the correct face image for an MDFC/Transform/Flip permanent. Empty when unknown.
    QString ruledActiveFaceName(const QString &cardId, int faceIndex) const;

private:
    struct PendingRuledCastVisual
    {
        QString cardName;
        int serverCardId = -1;
        int casterPlayerId = -1;
        QVector<quint32> targetOids;
    };
    struct RuledBatchApplyResult
    {
        bool zoneViewApplied = false;
        bool handOrLibraryChanged = false;
        bool battlefieldOrderChanged = false;
        bool tapStateEventsQueued = false;
        bool phaseChanged = false;
    };

    /// Mainboard Oracle card names per player id, one entry per copy (the format both
    /// deck validation and the sidecar SessionStart consume).
    QList<QPair<int, QStringList>> ruledMainboardNamesByPlayer() const;
    /// Tells everyone a ruled game cannot start because of unimplemented cards: a game-log
    /// message plus a popup (Event_NotifyUser CUSTOM) to every player, naming the cards
    /// per player with copy counts.
    void notifyRuledUnimplementedCards(const QList<QPair<int, QStringList>> &deckByPlayer,
                                       const QStringList &missingNames);
    /// Sends a rules-engine notice to every player: a game-log message plus a popup
    /// (Event_NotifyUser CUSTOM) with the given title. Shared by the pregame-unreachable and
    /// mid-game-disconnect paths.
    void sendRuledEngineNotice(const QString &title, const QString &message);
    /// Tells everyone a ruled game cannot start because the rules engine (tricerules sidecar)
    /// is unreachable. The client commits to ruled-vs-freeform at join time, so a started game
    /// cannot be downgraded to casual mid-life — we block the start instead of half-starting.
    void notifyRuledEngineUnreachable();
    /// Handles the rules engine connection dropping during an active ruled game: notifies the
    /// players once (the game is unrecoverable) and tears down the dead relay so subsequent
    /// commands fail fast instead of re-timing-out and re-notifying. Idempotent.
    void handleRuledEngineConnectionLost();
    void applyRuledStartupBatch(const ruled::v1::IpcResponse &resp,
                                const QList<QPair<int, QStringList>> &deckByPlayer);
    RuledBatchApplyResult applyRuledBatch(const ruled::v1::IpcResponse &resp);
    // applyRuledBatch passes, in call order (order dependencies are load-bearing — see the
    // comment in applyRuledBatch; never merge or reorder these):
    /// Index every CardCatalog event in `batch` into the name/id lookups. Returns true if the
    /// batch carried one. Runs at startup and as the first batch pass, since the zone reconcile
    /// resolves physical cards by translating their names through this index.
    bool indexCardCatalogEvents(const ruled::v1::RuledEventBatch &batch);
    /// Mint physical Server_Cards for dev-conjured cards, which have no deck card behind them.
    /// Sets `result.handOrLibraryChanged` for a hand conjure, whose card is deliberately revealed
    /// by the redacted full-state resync rather than by a broadcast creation event.
    void applyDevCardConjures(const ruled::v1::RuledEventBatch &batch, RuledBatchApplyResult &result);
    void applyTokenCreations(const ruled::v1::RuledEventBatch &batch);
    void applyPermanentMoves(const ruled::v1::RuledEventBatch &batch,
                             const QHash<int, QHash<quint32, int>> &preBatchOidMaps);
    void applyPhaseStackAndZoneViews(const ruled::v1::RuledEventBatch &batch, RuledBatchApplyResult &result);
    void applyAttachmentRestores(const ruled::v1::RuledEventBatch &batch);
    void applyLifeManaAndCombatEvents(const ruled::v1::RuledEventBatch &batch);
    void applyRuledStackResolvedEvent(const ruled::v1::StackResolved &stackResolved);
    // broadcastRuledResponse stages: server-built identity-map injection, then per-participant
    // hidden-info redaction.
    void appendServerObjectMaps(ruled::v1::IpcResponse &toSend);
    ruled::v1::RuledEventBatch redactBatchForParticipant(const ruled::v1::RuledEventBatch &batch,
                                                         Server_AbstractParticipant *participant);

    /// Test-only: registers a participant directly on the game (bypassing addPlayer's
    /// network/userInterface plumbing) via the driver's Server_Game friendship.
    void insertParticipantForTest(int id, Server_AbstractParticipant *participant);

    /// Per-player ruled state (engine-oid identity maps); default-constructed on first access.
    RuledPlayerBinding &playerBinding(int playerId)
    {
        return playerBindings[playerId];
    }

    Server_Game *const game;
    QHash<int, RuledPlayerBinding> playerBindings;
    quint64 ruledSeed;
    int ruledPriorityPlayer;
    std::unique_ptr<RulesRelay> rulesRelay;
    /// Set once the rules engine connection drops during an active ruled game. The engine state
    /// is unrecoverable (a restarted sidecar is a fresh process with no session), so we notify
    /// the players exactly once and stop relaying further commands to a dead socket.
    bool ruledEngineConnectionLost = false;
    /// Engine-provided card identity catalog for this session (CardCatalog event, server-only):
    /// the single name<->id mapping — Servatrice never derives engine card ids itself.
    QHash<QString, ruled::v1::CardCatalog_Entry> ruledCardCatalogById;
    /// Trimmed, lowercased Oracle name -> engine card id (mirrors the engine's own normalization).
    QHash<QString, QString> ruledCardIdByLowerName;
    /// StackPushed.object_id -> engine card name; push and resolve may arrive in different ruled IPC batches.
    QHash<quint32, QString> ruledEngineStackPushDescriptionsByObjectId;
    // Stack object id -> Server_Card.id currently in the Cockatrice STACK zone.
    QHash<quint32, int> ruledStackObjectIdToServerCardId;
    /// Stack object id -> player who cast the spell (may differ from canonical stack zone owner).
    QHash<quint32, int> ruledStackObjectIdToCasterPlayerId;
    // Stack object id -> target engine object ids captured from CastSpell intent.
    QHash<quint32, QVector<quint32>> ruledStackTargetsByObjectId;
    /// Stack object ids that are spell *copies* (CR 707.10, StackPushed.is_copy). A copy has no
    /// physical Cockatrice card on the shared stack, so it is never bound to a Server_Card and its
    /// StackResolved is a no-op move — without this guard the copy (which shares the original's
    /// card_id/name) would mis-resolve the original's physical card.
    QSet<quint32> ruledStackCopyObjectIds;
    // Pending local cast intents waiting to be bound to the next StackPushed.object_id.
    QList<PendingRuledCastVisual> ruledPendingCastVisualQueue;
};

#endif
