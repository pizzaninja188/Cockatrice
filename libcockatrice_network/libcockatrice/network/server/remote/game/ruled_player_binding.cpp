// Fork-owned. See ruled_player_binding.h. This code was extracted verbatim from
// server_player.cpp (roadmap Step 4); the Server_Player is reached through its public
// interface plus the `friend struct RuledPlayerBinding` grant on Server_AbstractPlayer
// (covers the protected sendCreateTokenEvents token-broadcast helper).

#include "ruled_player_binding.h"

#include "../server_response_containers.h"
#include "ruled_game_driver.h"
#include "ruled_utils.h"
#include "server_card.h"
#include "server_cardzone.h"
#include "server_game.h"
#include "server_player.h"

#include <QDebug>
#include <QStringList>
#include <libcockatrice/protocol/pb/card_attributes.pb.h>
#include <libcockatrice/protocol/pb/event_destroy_card.pb.h>
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

QString mergeRuledBattleControllerIntoAnnotation(const QString &baseAnn, const QString &controllerName)
{
    const QString marker = QStringLiteral("Battle controller: ");
    QStringList kept;
    for (const QString &line : baseAnn.split(QLatin1Char('\n'))) {
        if (!line.trimmed().startsWith(marker)) {
            kept.append(line);
        }
    }
    const QString without = kept.join(QLatin1Char('\n')).trimmed();
    if (controllerName.isEmpty()) {
        return without;
    }
    const QString controllerLine = marker + controllerName;
    return without.isEmpty() ? controllerLine : without + QLatin1Char('\n') + controllerLine;
}

// Engine-authored labels for nonintrinsic rules state. Keep them on one replaceable line so every
// authoritative battlefield sync can remove expired effects without disturbing user text or the
// other ruled annotation lines above. Strip the legacy marker during the transition as well.
QString mergeRuledEffectsIntoAnnotation(const QString &baseAnn, const QStringList &effectLabels)
{
    const QString marker = QStringLiteral("Effects:");
    const QString legacyMarker = QStringLiteral("Granted:");
    QStringList kept;
    for (const QString &line : baseAnn.split(QLatin1Char('\n'))) {
        const QString trimmed = line.trimmed();
        if (trimmed.startsWith(marker) || trimmed.startsWith(legacyMarker)) {
            continue;
        }
        kept.append(line);
    }
    QString without = kept.join(QLatin1Char('\n')).trimmed();
    if (effectLabels.isEmpty()) {
        return without;
    }
    const QString effectsLine = marker + QLatin1Char(' ') + effectLabels.join(QStringLiteral(", "));
    if (without.isEmpty()) {
        return effectsLine;
    }
    return without + QLatin1Char('\n') + effectsLine;
}

// CR 707 display aid. This line is authoritative and replaceable so a copied permanent can
// return to its physical identity without retaining stale copy text.
QString mergeRuledCopyIntoAnnotation(const QString &baseAnn, const QString &copyAnnotation)
{
    const QString marker = QStringLiteral("Copy: ");
    QStringList kept;
    for (const QString &line : baseAnn.split(QLatin1Char('\n'))) {
        if (line.trimmed().startsWith(marker)) {
            continue;
        }
        kept.append(line);
    }
    const QString without = kept.join(QLatin1Char('\n')).trimmed();
    if (copyAnnotation.isEmpty()) {
        return without;
    }
    if (without.isEmpty()) {
        return copyAnnotation;
    }
    return without + QLatin1Char('\n') + copyAnnotation;
}

QString mergeRuledEnchantingIntoAnnotation(const QString &baseAnn, const QString &playerName)
{
    const QString marker = QStringLiteral("Enchanting: ");
    QStringList kept;
    for (const QString &line : baseAnn.split(QLatin1Char('\n'))) {
        if (!line.trimmed().startsWith(marker)) {
            kept.append(line);
        }
    }
    const QString without = kept.join(QLatin1Char('\n')).trimmed();
    if (playerName.isEmpty()) {
        return without;
    }
    const QString enchantingLine = marker + playerName;
    if (without.isEmpty()) {
        return enchantingLine;
    }
    return without + QLatin1Char('\n') + enchantingLine;
}

