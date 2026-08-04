// Fork-owned. See ruled_player_binding.h. This code was extracted verbatim from
// server_player.cpp (roadmap Step 4); the Server_Player is reached through its public
// interface plus the `friend struct RuledPlayerBinding` grant on Server_AbstractPlayer
// (covers the protected sendCreateTokenEvents token-broadcast helper).

#include "ruled_player_binding.h"

#include "../server_response_containers.h"
#include "ruled_game_driver.h"
#include "server_card.h"
#include "server_cardzone.h"
#include "server_game.h"
#include "server_player.h"

#include <QDebug>
#include <QStringList>
#include <libcockatrice/protocol/pb/card_attributes.pb.h>
#include <libcockatrice/protocol/pb/event_set_card_attr.pb.h>
#include <libcockatrice/utility/zone_names.h>

namespace
{
QString stripRuledDamageLine(const QString &ann)
{
    if (ann.isEmpty()) {
        return ann;
    }
    const QString marker = QStringLiteral("Ruled Dmg:");
    QStringList kept;
    for (const QString &line : ann.split(QLatin1Char('\n'))) {
        const QString trimmed = line.trimmed();
        if (trimmed.startsWith(marker)) {
            continue;
        }
        kept.append(line);
    }
    return kept.join(QLatin1Char('\n')).trimmed();
}

QString mergeRuledDamageIntoAnnotation(const QString &baseAnn, uint32_t damage)
{
    QString without = stripRuledDamageLine(baseAnn);
    if (damage == 0) {
        return without;
    }
    const QString dmgLine = QStringLiteral("Ruled Dmg: %1").arg(damage);
    if (without.isEmpty()) {
        return dmgLine;
    }
    return without + QLatin1Char('\n') + dmgLine;
}

// Engine-emitted counter lines end with "counter(s)" (e.g. "1 +1/+1 counter(s)"). Strip any such
// line so a stale counter annotation never lingers after counters change or are removed.
QString stripRuledCounterLines(const QString &ann)
{
    if (ann.isEmpty()) {
        return ann;
    }
    const QString suffix = QStringLiteral("counter(s)");
    QStringList kept;
    for (const QString &line : ann.split(QLatin1Char('\n'))) {
        if (line.trimmed().endsWith(suffix)) {
            continue;
        }
        kept.append(line);
    }
    return kept.join(QLatin1Char('\n')).trimmed();
}

// `counterAnn` is the engine's per-permanent counter annotation (possibly multi-line, empty if none).
QString mergeRuledCountersIntoAnnotation(const QString &baseAnn, const QString &counterAnn)
{
    QString without = stripRuledCounterLines(baseAnn);
    if (counterAnn.isEmpty()) {
        return without;
    }
    if (without.isEmpty()) {
        return counterAnn;
    }
    return without + QLatin1Char('\n') + counterAnn;
}

// CR 108.3 vs CR 110.2: a permanent controlled by someone who does not own it needs to say whose
// card it is, or the board is unreadable after a reanimation. Same shape as the damage/counter
// lines: driven from the engine every sync, so it appears when control diverges and disappears
// again the moment the card goes home. `ownerName` empty means "owner == controller, strip it".
//
// This deliberately does not reuse upstream's Server_AbstractPlayer::onCardBeingMoved annotation
// (which ruled mode opts out of): that fires once at move time and never clears, so it would
// outlive the control change.
QString mergeRuledOwnerIntoAnnotation(const QString &baseAnn, const QString &ownerName)
{
    const QString marker = QStringLiteral("Owner: ");
    QStringList kept;
    for (const QString &line : baseAnn.split(QLatin1Char('\n'))) {
        if (line.trimmed().startsWith(marker)) {
            continue;
        }
        kept.append(line);
    }
    QString without = kept.join(QLatin1Char('\n')).trimmed();
    if (ownerName.isEmpty()) {
        return without;
    }
    const QString ownerLine = marker + ownerName;
    if (without.isEmpty()) {
        return ownerLine;
    }
    return without + QLatin1Char('\n') + ownerLine;
}
} // namespace

