#ifndef PLAYER_H
#define PLAYER_H

#include "server_abstract_player.h"
#include <QHash>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

class Server_Player : public Server_AbstractPlayer
{
    Q_OBJECT
private:
    QMap<int, Server_Counter *> counters;
    QList<int> lastDrawList;
    // Latest mapping between engine ObjectIds (parallel to RuledPerPlayerView::battlefield)
    // and the corresponding Server_Card on this player's TABLE zone. Updated each
    // applyRuledEngineZoneView; consumed by Server_Game::applyRuledBatch when translating
    // engine-side combat events into client-visible Cockatrice events.
    QHash<quint32, int> engineOidToServerCardId;
    QHash<int, quint32> serverCardIdToEngineOid;
    QHash<quint32, bool> engineOidToSummoningSick;

public:
    struct RuledZoneSyncResult
    {
        bool handOrLibraryChanged = false;
        bool tapStateChanged = false;
        // engine_oid -> Server_Card.id, captured this sync. Empty when sync failed.
        QHash<quint32, int> engineOidToServerCardId;
    };

    QHash<quint32, int> getEngineOidToServerCardId() const
    {
        return engineOidToServerCardId;
    }
    bool isEngineOidSummoningSick(quint32 engineOid) const
    {
        return engineOidToSummoningSick.value(engineOid, false);
    }
    Server_Card *findCardByEngineOid(quint32 engineOid) const;

    Server_Player(Server_Game *_game,
                  int _playerId,
                  const ServerInfo_User &_userInfo,
                  bool _judge,
                  Server_AbstractUserInterface *_handler);
    ~Server_Player() override;
    const QMap<int, Server_Counter *> &getCounters() const
    {
        return counters;
    }
    int newCounterId() const;
    void addCounter(Server_Counter *counter);

    void setupZones() override;
    void clearZones() override;
    RuledZoneSyncResult applyRuledEngineZoneView(const ruled::v1::RuledPerPlayerView &v,
                                                 GameEventStorage *tapGes = nullptr);
    void shuffleMainDeckForRuledFallback();

    Response::ResponseCode drawCards(GameEventStorage &ges, int number);
    void onCardBeingMoved(GameEventStorage &ges,
                          const MoveCardStruct &cardStruct,
                          Server_CardZone *startzone,
                          Server_CardZone *targetzone,
                          bool undoingDraw) override;

    Response::ResponseCode
    cmdDeckSelect(const Command_DeckSelect &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdSetSideboardPlan(const Command_SetSideboardPlan &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdSetSideboardLock(const Command_SetSideboardLock &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdShuffle(const Command_Shuffle &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdMulligan(const Command_Mulligan &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdDrawCards(const Command_DrawCards &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdUndoDraw(const Command_UndoDraw &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdIncCounter(const Command_IncCounter &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdCreateCounter(const Command_CreateCounter &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdSetCounter(const Command_SetCounter &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdDelCounter(const Command_DelCounter &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdNextTurn(const Command_NextTurn &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdSetActivePhase(const Command_SetActivePhase &cmd, ResponseContainer &rc, GameEventStorage &ges) override;
    Response::ResponseCode
    cmdReverseTurn(const Command_ReverseTurn & /*cmd*/, ResponseContainer & /*rc*/, GameEventStorage &ges) override;
    Response::ResponseCode cmdChangeZoneProperties(const Command_ChangeZoneProperties &cmd,
                                                   ResponseContainer &rc,
                                                   GameEventStorage &ges) override;

    void getInfo(ServerInfo_Player *info,
                 Server_AbstractParticipant *playerWhosAsking,
                 bool omniscient,
                 bool withUserInfo) override;
};

#endif