QString mergeRuledDoorsIntoAnnotation(const QString &baseAnn, const QStringList &doorLabels)
{
    const QString marker = QStringLiteral("Doors:");
    QStringList kept;
    for (const QString &line : baseAnn.split(QLatin1Char('\n'))) {
        if (!line.trimmed().startsWith(marker)) {
            kept.append(line);
        }
    }
    const QString without = kept.join(QLatin1Char('\n')).trimmed();
    if (doorLabels.isEmpty()) {
        return without;
    }
    const QString doorsLine = marker + QLatin1Char(' ') + doorLabels.join(QStringLiteral(", "));
    if (without.isEmpty()) {
        return doorsLine;
    }
    return without + QLatin1Char('\n') + doorsLine;
}
} // namespace

void RuledPlayerBinding::unattachRuledCard(Server_Player *player, Server_Card *card, GameEventStorage &ges)
{
    if (!player || !card || !card->getParentCard()) {
        return;
    }
    player->unattachCard(ges, card);
}

RuledPlayerBinding::RuledZoneSyncResult
RuledPlayerBinding::applyRuledEngineZoneView(Server_Player *player,
                                             const ruled::v1::RuledPerPlayerView &v,
                                             GameEventStorage *tapGes,
                                             bool allowUntapReset,
                                             const QSet<quint32> *engineUntappedOids,
                                             bool battlefieldsUnchanged)
{
    RuledZoneSyncResult result;
    const int playerId = player->getPlayerId();
    if (v.player_id() != playerId) {
        return result;
    }
    const QMap<QString, Server_CardZone *> &zones = player->getZones();
    Server_CardZone *deckZone = zones.value(ZoneNames::DECK);
    Server_CardZone *handZone = zones.value(ZoneNames::HAND);
    Server_CardZone *stackZone = zones.value(ZoneNames::STACK);
    Server_CardZone *tableZone = zones.value(ZoneNames::TABLE);
    if (!deckZone || !handZone || !stackZone || !tableZone) {
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
        // Record the engine's authoritative hand order even if the physical concealed-zone
        // reconcile below cannot complete. A pre-existing mismatch elsewhere in the private
        // pool must not make a later cast fall back to the card occupying the same physical
        // index; any hand object whose OID is already bound can still be selected exactly.
        handEngineOidsInOrder.clear();
        handEngineOidsInOrder.reserve(v.hand_cards_size());
        for (const auto &handCard : v.hand_cards()) {
            handEngineOidsInOrder.append(static_cast<quint32>(handCard.object_id()));
        }
        QList<Server_Card *> pool;
        for (Server_Card *c : deckZone->getCards()) {
            pool.append(c);
        }
        for (Server_Card *c : handZone->getCards()) {
            pool.append(c);
        }
        QVector<QPair<quint32, QString>> libWants;
        libWants.reserve(v.library_cards_size());
        for (const auto &entry : v.library_cards()) {
            libWants.append(
                qMakePair(static_cast<quint32>(entry.object_id()), QString::fromStdString(entry.card_id())));
        }
        if (v.hand_cards_size() + libWants.size() != pool.size()) {
            qWarning() << "applyRuledEngineZoneView: count mismatch hand" << v.hand_cards_size() << "lib"
                       << libWants.size() << "pool" << pool.size() << "library_cards" << v.library_cards_size();
            return result;
        }
        // Stable physical identity wins over name matching. The fallback is required only for the
        // first startup sync, before the server has ever seen these engine ObjectIds. Afterwards,
        // this prevents duplicate-name cards from exchanging identities during a shuffle.
        const auto previousOidForCard = [this](const Server_Card *card) {
            if (!card) {
                return 0u;
            }
            const int serverCardId = card->getId();
            if (card->getZone() && card->getZone()->getName() == ZoneNames::DECK) {
                return libraryServerCardIdToEngineOid.value(serverCardId, 0u);
            }
            return serverCardIdToEngineOid.value(serverCardId, 0u);
        };
        QList<Server_Card *> handList;
        for (int i = 0; i < v.hand_cards_size(); ++i) {
            const QString want = QString::fromStdString(v.hand_cards(i).card_id());
            const quint32 wantOid = static_cast<quint32>(v.hand_cards(i).object_id());
            int found = -1;
            for (int j = 0; j < pool.size(); ++j) {
                if (previousOidForCard(pool[j]) == wantOid && trId(pool[j]) == want) {
                    found = j;
                    break;
                }
            }
            if (found < 0) {
                for (int j = 0; j < pool.size(); ++j) {
                    if (trId(pool[j]) == want) {
                        found = j;
                        break;
                    }
                }
            }
            if (found < 0) {
                qWarning() << "applyRuledEngineZoneView: missing" << want << "player" << playerId;
                return result;
            }
            handList.append(pool.takeAt(found));
        }
        QList<Server_Card *> libList;
        for (const auto &entry : libWants) {
            const quint32 wantOid = entry.first;
            const QString &want = entry.second;
            int found = -1;
            for (int j = 0; j < pool.size(); ++j) {
                if (previousOidForCard(pool[j]) == wantOid && trId(pool[j]) == want) {
                    found = j;
                    break;
                }
            }
            if (found < 0) {
                for (int j = 0; j < pool.size(); ++j) {
                    if (trId(pool[j]) == want) {
                        found = j;
                        break;
                    }
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
        // Rebuild every concealed-zone binding from the authoritative engine order. These maps
        // never cross the server boundary, but they let later moves select the exact physical
        // object even when the library contains duplicate card names.
        for (Server_Card *card : currentHand) {
            const auto oidIt = serverCardIdToEngineOid.constFind(card->getId());
            if (oidIt != serverCardIdToEngineOid.constEnd()) {
                engineOidToServerCardId.remove(*oidIt);
                serverCardIdToEngineOid.remove(card->getId());
            }
        }
        libraryEngineOidToServerCardId.clear();
        libraryServerCardIdToEngineOid.clear();
        for (int i = 0; i < handList.size(); ++i) {
            registerEngineOid(static_cast<quint32>(v.hand_cards(i).object_id()), handList[i]->getId());
        }
        for (int i = 0; i < libList.size(); ++i) {
            registerLibraryEngineOid(libWants[i].first, libList[i]->getId());
        }
        // Every early return above leaves this false, so a failed reconcile keeps warning about
        // later omissions rather than pretending the zones were seeded.
        privateZonesSynced = true;
    }

    // A global battlefield omission means the complete prior replacement remains authoritative.
    // Keep every identity/keyword/creature map and every physical visual untouched. Empty lists
    // without this flag still mean an explicit replacement with an empty battlefield.
    if (battlefieldsUnchanged) {
        if (!battlefieldSynced) {
            qWarning() << "applyRuledEngineZoneView: player" << playerId
                       << "reports unchanged battlefield before any full sync — identity and visuals were never seeded";
        }
    } else {
        battlefieldSynced = false;
        // Build an engine_oid -> Server_Card id map and (when permitted) sync tap state.
        // The map is rebuilt whenever the engine reports a battlefield, even if the caller
        // doesn't ask for tap propagation, so combat translation in the driver can rely on it.
        QList<Server_Card *> engineTableCards;
        engineTableCards.reserve(tableZone->getCards().size());
        for (Server_Card *card : tableZone->getCards()) {
            if (card && card->getId() != enduringStoryServerCardId &&
                !staticEmblemServerCardIds.values().contains(card->getId())) {
                engineTableCards.append(card);
            }
        }
        if (v.battlefield_objects_size() == engineTableCards.size()) {
            QList<Server_Card *> tablePool;
            tablePool.reserve(engineTableCards.size());
            for (Server_Card *c : engineTableCards) {
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
                // The engine's effective types are authoritative. This also reflows a permanent after
                // a type-changing effect and repairs stale coordinates during join/reconnect.
                QList<int> expectedY;
                expectedY.reserve(ordered.size());
                for (int i = 0; i < ordered.size(); ++i) {
                    const auto &object = v.battlefield_objects(i);
                    expectedY.append(ruledBattlefieldGridY(object.is_creature(), object.is_land()));
                }

                const QList<Server_Card *> &zoneOrderBefore = engineTableCards;
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
                        const int x =
                            c->getParentCard() ? -1 : tableZone->getFreeGridColumn(-1, y, c->getName(), y != 2);
                        tableZone->insertCard(c, x, y);
                    }
                    result.battlefieldOrderChanged = true;
                }

                // Battlefield, stack, and hand share these server-only maps. Preserve the interactive
                // nonbattlefield bindings before rebuilding the battlefield half. In particular, a
                // state-based action can change the battlefield while another spell is still waiting
                // underneath on the stack; losing that binding would strand its physical card when it
                // later resolves or fizzles.
                QHash<quint32, int> preservedNonbattlefieldOidToServerCardId;
                QHash<int, quint32> preservedNonbattlefieldServerCardIdToOid;
                for (Server_Card *nonbattlefieldCard : handZone->getCards() + stackZone->getCards()) {
                    if (!nonbattlefieldCard) {
                        continue;
                    }
                    const int serverCardId = nonbattlefieldCard->getId();
                    const auto oidIt = serverCardIdToEngineOid.constFind(serverCardId);
                    if (oidIt == serverCardIdToEngineOid.constEnd()) {
                        continue;
                    }
                    preservedNonbattlefieldOidToServerCardId.insert(*oidIt, serverCardId);
                    preservedNonbattlefieldServerCardIdToOid.insert(serverCardId, *oidIt);
                }

                engineOidToServerCardId = preservedNonbattlefieldOidToServerCardId;
                serverCardIdToEngineOid = preservedNonbattlefieldServerCardIdToOid;
                engineOidToSummoningSick.clear();
                engineOidToHaste.clear();
                engineOidToTrample.clear();
                engineOidToCreature.clear();
                engineOidToFaceDown.clear();
                engineOidToZoneChangeGeneration.clear();
                engineOidToUnderlyingCardId.clear();
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
                    engineOidToFaceDown.insert(oid, battlefieldObject.face_down());
                    engineOidToZoneChangeGeneration.insert(
                        oid, static_cast<quint64>(battlefieldObject.zone_change_generation()));
                    engineOidToUnderlyingCardId.insert(oid, QString::fromStdString(battlefieldObject.card_id()));

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

                        // Earthbend and other type-changing effects must also remove a former
                        // creature's badge. Printed Oracle P/T is not authoritative in ruled mode.
                        const QString newPt = isCreature ? QStringLiteral("%1/%2").arg(pwr).arg(tgh) : QString();
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

                        const QString counterAnn = QString::fromStdString(battlefieldObject.counters_annotation());
                        const int controllerPlayerId = battlefieldObject.has_controller_player_id()
                                                           ? battlefieldObject.controller_player_id()
                                                           : playerId;
                        QString ownerName;
                        if (battlefieldObject.owner_player_id() != controllerPlayerId) {
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
                        QString battleControllerName;
                        if (battlefieldObject.has_battle_protector_player_id() && controllerPlayerId != playerId) {
                            battleControllerName = QStringLiteral("P%1").arg(controllerPlayerId);
                            if (Server_Game *g = player->getGame()) {
                                if (Server_AbstractPlayer *controllerPlayer = g->getPlayer(controllerPlayerId)) {
                                    const QString resolvedName =
                                        QString::fromStdString(controllerPlayer->getUserInfo()->name());
                                    if (!resolvedName.isEmpty()) {
                                        battleControllerName = resolvedName;
                                    }
                                }
                            }
                        }
                        mergedAnn = mergeRuledBattleControllerIntoAnnotation(mergedAnn, battleControllerName);
                        mergedAnn = mergeRuledCopyIntoAnnotation(
                            mergedAnn, QString::fromStdString(battlefieldObject.copy_annotation()));
                        QString enchantedPlayerName;
                        if (battlefieldObject.has_attachment_recipient() &&
                            battlefieldObject.attachment_recipient().recipient_case() ==
                                ruled::v1::AttachmentRecipient::kPlayerId) {
                            const int enchantedPlayerId = battlefieldObject.attachment_recipient().player_id();
                            enchantedPlayerName = QStringLiteral("P%1").arg(enchantedPlayerId);
                            if (Server_Game *g = player->getGame()) {
                                if (Server_AbstractPlayer *enchantedPlayer = g->getPlayer(enchantedPlayerId)) {
                                    const QString resolvedName =
                                        QString::fromStdString(enchantedPlayer->getUserInfo()->name());
                                    if (!resolvedName.isEmpty()) {
                                        enchantedPlayerName = resolvedName;
                                    }
                                }
                            }
                        }
                        mergedAnn = mergeRuledEnchantingIntoAnnotation(mergedAnn, enchantedPlayerName);
                        QStringList doorLabels;
                        doorLabels.reserve(battlefieldObject.room_doors_size());
                        for (const auto &door : battlefieldObject.room_doors()) {
                            doorLabels.append(QStringLiteral("%1 (%2)").arg(
                                QString::fromStdString(door.name()),
                                door.unlocked() ? QStringLiteral("unlocked") : QStringLiteral("locked")));
                        }
                        mergedAnn = mergeRuledDoorsIntoAnnotation(mergedAnn, doorLabels);
                        QStringList rulesAnnotationLabels;
                        rulesAnnotationLabels.reserve(battlefieldObject.rules_annotation_labels_size());
                        for (const std::string &label : battlefieldObject.rules_annotation_labels()) {
                            rulesAnnotationLabels.append(QString::fromStdString(label));
                        }
                        mergedAnn = mergeRuledEffectsIntoAnnotation(mergedAnn, rulesAnnotationLabels);
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
                battlefieldSynced = true;
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

    // Move passes can reach a public pile in a different order from engine resolution
    // (Lightning Bolt and its victim, or simultaneous mill/discard). Preserve recorded
    // identities, then arrange the physical pile newest-first from the engine's order.
    auto reconcilePublicZone = [&](Server_CardZone *zone, const auto &oids, QHash<quint32, int> &bindings) {
        if (!zone || oids.size() != zone->getCards().size()) {
            return false;
        }
        QList<Server_Card *> pool = zone->getCards();
        QList<Server_Card *> oldestFirst(oids.size(), nullptr);
        for (int i = 0; i < oids.size(); ++i) {
            const auto bound = bindings.constFind(static_cast<quint32>(oids.Get(i)));
            if (bound == bindings.constEnd()) {
                continue;
            }
            for (int j = 0; j < pool.size(); ++j) {
                if (pool[j]->getId() == *bound) {
                    oldestFirst[i] = pool.takeAt(j);
                    break;
                }
            }
        }
        // A first snapshot may seed previously unbound cards by pile position. Reserve
        // every known identity first so an unbound slot cannot steal a moved card.
        for (Server_Card *&card : oldestFirst) {
            if (!card) {
                card = pool.takeLast();
            }
        }
        const QList<Server_Card *> newestFirst(oldestFirst.crbegin(), oldestFirst.crend());
        if (newestFirst != zone->getCards()) {
            const auto previous = zone->getCards();
            for (Server_Card *card : previous) {
                zone->removeCard(card);
            }
            for (Server_Card *card : newestFirst) {
                zone->insertCard(card, -1, 0);
            }
            result.publicZoneOrderChanged = true;
        }
        bindings.clear();
        for (int i = 0; i < oids.size(); ++i) {
            bindings.insert(static_cast<quint32>(oids.Get(i)), oldestFirst[i]->getId());
        }
        return true;
    };
    if (reconcilePublicZone(zones.value(ZoneNames::GRAVE), v.graveyard_object_ids(),
                            graveyardEngineOidToServerCardId)) {
        graveyardEngineOidsOldestFirst.clear();
        for (const auto oid : v.graveyard_object_ids()) {
            graveyardEngineOidsOldestFirst.append(static_cast<quint32>(oid));
        }
    }
    reconcilePublicZone(zones.value(ZoneNames::EXILE), v.exile_object_ids(), exileEngineOidToServerCardId);
    result.engineOidToServerCardId = engineOidToServerCardId;
    return result;
}

Server_Card *RuledPlayerBinding::findCardByEngineOid(const Server_Player *player, quint32 engineOid) const
{
    auto it = engineOidToServerCardId.constFind(engineOid);
    int serverCardId = -1;
    if (it != engineOidToServerCardId.constEnd()) {
        serverCardId = *it;
    } else {
        const auto libraryIt = libraryEngineOidToServerCardId.constFind(engineOid);
        if (libraryIt != libraryEngineOidToServerCardId.constEnd()) {
            serverCardId = *libraryIt;
        }
    }
    if (serverCardId < 0) {
        return nullptr;
    }
    for (const char *zn : {ZoneNames::TABLE, ZoneNames::HAND, ZoneNames::STACK, ZoneNames::DECK}) {
        if (Server_CardZone *z = player->getZones().value(zn)) {
            if (z->getType() == ServerInfo_Zone::HiddenZone) {
                for (Server_Card *card : z->getCards()) {
                    if (card && card->getId() == serverCardId) {
                        return card;
                    }
                }
                continue;
            }
            if (Server_Card *c = z->getCard(serverCardId, nullptr, false)) {
                return c;
            }
        }
    }
    return nullptr;
}

Server_Card *RuledPlayerBinding::findHandCardByEngineIndex(const Server_Player *player, int engineIndex) const
{
    if (engineIndex < 0 || engineIndex >= handEngineOidsInOrder.size()) {
        return nullptr;
    }
    Server_Card *card = findCardByEngineOid(player, handEngineOidsInOrder.at(engineIndex));
    if (!card || !card->getZone() || card->getZone()->getName() != ZoneNames::HAND) {
        return nullptr;
    }
    return card;
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

Server_Card *RuledPlayerBinding::findExileCardByEngineOid(const Server_Player *player, quint32 engineOid) const
{
    const auto it = exileEngineOidToServerCardId.constFind(engineOid);
    if (it == exileEngineOidToServerCardId.constEnd()) {
        return nullptr;
    }
    if (Server_CardZone *zone = player->getZones().value(ZoneNames::EXILE)) {
        return zone->getCard(*it, nullptr, false);
    }
    return nullptr;
}

void RuledPlayerBinding::createRuledToken(Server_Player *player,
                                          quint32 engineOid,
                                          const ruled::v1::TokenIdentity &identity,
                                          int battlefieldGridY,
                                          bool entersTapped,
                                          GameEventStorage &ges)
{
    Server_CardZone *table = player->getZones().value(ZoneNames::TABLE);
    if (!table) {
        return;
    }
    const QString name = QString::fromStdString(identity.name());
    const int y = battlefieldGridY;
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
    QStringList abilityTexts;
    abilityTexts.reserve(identity.ability_texts_size());
    for (const auto &text : identity.ability_texts()) {
        abilityTexts.append(QString::fromStdString(text));
    }
    card->setTokenAbilityTexts(abilityTexts);
    card->setAnnotation(QStringLiteral("Token"));
    card->setTapped(entersTapped);
    // CR 111.7: when the engine later moves the token off the battlefield it ceases to exist;
    // destroy-on-zone-change makes the client drop the card the moment that move arrives.
    card->setDestroyOnZoneChange(true);
    table->insertCard(card, x, y);
    player->sendCreateTokenEvents(table, card, x, y, ges);
    if (entersTapped) {
        Event_SetCardAttr tapEvent;
        tapEvent.set_zone_name(std::string(ZoneNames::TABLE));
        tapEvent.set_card_id(card->getId());
        tapEvent.set_attribute(AttrTapped);
        tapEvent.set_attr_value("1");
        ges.enqueueGameEvent(tapEvent, player->getPlayerId());
    }

    // Pre-register the engine ObjectId <-> Server_Card binding so the zone-view sync that follows
    // in the same batch matches this engine battlefield slot to the freshly minted card (rather
    // than failing to find a physical card and aborting the reconcile).
    engineOidToServerCardId.insert(engineOid, card->getId());
    serverCardIdToEngineOid.insert(card->getId(), engineOid);
}

bool RuledPlayerBinding::ensureEnduringStoryToken(Server_Player *player, int battlefieldGridY, GameEventStorage *ges)
{
    Server_CardZone *table = player ? player->getZones().value(ZoneNames::TABLE) : nullptr;
    if (!table) {
        return false;
    }
    for (Server_Card *card : table->getCards()) {
        if (card && card->getId() == enduringStoryServerCardId) {
            return false;
        }
    }

    const QString name = QStringLiteral("Enduring Story");
    int x = table->hasCoords() ? table->getFreeGridColumn(-1, battlefieldGridY, name, true) : 0;
    if (x < 0) {
        x = 0;
    }
    auto *card = new Server_Card({name, QString()}, player->newCardId(), x, battlefieldGridY);
    card->moveToThread(player->thread());
    card->setAnnotation(QStringLiteral("Token"));
    card->setDestroyOnZoneChange(true);
    table->insertCard(card, x, battlefieldGridY);
    enduringStoryServerCardId = card->getId();
    if (ges) {
        player->sendCreateTokenEvents(table, card, x, battlefieldGridY, *ges);
    }
    return true;
}

bool RuledPlayerBinding::reconcileStaticEmblemTokens(Server_Player *player,
                                                      const ruled::v1::RuledPerPlayerView &view,
                                                      int battlefieldGridY,
                                                      GameEventStorage *ges)
{
    Server_CardZone *table = player ? player->getZones().value(ZoneNames::TABLE) : nullptr;
    if (!table) {
        return false;
    }

    QSet<quint32> desired;
    for (const auto &emblem : view.static_emblems()) {
        desired.insert(static_cast<quint32>(emblem.object_id()));
    }

    bool changed = false;
    for (auto it = staticEmblemServerCardIds.begin(); it != staticEmblemServerCardIds.end();) {
        if (desired.contains(it.key())) {
            ++it;
            continue;
        }
        if (Server_Card *card = table->getCard(it.value(), nullptr, false)) {
            table->removeCard(card);
            if (ges) {
                Event_DestroyCard event;
                event.set_zone_name(std::string(ZoneNames::TABLE));
                event.set_card_id(static_cast<::google::protobuf::uint32>(card->getId()));
                ges->enqueueGameEvent(event, player->getPlayerId());
            }
            card->deleteLater();
        }
        it = staticEmblemServerCardIds.erase(it);
        changed = true;
    }

    for (const auto &emblem : view.static_emblems()) {
        const quint32 objectId = static_cast<quint32>(emblem.object_id());
        const int existingId = staticEmblemServerCardIds.value(objectId, -1);
        if (existingId >= 0 && table->getCard(existingId, nullptr, false)) {
            continue;
        }
        const QString name = QString::fromStdString(emblem.display_name());
        int x = table->hasCoords() ? table->getFreeGridColumn(-1, battlefieldGridY, name, true) : 0;
        if (x < 0) {
            x = 0;
        }
        auto *card = new Server_Card({name, QString()}, player->newCardId(), x, battlefieldGridY);
        card->moveToThread(player->thread());
        card->setAnnotation(QStringLiteral("Emblem"));
        card->setDestroyOnZoneChange(true);
        table->insertCard(card, x, battlefieldGridY);
        staticEmblemServerCardIds.insert(objectId, card->getId());
        if (ges) {
            player->sendCreateTokenEvents(table, card, x, battlefieldGridY, *ges);
        }
        changed = true;
    }
    return changed;
}

bool RuledPlayerBinding::createRuledDevCard(Server_Player *player,
                                            quint32 engineOid,
                                            const QString &cardName,
                                            int battlefieldGridY,
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
        handEngineOidsInOrder.append(engineOid);
        return true;
    }

    // Table: the driver resolved the engine-authored effective type from the batch's full
    // battlefield view before this early physical creation pass.
    const int y = battlefieldGridY;
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