RuledPlayerBinding::RuledZoneSyncResult
RuledPlayerBinding::applyRuledEngineZoneView(Server_Player *player,
                                             const ruled::v1::RuledPerPlayerView &v,
                                             GameEventStorage *tapGes,
                                             bool allowUntapReset,
                                             const QSet<quint32> *engineUntappedOids)
{
    RuledZoneSyncResult result;
    const int playerId = player->getPlayerId();
    if (v.player_id() != playerId) {
        return result;
    }
    const QMap<QString, Server_CardZone *> &zones = player->getZones();
    Server_CardZone *deckZone = zones.value(ZoneNames::DECK);
    Server_CardZone *handZone = zones.value(ZoneNames::HAND);
    Server_CardZone *tableZone = zones.value(ZoneNames::TABLE);
    if (!deckZone || !handZone || !tableZone) {
        return result;
    }
    // Engine card ids come from the session catalog (engine-owned identity); the server
    // never derives ids from names itself. The ruled driver is always non-null here:
    // zone views only arrive in ruled games.
    Server_Game *game = player->getGame();
    const auto trId = [game](Server_Card *c) { return game->ruled()->ruledCardIdForName(c->getName()); };
    // The engine omits hand + library while they are unchanged, so the reconcile below — which
    // rebuilds a ~60-card pool and matches every engine card id against it — runs only on a batch
    // that actually moved a card into or out of one of those zones. The battlefield and graveyard
    // sections after it are unconditional: those are re-sent in full every view and the engine
    // ObjectId map is rebuilt from them.
    if (v.private_zones_unchanged()) {
        if (!privateZonesSynced) {
            qWarning() << "applyRuledEngineZoneView: player" << playerId
                       << "reports unchanged hand/library before any full sync — physical zones were "
                          "never seeded and will stay stale until the engine reports a change";
        }
    } else {
        QList<Server_Card *> pool;
        for (Server_Card *c : deckZone->getCards()) {
            pool.append(c);
        }
        for (Server_Card *c : handZone->getCards()) {
            pool.append(c);
        }
        QStringList libWants;
        for (const std::string &id : v.library_card_ids()) {
            if (!id.empty()) {
                libWants.append(QString::fromStdString(id));
            }
        }
        if (v.hand_cards_size() + libWants.size() != pool.size()) {
            qWarning() << "applyRuledEngineZoneView: count mismatch hand" << v.hand_cards_size() << "lib"
                       << libWants.size() << "pool" << pool.size() << "library_card_ids" << v.library_card_ids_size();
            return result;
        }
        QList<Server_Card *> handList;
        for (int i = 0; i < v.hand_cards_size(); ++i) {
            const QString want = QString::fromStdString(v.hand_cards(i).card_id());
            int found = -1;
            for (int j = 0; j < pool.size(); ++j) {
                if (trId(pool[j]) == want) {
                    found = j;
                    break;
                }
            }
            if (found < 0) {
                qWarning() << "applyRuledEngineZoneView: missing" << want << "player" << playerId;
                return result;
            }
            handList.append(pool.takeAt(found));
        }
        QList<Server_Card *> libList;
        for (const QString &want : libWants) {
            int found = -1;
            for (int j = 0; j < pool.size(); ++j) {
                if (trId(pool[j]) == want) {
                    found = j;
                    break;
                }
            }
            if (found < 0) {
                qWarning() << "applyRuledEngineZoneView: missing lib" << want;
                return result;
            }
            libList.append(pool.takeAt(found));
        }
        if (!pool.isEmpty()) {
            return result;
        }

        const QList<Server_Card *> currentHand = handZone->getCards();
        const QList<Server_Card *> currentDeck = deckZone->getCards();
        bool handMatches = (currentHand.size() == handList.size());
        if (handMatches) {
            for (int i = 0; i < currentHand.size(); ++i) {
                if (currentHand[i] != handList[i]) {
                    handMatches = false;
                    break;
                }
            }
        }
        bool deckMatches = (currentDeck.size() == libList.size());
        if (deckMatches) {
            for (int i = 0; i < currentDeck.size(); ++i) {
                if (currentDeck[i] != libList[i]) {
                    deckMatches = false;
                    break;
                }
            }
        }

        if (!handMatches || !deckMatches) {
            for (Server_Card *c : currentDeck) {
                deckZone->removeCard(c);
            }
            for (Server_Card *c : currentHand) {
                handZone->removeCard(c);
            }

            for (Server_Card *c : handList) {
                handZone->insertCard(c, -1, 0);
            }
            for (Server_Card *c : libList) {
                deckZone->insertCard(c, -1, 0);
            }
            result.handOrLibraryChanged = true;
        }
        // Every early return above leaves this false, so a failed reconcile keeps warning about
        // later omissions rather than pretending the zones were seeded.
        privateZonesSynced = true;
    }

    // Build an engine_oid -> Server_Card id map and (when permitted) sync tap state.
    // The map is rebuilt whenever the engine reports a battlefield, even if the caller
    // doesn't ask for tap propagation, so combat translation in the driver can rely on it.
    if (v.battlefield_objects_size() == tableZone->getCards().size()) {
        QList<Server_Card *> tablePool;
        tablePool.reserve(tableZone->getCards().size());
        for (Server_Card *c : tableZone->getCards()) {
            tablePool.append(c);
        }
        QList<Server_Card *> ordered;
        ordered.reserve(v.battlefield_objects_size());
        // Match engine slots to physical cards by stable engine ObjectId when possible.
        // Name-only matching mis-orders duplicates (e.g. two Forests), assigning the wrong
        // battlefield_tapped[] entry to each Server_Card and causing spurious tap/untap events.
        const QHash<int, quint32> prevServerCardIdToEngineOid = serverCardIdToEngineOid;

        for (int i = 0; i < v.battlefield_objects_size(); ++i) {
            const auto &battlefieldObject = v.battlefield_objects(i);
            const QString want = QString::fromStdString(battlefieldObject.card_id());
            const quint32 wantOid = static_cast<quint32>(battlefieldObject.object_id());
            int found = -1;
            if (wantOid != 0) {
                for (int j = 0; j < tablePool.size(); ++j) {
                    if (prevServerCardIdToEngineOid.value(tablePool[j]->getId()) == wantOid) {
                        found = j;
                        break;
                    }
                }
            }
            if (found < 0) {
                for (int j = 0; j < tablePool.size(); ++j) {
                    if (trId(tablePool[j]) == want) {
                        found = j;
                        break;
                    }
                }
            }
            if (found < 0) {
                ordered.clear();
                break;
            }
            ordered.append(tablePool.takeAt(found));
        }

        if (ordered.size() == v.battlefield_objects_size()) {
            // Determine expected row per card. Creatures → row 0 (top), lands → row 2
            // (bottom, preserved from play-land placement), other permanents → row 1 (middle).
            QList<int> expectedY;
            expectedY.reserve(ordered.size());
            for (int i = 0; i < ordered.size(); ++i) {
                const Server_Card *c = ordered[i];
                const int currentY = c ? c->getY() : 0;
                if (v.battlefield_objects(i).is_creature()) {
                    expectedY.append(0);
                } else if (currentY == 2) {
                    expectedY.append(2); // land — keep in bottom row
                } else {
                    expectedY.append(1); // noncreature nonland permanent
                }
            }

            const QList<Server_Card *> &zoneOrderBefore = tableZone->getCards();
            bool orderMismatch = false;
            if (zoneOrderBefore.size() == ordered.size()) {
                for (int i = 0; i < ordered.size(); ++i) {
                    if (zoneOrderBefore.at(i) != ordered[i] || (ordered[i] && ordered[i]->getY() != expectedY[i])) {
                        orderMismatch = true;
                        break;
                    }
                }
            } else {
                orderMismatch = true;
            }
            if (orderMismatch && tableZone->hasCoords()) {
                for (Server_Card *c : ordered) {
                    if (c) {
                        tableZone->removeCard(c);
                    }
                }
                for (int i = 0; i < ordered.size(); ++i) {
                    Server_Card *c = ordered[i];
                    if (!c) {
                        continue;
                    }
                    const int y = expectedY[i];
                    // An attached card (aura, equipment) carries x = -1 by upstream convention —
                    // see Server_AbstractPlayer::cmdAttachCard, which sets exactly that on attach.
                    // It is drawn against its parent rather than occupying a grid column, so handing
                    // it a real column here both steals a slot from unattached permanents and gives
                    // the client a stale grid position to render the card at.
                    const int x = c->getParentCard() ? -1 : tableZone->getFreeGridColumn(-1, y, c->getName(), y != 2);
                    tableZone->insertCard(c, x, y);
                }
                result.battlefieldOrderChanged = true;
            }

            engineOidToServerCardId.clear();
            serverCardIdToEngineOid.clear();
            engineOidToSummoningSick.clear();
            engineOidToHaste.clear();
            engineOidToTrample.clear();
            engineOidToCreature.clear();
            for (int i = 0; i < ordered.size(); ++i) {
                Server_Card *card = ordered[i];
                if (!card) {
                    continue;
                }
                const auto &battlefieldObject = v.battlefield_objects(i);
                const quint32 oid = static_cast<quint32>(battlefieldObject.object_id());
                engineOidToServerCardId.insert(oid, card->getId());
                serverCardIdToEngineOid.insert(card->getId(), oid);
                const bool summoningSick = battlefieldObject.summoning_sick();
                engineOidToSummoningSick.insert(oid, summoningSick);
                const auto hasKeyword = [&battlefieldObject](const char *keyword) {
                    for (const std::string &candidate : battlefieldObject.keywords()) {
                        if (candidate == keyword) {
                            return true;
                        }
                    }
                    return false;
                };
                const bool hasHaste = hasKeyword("Haste");
                engineOidToHaste.insert(oid, hasHaste);
                const bool hasTrample = hasKeyword("Trample");
                engineOidToTrample.insert(oid, hasTrample);
                const bool isCreatureFlag = battlefieldObject.is_creature();
                engineOidToCreature.insert(oid, isCreatureFlag);

                if (tapGes) {
                    const bool desiredTapped = battlefieldObject.tapped();
                    if (card->getTapped() != desiredTapped) {
                        // Do not force untap from engine during non-untap batches: Cockatrice may have
                        // tapped permanents for mana (or other UI) that the engine has not yet
                        // reflected in battlefield_tapped. Real untap-step sync is delivered in the
                        // same ruled batch as PhaseChanged(PHASE_ID_UNTAP) (see tricerules
                        // finish_cleanup_roll_new_turn).
                        //
                        // A permanent named by the batch's PermanentsUntapped event is exempt: the
                        // engine reported an actual CR 701.20 untap edge for it (untap effect,
                        // untap step, CR 605 mana undo), so there is no local tap to protect and
                        // suppressing it would leave the client drawing an untapped permanent
                        // sideways.
                        const bool engineUntappedIt = engineUntappedOids && engineUntappedOids->contains(oid);
                        if (!allowUntapReset && !engineUntappedIt && card->getTapped() && !desiredTapped) {
                            continue;
                        }
                        // Engine tap state is authoritative for ruled games (taps, and untaps
                        // during the untap step when allowUntapReset is true for the active player).
                        card->setTapped(desiredTapped);
                        result.tapStateChanged = true;
                        Event_SetCardAttr tapEv;
                        tapEv.set_zone_name(std::string(ZoneNames::TABLE));
                        tapEv.set_card_id(card->getId());
                        tapEv.set_attribute(AttrTapped);
                        tapEv.set_attr_value(desiredTapped ? "1" : "0");
                        tapGes->enqueueGameEvent(tapEv, playerId);
                    }
                }

                if (tapGes) {
                    const bool isCreature = battlefieldObject.is_creature();
                    const auto pwr = static_cast<uint32_t>(battlefieldObject.power());
                    const auto tgh = static_cast<uint32_t>(battlefieldObject.toughness());
                    const auto dmg = static_cast<uint32_t>(battlefieldObject.damage());

                    if (isCreature) {
                        const QString newPt = QStringLiteral("%1/%2").arg(pwr).arg(tgh);
                        if (card->getPT() != newPt) {
                            card->setPT(newPt);
                            result.tapStateChanged = true;
                            Event_SetCardAttr ptEv;
                            ptEv.set_zone_name(std::string(ZoneNames::TABLE));
                            ptEv.set_card_id(card->getId());
                            ptEv.set_attribute(AttrPT);
                            ptEv.set_attr_value(newPt.toStdString());
                            tapGes->enqueueGameEvent(ptEv, playerId);
                        }
                    }

                    const QString counterAnn = QString::fromStdString(battlefieldObject.counters_annotation());
                    // The permanent is listed in *this* seat's view, so this seat controls it
                    // (the engine battlefield list is the control index). Name the owner only
                    // when the two differ; an empty name strips any stale line.
                    QString ownerName;
                    if (battlefieldObject.owner_player_id() != playerId) {
                        if (Server_Game *g = player->getGame()) {
                            if (Server_AbstractPlayer *ownerPlayer =
                                    g->getPlayer(battlefieldObject.owner_player_id())) {
                                ownerName = QString::fromStdString(ownerPlayer->getUserInfo()->name());
                            }
                        }
                    }
                    QString mergedAnn = mergeRuledDamageIntoAnnotation(card->getAnnotation(), isCreature ? dmg : 0);
                    mergedAnn = mergeRuledCountersIntoAnnotation(mergedAnn, counterAnn);
                    mergedAnn = mergeRuledOwnerIntoAnnotation(mergedAnn, ownerName);
                    if (mergedAnn != card->getAnnotation()) {
                        card->setAnnotation(mergedAnn);
                        result.tapStateChanged = true;
                        Event_SetCardAttr annEv;
                        annEv.set_zone_name(std::string(ZoneNames::TABLE));
                        annEv.set_card_id(card->getId());
                        annEv.set_attribute(AttrAnnotation);
                        annEv.set_attr_value(mergedAnn.toStdString());
                        tapGes->enqueueGameEvent(annEv, playerId);
                    }
                }
            }
        }
    }

    // Hand OIDs (discard, bounce-to-hand, etc.): register after battlefield rebuild so
    // engineOidToServerCardId.clear() above does not drop them, and strip stale hand keys
    // before insert so moved cards do not leave orphan map entries.
    if (v.hand_cards_size() == handZone->getCards().size()) {
        for (Server_Card *hc : handZone->getCards()) {
            const int cid = hc->getId();
            const auto soIt = serverCardIdToEngineOid.constFind(cid);
            if (soIt != serverCardIdToEngineOid.constEnd()) {
                engineOidToServerCardId.remove(*soIt);
                serverCardIdToEngineOid.remove(cid);
            }
        }
        for (int i = 0; i < v.hand_cards_size(); ++i) {
            const quint32 oid = static_cast<quint32>(v.hand_cards(i).object_id());
            Server_Card *card = handZone->getCards().at(i);
            engineOidToServerCardId.insert(oid, card->getId());
            serverCardIdToEngineOid.insert(card->getId(), oid);
        }
    }

    // Graveyard OIDs: build a position-based map from the graveyard_object_id parallel array.
    // The two sides run in *opposite* directions and must be walked as such: the engine's
    // graveyard vector is oldest-first (each card is pushed on entry), while the physical
    // Cockatrice pile is newest-first (each arrival is inserted at position 0 so the pile renders
    // the most recent card — see PileZone::paint, which draws index 0). Pairing them by equal
    // index silently mismaps every card, which makes graveyard cards resolve to the wrong target
    // when clicked. Sizes still have to agree for positions to mean anything.
    Server_CardZone *graveZone = zones.value(ZoneNames::GRAVE);
    if (graveZone && v.graveyard_object_ids_size() == graveZone->getCards().size()) {
        graveyardEngineOidToServerCardId.clear();
        graveyardEngineOidsOldestFirst.clear();
        const int graveyardSize = v.graveyard_object_ids_size();
        for (int i = 0; i < graveyardSize; ++i) {
            const quint32 oid = static_cast<quint32>(v.graveyard_object_ids(i));
            Server_Card *card = graveZone->getCards().at(graveyardSize - 1 - i);
            graveyardEngineOidToServerCardId.insert(oid, card->getId());
            graveyardEngineOidsOldestFirst.append(oid);
        }
    }

    result.engineOidToServerCardId = engineOidToServerCardId;
    return result;
}

