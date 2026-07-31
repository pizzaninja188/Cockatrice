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

    /// Bind `engineOid` to `serverCardId` outside a zone-view sync.
    ///
    /// Needed when a permanent arrives on this seat's table from another seat (control change):
    /// the oid is not in this binding until the next sync, and `applyRuledEngineZoneView`'s
    /// fallback for an unknown slot is to match by card_id — which, with two identical
    /// permanents, can hand this slot the card belonging to the other one and silently swap the
    /// pairing. Registering the pair up front means the sync recognises the slot.
    void registerEngineOid(quint32 engineOid, int serverCardId)
    {
        engineOidToServerCardId.insert(engineOid, serverCardId);
        serverCardIdToEngineOid.insert(serverCardId, engineOid);
    }

    /// `engineUntappedOids` (may be null) carries the batch's `PermanentsUntapped` object ids —
    /// permanents the engine genuinely untapped (CR 701.20). Those are applied to the client even
    /// when `allowUntapReset` is false, which is what makes an untap *effect* visible mid-turn.
    RuledZoneSyncResult applyRuledEngineZoneView(Server_Player *player,
                                                 const ruled::v1::RuledPerPlayerView &v,
                                                 GameEventStorage *tapGes = nullptr,
                                                 bool allowUntapReset = true,
                                                 const QSet<quint32> *engineUntappedOids = nullptr);
    // CR 111: mint a physical token Server_Card on the player's table from an engine
    // TokenCreated identity (tokens have no deck card / Oracle entry) and bind it to `engineOid`
    // so the following zone-view sync matches the engine battlefield slot to it. The token is
    // marked destroy-on-zone-change so it disappears client-side when the engine moves it off the
    // battlefield (CR 111.7). Broadcasts an Event_CreateToken via `ges`.
    void createRuledToken(Server_Player *player,
                          quint32 engineOid,
                          const ruled::v1::TokenIdentity &identity,
                          GameEventStorage &ges);
    // Mint a physical Server_Card for a dev-conjured card (see DevCardConjured) into the hand or
    // the table, binding it to `engineOid` the same way createRuledToken does — the zone-view sync
    // later in this batch must find a physical card for the engine's new slot, or it abandons the
    // whole reconcile with only a warning. Returns true if a card was created.
    //
    // Unlike a token this is a real card: no "Token" annotation and no destroy-on-zone-change, so
    // it survives leaving the battlefield exactly as its engine object does (CR 111.7 applies to
    // tokens only). Display data (art, P/T, types) comes from the client's Oracle database by
    // name, so unlike TokenIdentity nothing has to be described inline.
    //
    // Only a table conjure enqueues a creation event. A hand conjure deliberately enqueues
    // nothing: Event_CreateToken's plain path broadcasts to every player, which would reveal the
    // conjured card to the opponent. The caller forces the ordinary full-state resync instead,
    // which redacts private zones per recipient.
    bool createRuledDevCard(Server_Player *player,
                            quint32 engineOid,
                            const QString &cardName,
                            bool isCreature,
                            bool toBattlefield,
                            GameEventStorage &ges);
};

#endif
