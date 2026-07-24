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

namespace {
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
} // namespace

RuledPlayerBinding::RuledZoneSyncResult
RuledPlayerBinding::applyRuledEngineZoneView(Server_Player *player,
                                             const ruled::v1::RuledPerPlayerView &v,
                                             GameEventStorage *tapGes,
                                             bool allowUntapReset)
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
    QList<Server_Card *> pool;
    for (Server_Card *c : deckZone->getCards()) {
        pool.append(c);
    }
    for (Server_Card *c : handZone->getCards()) {
        pool.append(c);
    }
    QStringList libWants;
    for (const std::string &id : v.lib_ids()) {
        if (!id.empty()) {
            libWants.append(QString::fromStdString(id));
        }
    }
    if (v.hand_size() + libWants.size() != pool.size()) {
        qWarning() << "applyRuledEngineZoneView: count mismatch hand" << v.hand_size() << "lib" << libWants.size()
                   << "pool" << pool.size() << "lib_ids" << v.lib_ids_size();
        return result;
    }
    QList<Server_Card *> handList;
    for (int i = 0; i < v.hand_size(); ++i) {
        const QString want = QString::fromStdString(v.hand(i));
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

    // Build an engine_oid -> Server_Card id map and (when permitted) sync tap state.
    // The map is rebuilt whenever the engine reports a battlefield, even if the caller
    // doesn't ask for tap propagation, so combat translation in the driver can rely on it.
    if (v.battlefield_size() == tableZone->getCards().size() &&
        v.battlefield_size() == v.battlefield_object_id_size()) {
        QList<Server_Card *> tablePool;
        tablePool.reserve(tableZone->getCards().size());
        for (Server_Card *c : tableZone->getCards()) {
            tablePool.append(c);
        }
        QList<Server_Card *> ordered;
        ordered.reserve(v.battlefield_size());
        // Match engine slots to physical cards by stable engine ObjectId when possible.
        // Name-only matching mis-orders duplicates (e.g. two Forests), assigning the wrong
        // battlefield_tapped[] entry to each Server_Card and causing spurious tap/untap events.
        const QHash<int, quint32> prevServerCardIdToEngineOid = serverCardIdToEngineOid;

        for (int i = 0; i < v.battlefield_size(); ++i) {
            const QString want = QString::fromStdString(v.battlefield(i));
            const quint32 wantOid = static_cast<quint32>(v.battlefield_object_id(i));
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

        if (ordered.size() == v.battlefield_size()) {
            // Determine expected row per card. Creatures → row 0 (top), lands → row 2
            // (bottom, preserved from play-land placement), other permanents → row 1 (middle).
            const bool haveIsCreature = v.battlefield_is_creature_size() == v.battlefield_size();
            QList<int> expectedY;
            expectedY.reserve(ordered.size());
            for (int i = 0; i < ordered.size(); ++i) {
                const Server_Card *c = ordered[i];
                const int currentY = c ? c->getY() : 0;
                if (haveIsCreature && v.battlefield_is_creature(i)) {
                    expectedY.append(0);
                } else if (currentY == 2) {
                    expectedY.append(2); // land — keep in bottom row
                } else if (!haveIsCreature) {
                    expectedY.append(currentY); // no creature flags from engine — preserve row to avoid flicker
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
                    const int x = tableZone->getFreeGridColumn(-1, y, c->getName(), y != 2);
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
            const bool haveCreatureStats = v.battlefield_power_size() == v.battlefield_size() &&
                                           v.battlefield_toughness_size() == v.battlefield_size() &&
                                           v.battlefield_damage_size() == v.battlefield_size() &&
                                           v.battlefield_is_creature_size() == v.battlefield_size();
            for (int i = 0; i < ordered.size(); ++i) {
                Server_Card *card = ordered[i];
                if (!card) {
                    continue;
                }
                const quint32 oid = static_cast<quint32>(v.battlefield_object_id(i));
                engineOidToServerCardId.insert(oid, card->getId());
                serverCardIdToEngineOid.insert(card->getId(), oid);
                const bool summoningSick =
                    (i < v.battlefield_summoning_sick_size()) ? v.battlefield_summoning_sick(i) : false;
                engineOidToSummoningSick.insert(oid, summoningSick);
                const bool hasHaste = (i < v.battlefield_haste_size()) ? v.battlefield_haste(i) : false;
                engineOidToHaste.insert(oid, hasHaste);
                const bool hasTrample = (i < v.battlefield_trample_size()) ? v.battlefield_trample(i) : false;
                engineOidToTrample.insert(oid, hasTrample);
                const bool isCreatureFlag =
                    (i < v.battlefield_is_creature_size()) ? v.battlefield_is_creature(i) : false;
                engineOidToCreature.insert(oid, isCreatureFlag);

                if (tapGes && i < v.battlefield_tapped_size()) {
                    const bool desiredTapped = v.battlefield_tapped(i);
                    if (card->getTapped() != desiredTapped) {
                        // Do not force untap from engine during non-untap batches: Cockatrice may have
                        // tapped permanents for mana (or other UI) that the engine has not yet
                        // reflected in battlefield_tapped. Real untap-step sync is delivered in the
                        // same ruled batch as phase_changed("untap") (see tricerules finish_cleanup_roll_new_turn).
                        if (!allowUntapReset && card->getTapped() && !desiredTapped) {
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

                if (tapGes && haveCreatureStats) {
                    const bool isCreature = v.battlefield_is_creature(i);
                    const auto pwr = static_cast<uint32_t>(v.battlefield_power(i));
                    const auto tgh = static_cast<uint32_t>(v.battlefield_toughness(i));
                    const auto dmg = static_cast<uint32_t>(v.battlefield_damage(i));

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

                    const QString counterAnn = (i < v.battlefield_counters_annotation_size())
                                                   ? QString::fromStdString(v.battlefield_counters_annotation(i))
                                                   : QString();
                    QString mergedAnn = mergeRuledDamageIntoAnnotation(card->getAnnotation(), isCreature ? dmg : 0);
                    mergedAnn = mergeRuledCountersIntoAnnotation(mergedAnn, counterAnn);
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
    if (v.hand_object_id_size() == v.hand_size() && v.hand_size() == handZone->getCards().size()) {
        for (Server_Card *hc : handZone->getCards()) {
            const int cid = hc->getId();
            const auto soIt = serverCardIdToEngineOid.constFind(cid);
            if (soIt != serverCardIdToEngineOid.constEnd()) {
                engineOidToServerCardId.remove(*soIt);
                serverCardIdToEngineOid.remove(cid);
            }
        }
        for (int i = 0; i < v.hand_size(); ++i) {
            const quint32 oid = static_cast<quint32>(v.hand_object_id(i));
            Server_Card *card = handZone->getCards().at(i);
            engineOidToServerCardId.insert(oid, card->getId());
            serverCardIdToEngineOid.insert(card->getId(), oid);
        }
    }

    // Graveyard OIDs: build position-based map from graveyard_object_id parallel array.
    // The engine's graveyard and the relay graveyard zone both maintain insertion order, so
    // position matching is correct as long as the sizes agree.
    Server_CardZone *graveZone = zones.value(ZoneNames::GRAVE);
    if (graveZone && v.graveyard_object_id_size() == graveZone->getCards().size()) {
        graveyardEngineOidToServerCardId.clear();
        for (int i = 0; i < v.graveyard_object_id_size(); ++i) {
            const quint32 oid = static_cast<quint32>(v.graveyard_object_id(i));
            Server_Card *card = graveZone->getCards().at(i);
            graveyardEngineOidToServerCardId.insert(oid, card->getId());
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