Server_Card *RuledPlayerBinding::findCardByEngineOid(const Server_Player *player, quint32 engineOid) const
{
    const auto it = engineOidToServerCardId.constFind(engineOid);
    if (it == engineOidToServerCardId.constEnd()) {
        return nullptr;
    }
    const int serverCardId = *it;
    for (const char *zn : {ZoneNames::TABLE, ZoneNames::HAND, ZoneNames::STACK}) {
        if (Server_CardZone *z = player->getZones().value(zn)) {
            if (Server_Card *c = z->getCard(serverCardId, nullptr, false)) {
                return c;
            }
        }
    }
    return nullptr;
}

Server_Card *RuledPlayerBinding::findGraveyardCardByEngineIndex(const Server_Player *player, int engineIndex) const
{
    if (engineIndex < 0 || engineIndex >= graveyardEngineOidsOldestFirst.size()) {
        return nullptr;
    }
    return findGraveyardCardByEngineOid(player, graveyardEngineOidsOldestFirst.at(engineIndex));
}

Server_Card *RuledPlayerBinding::findGraveyardCardByEngineOid(const Server_Player *player, quint32 engineOid) const
{
    const auto it = graveyardEngineOidToServerCardId.constFind(engineOid);
    if (it == graveyardEngineOidToServerCardId.constEnd()) {
        return nullptr;
    }
    const int serverCardId = *it;
    if (Server_CardZone *z = player->getZones().value(ZoneNames::GRAVE)) {
        return z->getCard(serverCardId, nullptr, false);
    }
    return nullptr;
}

