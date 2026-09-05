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
        bool publicZoneOrderChanged = false;
        // engine_oid -> Server_Card.id, captured this sync. Empty when sync failed.
        QHash<quint32, int> engineOidToServerCardId;
    };

    // Latest server-only mapping between engine ObjectIds in RuledPerPlayerView's battlefield,
    // hand, and library rows and the corresponding Server_Card. Updated each
    // applyRuledEngineZoneView; consumed by RuledBatchSynchronizer::applyBatch when translating
    // engine-side events into client-visible Cockatrice events.
    QHash<quint32, int> engineOidToServerCardId;
    QHash<int, quint32> serverCardIdToEngineOid;
    // Library identity is server-only and intentionally separate from the public/interactive
    // object map above. Server_Card ids are zone-scoped, so mixing deck ids into that reverse map
    // can shadow a battlefield card with the same numeric id.
    QHash<quint32, int> libraryEngineOidToServerCardId;
    QHash<int, quint32> libraryServerCardIdToEngineOid;
    // Engine hand order, retained from the latest full private-zone view. CastSpell names an
    // engine hand index; resolving it through this vector and the OID map avoids assuming the
    // physical Cockatrice hand happens to have the same positional order.
    QVector<quint32> handEngineOidsInOrder;
    QHash<quint32, bool> engineOidToSummoningSick;
    QHash<quint32, bool> engineOidToHaste;
    QHash<quint32, bool> engineOidToTrample;
    QHash<quint32, bool> engineOidToCreature;
    QHash<quint32, bool> engineOidToFaceDown;
    QHash<quint32, quint64> engineOidToZoneChangeGeneration;
    QHash<quint32, QString> engineOidToUnderlyingCardId;
    // Parallel to engineOidToServerCardId but scoped to the graveyard zone.
    // Updated from RuledPerPlayerView::graveyard_object_ids each zone-view sync.
    QHash<quint32, int> graveyardEngineOidToServerCardId;
    // Engine graveyard object ids in the engine's own order (oldest first). Kept alongside the
    // hash above because a command that names a graveyard *slot* (a flashback cast) has only the
    // index, and the physical pile runs the other way — see the ordering note in the .cpp.
    QVector<quint32> graveyardEngineOidsOldestFirst;
    // Public exile identity for Adventure and other engine-authorized casts from exile.
    QHash<quint32, int> exileEngineOidToServerCardId;
    // Whether a zone view has ever reconciled this player's hand and library. The engine omits
    // those two zones while they are unchanged (RuledPerPlayerView::private_zones_unchanged), so
    // an omission is only meaningful once a full snapshot has actually landed here — an
    // "unchanged" view arriving before that means the physical zones were never seeded and would
    // now stay stale silently. applyRuledEngineZoneView warns instead.
    bool privateZonesSynced = false;
    // Whether at least one complete battlefield replacement has been applied successfully.
    // `ZoneViewSync::battlefields_unchanged` is meaningful only after this seed exists.
    bool battlefieldSynced = false;
    // Presentation-only helper token for CR 702.195. It deliberately has no engine ObjectId and
    // is excluded from battlefield reconciliation; the engine stores only the player designation.
    int enduringStoryServerCardId = -1;
    // Presentation-only emblem markers keyed by engine runtime marker id. These cards are kept
    // outside the ordinary engine ObjectId map because emblems are not battlefield permanents.
    QHash<quint32, int> staticEmblemServerCardIds;

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
    bool isEngineOidFaceDown(quint32 engineOid) const
    {
        return engineOidToFaceDown.value(engineOid, false);
    }
    quint64 engineOidZoneChangeGeneration(quint32 engineOid) const
    {
        return engineOidToZoneChangeGeneration.value(engineOid, 0);
    }
    QString engineOidUnderlyingCardId(quint32 engineOid) const
    {
        return engineOidToUnderlyingCardId.value(engineOid);
    }
    Server_Card *findCardByEngineOid(const Server_Player *player, quint32 engineOid) const;
    Server_Card *findHandCardByEngineIndex(const Server_Player *player, int engineIndex) const;
    Server_Card *findGraveyardCardByEngineOid(const Server_Player *player, quint32 engineOid) const;
    Server_Card *findExileCardByEngineOid(const Server_Player *player, quint32 engineOid) const;
    /// Remove a stale physical parent-card relationship and restore the card to its table row,
    /// emitting the ordinary Cockatrice unattach/move events. RuledGameDriver decides from the
    /// authoritative typed recipient when this transition is required.
    void unattachRuledCard(Server_Player *player, Server_Card *card, GameEventStorage &ges);
    /// Resolve an *engine* graveyard index (as carried by CastSpell.hand_card_index on a flashback
    /// cast) to the physical card. Never index the pile with an engine index directly: the two
    /// zones run in opposite directions.
    Server_Card *findGraveyardCardByEngineIndex(const Server_Player *player, int engineIndex) const;

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
    void registerLibraryEngineOid(quint32 engineOid, int serverCardId)
    {
        libraryEngineOidToServerCardId.insert(engineOid, serverCardId);
        libraryServerCardIdToEngineOid.insert(serverCardId, engineOid);
    }

    /// `engineUntappedOids` (may be null) carries the batch's `PermanentsUntapped` object ids —
    /// permanents the engine genuinely untapped (CR 701.20). Those are applied to the client even
    /// when `allowUntapReset` is false, which is what makes an untap *effect* visible mid-turn.
    RuledZoneSyncResult applyRuledEngineZoneView(Server_Player *player,
                                                 const ruled::v1::RuledPerPlayerView &v,
                                                 GameEventStorage *tapGes = nullptr,
                                                 bool allowUntapReset = true,
                                                 const QSet<quint32> *engineUntappedOids = nullptr,
                                                 bool battlefieldsUnchanged = false);
    // CR 111: mint a physical token Server_Card on the player's table from an engine
    // TokenCreated identity (tokens have no deck card / Oracle entry) and bind it to `engineOid`
    // so the following zone-view sync matches the engine battlefield slot to it. The token is
    // marked destroy-on-zone-change so it disappears client-side when the engine moves it off the
    // battlefield (CR 111.7). Broadcasts an Event_CreateToken via `ges`.
    void createRuledToken(Server_Player *player,
                          quint32 engineOid,
                          const ruled::v1::TokenIdentity &identity,
                          int battlefieldGridY,
                          bool entersTapped,
                          GameEventStorage &ges);
    /// Materialize the public enduring-story designation as an ordinary battlefield token.
    /// Returns true only when a new token was created. `ges` may be null during startup
    /// restoration, when inserting the card before the full-state sync is enough.
    bool ensureEnduringStoryToken(Server_Player *player, int battlefieldGridY, GameEventStorage *ges);
    /// Reconcile the exact engine-authored emblem marker snapshot as table tokens. `ges` may be
    /// null during startup restoration; normal batches broadcast create/destroy events.
    bool reconcileStaticEmblemTokens(Server_Player *player,
                                     const ruled::v1::RuledPerPlayerView &view,
                                     int battlefieldGridY,
                                     GameEventStorage *ges);
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
                            int battlefieldGridY,
                            bool toBattlefield,
                            GameEventStorage &ges);
};

#endif
