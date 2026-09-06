#ifndef RULED_BATCH_SYNCHRONIZER_H
#define RULED_BATCH_SYNCHRONIZER_H

// Fork-owned physical projection of authoritative tricerules batches. This class owns every
// engine-oid/Server_Card binding and applies the documented batch passes synchronously, without
// deciding rules legality or participant visibility.

#include "../server_response_containers.h"
#include "ruled_player_binding.h"

#include <QHash>
#include <QList>
#include <QSet>
#include <QString>
#include <QVector>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

class RuledGameSession;
class Server_Card;
class Server_Game;

class RuledBatchSynchronizer
{
    friend class RuledBatchTest;
    friend class RuledBroadcastRouter;
    friend class RuledGameDriver;

public:
    struct BatchApplyResult
    {
        bool zoneViewApplied = false;
        bool handOrLibraryChanged = false;
        bool battlefieldOrderChanged = false;
        bool publicZoneOrderChanged = false;
        bool battlefieldDisplayChanged = false;
        bool tapStateEventsQueued = false;
        bool phaseChanged = false;
    };

    RuledBatchSynchronizer(Server_Game *game, RuledGameSession *session);

    void resetForNewGame();
    void applyAcceptedCommandVisuals(int playerId, const ruled::v1::RuledCommand &command);
    BatchApplyResult applyBatch(const ruled::v1::IpcResponse &response);
    void applyStartupBatch(const ruled::v1::IpcResponse &response, const QList<QPair<int, QStringList>> &deckByPlayer);
    void revealFaceDownPermanentsOnConcede(int concedingPlayerId, GameEventStorage &events);

    [[nodiscard]] int priorityPlayer() const;
    void setPriorityPlayer(int playerId);
    [[nodiscard]] QString cardIdForName(const QString &cardName) const;
    [[nodiscard]] QString cardNameForId(const QString &cardId) const;
    [[nodiscard]] QString faceDisplayName(const QString &cardId, int faceIndex) const;

private:
    struct PendingRuledCastVisual
    {
        QString cardName;
        int serverCardId = -1;
        int casterPlayerId = -1;
        QVector<quint32> targetOids;
    };

    bool indexCardCatalogEvents(const ruled::v1::RuledEventBatch &batch);
    void applyDevCardConjures(const ruled::v1::RuledEventBatch &batch,
                              const QHash<quint32, int> &battlefieldGridRows,
                              const QHash<quint32, int> &battlefieldDisplayPlayers,
                              BatchApplyResult &result);
    void applyTokenCreations(const ruled::v1::RuledEventBatch &batch, const QHash<quint32, int> &battlefieldGridRows);
    void applyPermanentMoves(const ruled::v1::RuledEventBatch &batch,
                             const QHash<int, QHash<quint32, int>> &preBatchOidMaps,
                             const QHash<quint32, int> &battlefieldGridRows,
                             const QHash<quint32, int> &battlefieldDisplayPlayers);
    void applyBattlefieldControllerTransfers(const ruled::v1::ZoneViewSync &zoneView, BatchApplyResult &result);
    void applyPhaseStackAndZoneViews(const ruled::v1::RuledEventBatch &batch,
                                     const QHash<quint32, int> &battlefieldGridRows,
                                     const QHash<quint32, int> &battlefieldDisplayPlayers,
                                     BatchApplyResult &result);
    void applyFaceDisplays(const ruled::v1::RuledEventBatch &batch, BatchApplyResult &result);
    Server_Card *findBattlefieldCardByEngineOid(quint32 oid, int preferredControllerId = -1);
    void applyAttachmentRestores(const ruled::v1::RuledEventBatch &batch);
    void applyLifeManaAndCombatEvents(const ruled::v1::RuledEventBatch &batch);
    void applyStackResolvedEvent(const ruled::v1::StackResolved &stackResolved,
                                 const QHash<quint32, int> &battlefieldGridRows,
                                 const QHash<quint32, int> &battlefieldDisplayPlayers);
    void applyStackObjectCounteredEvent(const ruled::v1::StackObjectCountered &countered);

    RuledPlayerBinding &playerBinding(int playerId);

    Server_Game *const game;
    RuledGameSession *const session;
    QHash<int, RuledPlayerBinding> playerBindings;
    int ruledPriorityPlayer = -1;
    QHash<QString, ruled::v1::CardCatalog_Entry> ruledCardCatalogById;
    QHash<QString, QString> ruledCardIdByLowerName;
    QHash<quint32, QString> ruledEngineStackPushDescriptionsByObjectId;
    QHash<quint32, int> ruledStackObjectIdToServerCardId;
    QHash<quint32, int> ruledStackObjectIdToCasterPlayerId;
    QHash<quint32, QVector<quint32>> ruledStackTargetsByObjectId;
    QSet<quint32> ruledStackCopyObjectIds;
    QList<PendingRuledCastVisual> ruledPendingCastVisualQueue;
};

#endif