void RuledPlayerBinding::createRuledToken(Server_Player *player,
                                          quint32 engineOid,
                                          const ruled::v1::TokenIdentity &identity,
                                          GameEventStorage &ges)
{
    Server_CardZone *table = player->getZones().value(ZoneNames::TABLE);
    if (!table) {
        return;
    }
    const QString name = QString::fromStdString(identity.name());
    // Creatures sit in the top row, other token permanents in the middle (mirrors the
    // creature/noncreature row split applyRuledEngineZoneView applies to deck-card permanents).
    int y = identity.is_creature() ? 0 : 1;
    int x = 0;
    if (table->hasCoords()) {
        x = table->getFreeGridColumn(-1, y, name, true);
    }
    if (x < 0) {
        x = 0;
    }

    auto *card = new Server_Card({name, QString()}, player->newCardId(), x, y);
    card->moveToThread(player->thread());
    card->setColor(QString::fromStdString(identity.color()));
    card->setPT(QString::fromStdString(identity.pt()));
    card->setTokenBasePt(QString::fromStdString(identity.pt()));
    QStringList keywords;
    keywords.reserve(identity.keywords_size());
    for (const auto &kw : identity.keywords()) {
        keywords.append(QString::fromStdString(kw));
    }
    card->setTokenAbilityKeywords(keywords);
    card->setAnnotation(QStringLiteral("Token"));
    // CR 111.7: when the engine later moves the token off the battlefield it ceases to exist;
    // destroy-on-zone-change makes the client drop the card the moment that move arrives.
    card->setDestroyOnZoneChange(true);
    table->insertCard(card, x, y);
    player->sendCreateTokenEvents(table, card, x, y, ges);

    // Pre-register the engine ObjectId <-> Server_Card binding so the zone-view sync that follows
    // in the same batch matches this engine battlefield slot to the freshly minted card (rather
    // than failing to find a physical card and aborting the reconcile).
    engineOidToServerCardId.insert(engineOid, card->getId());
    serverCardIdToEngineOid.insert(card->getId(), engineOid);
}

