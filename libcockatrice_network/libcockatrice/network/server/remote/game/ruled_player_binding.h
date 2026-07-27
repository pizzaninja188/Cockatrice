#ifndef RULED_PLAYER_BINDING_H
#define RULED_PLAYER_BINDING_H

// Fork-owned. Per-player ruled-mode state: the engine ObjectId <-> Server_Card
// identity maps plus the zone-view/token translation that maintains them. One
// binding per player, stored as QHash<int, RuledPlayerBinding> on RuledGameDriver
// (keyed by player id); methods take the Server_Player they act on as a parameter,
// which keeps upstream server_player.{h,cpp} free of ruled code and of the
// ruled_v1.pb.h include.

#include <QHash>
#include <QtGlobal>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

class GameEventStorage;
class Server_Card;
class Server_Player;

struct RuledPlayerBinding
{
    struct RuledZoneSyncResult
    {
        bool handOrLibraryChanged = false;
        bool tapStateChanged = false;
        /// TABLE card order was rewritten to match engine battlefield order.
        bool battlefieldOrderChanged = false;
        // engine_oid -> Server_Card.id, captured this sync. Empty when sync failed.
        QHash<quint32, int> engineOidToServerCardId;
    };

    // Latest mapping between engine ObjectIds in RuledPerPlayerView::battlefield_objects /
    // hand_cards and the corresponding Server_Card. Updated each
    // applyRuledEngineZoneView; consumed by RuledGameDriver::applyRuledBatch when translating
    // engine-side events into client-visible Cockatrice events.
    QHash<quint32, int> engineOidToServerCardId;
    QHash<int, quint32> serverCardIdToEngineOid;
    QHash<quint32, bool> engineOidToSummoningSick;
    QHash<quint32, bool> engineOidToHaste;
    QHash<quint32, bool> engineOidToTrample;
    QHash<quint32, bool> engineOidToCreature;
    // Parallel to engineOidToServerCardId but scoped to the graveyard zone.
    // Updated from RuledPerPlayerView::graveyard_object_ids each zone-view sync.
    QHash<quint32, int> graveyardEngineOidToServerCardId;

    bool isEngineOidSummoningSick(quint32 engineOid) const
    {
        return engineOidToSummoningSick.value(engineOid, false);
    }
    bool isEngineOidHaste(quint32 engineOid) const
    {
        return engineOidToHaste.value(engineOid, false);
    }
    bool isEngineOidTrample(quint32 engineOid) const
    {
        return engineOidToTrample.value(engineOid, false);
    }
    bool isEngineOidCreature(quint32 engineOid) const
    {
        return engineOidToCreature.value(engineOid, false);
    }
    Server_Card *findCardByEngineOid(const Server_Player *player, quint32 engineOid) const;
    Server_Card *findGraveyardCardByEngineOid(const Server_Player *player, quint32 engineOid) const;

    RuledZoneSyncResult applyRuledEngineZoneView(Server_Player *player,
                                                 const ruled::v1::RuledPerPlayerView &v,
                                                 GameEventStorage *tapGes = nullptr,
                                                 bool allowUntapReset = true);
    // CR 111: mint a physical token Server_Card on the player's table from an engine
    // TokenCreated identity (tokens have no deck card / Oracle entry) and bind it to `engineOid`
    // so the following zone-view sync matches the engine battlefield slot to it. The token is
    // marked destroy-on-zone-change so it disappears client-side when the engine moves it off the
    // battlefield (CR 111.7). Broadcasts an Event_CreateToken via `ges`.
    void createRuledToken(Server_Player *player,
                          quint32 engineOid,
                          const ruled::v1::TokenIdentity &identity,
                          GameEventStorage &ges);
};

#endif