bool RuledPlayerBinding::createRuledDevCard(Server_Player *player,
                                            quint32 engineOid,
                                            const QString &cardName,
                                            bool isCreature,
                                            bool toBattlefield,
                                            GameEventStorage &ges)
{
    if (cardName.isEmpty()) {
        return false;
    }
    Server_CardZone *zone = player->getZones().value(toBattlefield ? ZoneNames::TABLE : ZoneNames::HAND);
    if (!zone) {
        return false;
    }

    if (!toBattlefield) {
        // Hand: append so the physical order matches the engine's (it pushes to the end of its own
        // hand vector too), which is what the zone reconcile's name matching lines up against.
        auto *card = new Server_Card({cardName, QString()}, player->newCardId(), 0, 0);
        card->moveToThread(player->thread());
        zone->insertCard(card, -1, 0);
        engineOidToServerCardId.insert(engineOid, card->getId());
        serverCardIdToEngineOid.insert(card->getId(), engineOid);
        return true;
    }

    // Table: same row split and grid placement createRuledToken uses, so a conjured permanent
    // lands where a legitimately played one would.
    int y = isCreature ? 0 : 1;
    int x = 0;
    if (zone->hasCoords()) {
        x = zone->getFreeGridColumn(-1, y, cardName, true);
    }
    if (x < 0) {
        x = 0;
    }
    auto *card = new Server_Card({cardName, QString()}, player->newCardId(), x, y);
    card->moveToThread(player->thread());
    zone->insertCard(card, x, y);
    player->sendCreateTokenEvents(zone, card, x, y, ges);
    engineOidToServerCardId.insert(engineOid, card->getId());
    serverCardIdToEngineOid.insert(card->getId(), engineOid);
    return true;
}
