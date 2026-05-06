#include "game_event_handler.h"

#include "board/arrow_item.h"
#include "board/arrow_target.h"
#include "board/card_item.h"
#include "../interface/widgets/tabs/tab_game.h"
#include "zones/logic/card_zone_logic.h"
#include "abstract_game.h"
#include "log/message_log_widget.h"
#include "player/player.h"
#include "player/player_manager.h"

#include <libcockatrice/network/client/abstract/abstract_client.h>
#include <libcockatrice/protocol/get_pb_extension.h>
#include <libcockatrice/protocol/pb/command_concede.pb.h>
#include <libcockatrice/protocol/pb/command_delete_arrow.pb.h>
#include <libcockatrice/protocol/pb/command_game_say.pb.h>
#include <libcockatrice/protocol/pb/command_leave_game.pb.h>
#include <libcockatrice/protocol/pb/command_next_turn.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/command_reverse_turn.pb.h>
#include <libcockatrice/protocol/pb/command_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/context_connection_state_changed.pb.h>
#include <libcockatrice/protocol/pb/context_deck_select.pb.h>
#include <libcockatrice/protocol/pb/event_game_closed.pb.h>
#include <libcockatrice/protocol/pb/event_game_host_changed.pb.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_join.pb.h>
#include <libcockatrice/protocol/pb/event_kicked.pb.h>
#include <libcockatrice/protocol/pb/game_event.pb.h>
#include <libcockatrice/protocol/pb/event_leave.pb.h>
#include <libcockatrice/protocol/pb/event_player_properties_changed.pb.h>
#include <libcockatrice/protocol/pb/event_reverse_turn.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_player.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pending_command.h>
#include <libcockatrice/utility/zone_names.h>
#include <QColor>
#include <QRegularExpression>
#include <QTimer>
#include <algorithm>

namespace {
struct ParsedRuledLandActions
{
    QSet<int> handIndices;
    QMultiHash<QString, int> handIndicesByCardName;
};

struct ParsedRuledCastActions
{
    QSet<int> handIndices;
    QMultiHash<QString, int> handIndicesByCardName;
};

bool parseCreaturePt(const QString &pt, int *outPower, int *outToughness)
{
    if (!outPower || !outToughness) {
        return false;
    }
    *outPower = *outToughness = 0;
    const QString s = pt.trimmed();
    if (s.isEmpty()) {
        return false;
    }
    const int slash = s.indexOf(QLatin1Char('/'));
    if (slash < 0) {
        return false;
    }
    const QString left = s.left(slash).trimmed();
    const QString right = s.mid(slash + 1).trimmed();
    if (left.contains(QLatin1Char('*')) || right.contains(QLatin1Char('*'))) {
        return false;
    }
    bool okP = false;
    bool okT = false;
    *outPower = left.toInt(&okP);
    *outToughness = right.toInt(&okT);
    return okP && okT;
}

GameEventHandler::RuledCombatPhase mapRuledPhaseSlugToCombatPhase(const QString &slug)
{
    if (slug == QLatin1String("declare_attackers")) {
        return GameEventHandler::RuledCombatPhase::DeclareAttackers;
    }
    if (slug == QLatin1String("declare_blockers")) {
        return GameEventHandler::RuledCombatPhase::DeclareBlockers;
    }
    if (slug == QLatin1String("assign_combat_damage")) {
        return GameEventHandler::RuledCombatPhase::AssignCombatDamage;
    }
    if (slug == QLatin1String("combat_damage")) {
        return GameEventHandler::RuledCombatPhase::CombatDamage;
    }
    if (slug == QLatin1String("end_combat")) {
        return GameEventHandler::RuledCombatPhase::None;
    }
    return GameEventHandler::RuledCombatPhase::None;
}

int mapRuledPhaseSlugToToolbarPhase(const QString &slug)
{
    if (slug == QLatin1String("untap")) {
        return 0;
    }
    if (slug == QLatin1String("upkeep")) {
        return 1;
    }
    if (slug == QLatin1String("draw")) {
        return 2;
    }
    if (slug == QLatin1String("main1")) {
        return 3;
    }
    if (slug == QLatin1String("begin_combat")) {
        return 4;
    }
    if (slug == QLatin1String("declare_attackers")) {
        return 5;
    }
    if (slug == QLatin1String("declare_blockers") || slug == QLatin1String("assign_combat_damage")) {
        return 6;
    }
    if (slug == QLatin1String("combat_damage")) {
        return 7;
    }
    if (slug == QLatin1String("end_combat")) {
        return 8;
    }
    if (slug == QLatin1String("main2")) {
        return 9;
    }
    if (slug == QLatin1String("end_step") || slug == QLatin1String("cleanup")) {
        return 10;
    }
    return -1;
}

ParsedRuledLandActions parseRuledLandActions(const ruled::v1::LegalActions &actions)
{
    static const QRegularExpression labelRegex(QStringLiteral(R"(^Play land (.*) \(hand idx (\d+)\)$)"));
    ParsedRuledLandActions parsed;

    for (const auto &label : actions.labels()) {
        const QRegularExpressionMatch match = labelRegex.match(QString::fromStdString(label));
        if (!match.hasMatch()) {
            continue;
        }

        bool ok = false;
        const int handIndex = match.captured(2).toInt(&ok);
        if (ok) {
            parsed.handIndices.insert(handIndex);
            parsed.handIndicesByCardName.insert(match.captured(1), handIndex);
        }
    }
    return parsed;
}

ParsedRuledCastActions parseRuledCastActions(const ruled::v1::LegalActions &actions)
{
    static const QRegularExpression labelRegex(QStringLiteral(R"(^Cast (.*) \(hand idx (\d+)\)$)"));
    ParsedRuledCastActions parsed;

    for (const auto &label : actions.labels()) {
        const QRegularExpressionMatch match = labelRegex.match(QString::fromStdString(label));
        if (!match.hasMatch()) {
            continue;
        }

        bool ok = false;
        const int handIndex = match.captured(2).toInt(&ok);
        if (ok) {
            parsed.handIndices.insert(handIndex);
            parsed.handIndicesByCardName.insert(match.captured(1), handIndex);
        }
    }
    return parsed;
}

ParsedRuledLandActions parseRuledCleanupDiscardActions(const ruled::v1::LegalActions &actions)
{
    static const QRegularExpression labelRegex(
        QStringLiteral(R"(^Discard (.*) \(cleanup, hand idx (\d+)\)$)"));
    ParsedRuledLandActions parsed;

    for (const auto &label : actions.labels()) {
        const QRegularExpressionMatch match = labelRegex.match(QString::fromStdString(label));
        if (!match.hasMatch()) {
            continue;
        }

        bool ok = false;
        const int handIndex = match.captured(2).toInt(&ok);
        if (ok) {
            parsed.handIndices.insert(handIndex);
            parsed.handIndicesByCardName.insert(match.captured(1), handIndex);
        }
    }
    return parsed;
}

CardItem *findStackCardByServerId(AbstractGame *ag, int serverCardId)
{
    if (!ag || serverCardId < 0) {
        return nullptr;
    }
    for (Player *p : ag->getPlayerManager()->getPlayers()) {
        if (!p) {
            continue;
        }
        if (CardItem *c = ag->getCard(p->getPlayerInfo()->getId(), QString::fromLatin1(ZoneNames::STACK), serverCardId)) {
            return c;
        }
    }
    return nullptr;
}

CardItem *findBattlefieldCardByEngineOid(AbstractGame *ag, const GameEventHandler *handler, quint32 engineOid)
{
    if (!ag || !handler || engineOid == 0) {
        return nullptr;
    }
    const int sid = handler->cardIdForEngineOid(engineOid);
    const int owner = handler->playerIdForEngineOid(engineOid);
    if (sid >= 0 && owner >= 0) {
        if (CardItem *c = ag->getCard(owner, QString::fromLatin1(ZoneNames::TABLE), sid)) {
            return c;
        }
    }
    PlayerManager *pm = ag->getPlayerManager();
    for (Player *p : pm->getPlayers()) {
        if (!p) {
            continue;
        }
        CardZoneLogic *zt = p->getZones().value(QString::fromLatin1(ZoneNames::TABLE), nullptr);
        if (!zt) {
            continue;
        }
        for (CardItem *c : zt->getCards()) {
            if (!c) {
                continue;
            }
            const int cid = c->getId();
            // BattlefieldObjectMap keys (player_id, server_card_id) use the server zone controller; CardItem
            // ownership can disagree, so try every seat id that appears in the ruled oid map.
            for (Player *op : pm->getPlayers()) {
                if (!op || !op->getPlayerInfo()) {
                    continue;
                }
                const int opId = op->getPlayerInfo()->getId();
                if (handler->engineOidForCardId(opId, cid) == engineOid) {
                    return c;
                }
            }
        }
    }
    return nullptr;
}

CardItem *resolveStackCardItemByEngineOid(AbstractGame *ag,
                                          TabGame *tab,
                                          const GameEventHandler *handler,
                                          PlayerManager *pm,
                                          quint32 stackSpellOid)
{
    const int sid = handler->cardIdForEngineOid(stackSpellOid);
    if (sid >= 0) {
        if (tab) {
            if (CardItem *c = tab->findVisibleStackSpellCardItem(sid)) {
                return c;
            }
        }
        if (CardItem *c = findStackCardByServerId(ag, sid)) {
            return c;
        }
    }
    for (Player *p : pm->getPlayers()) {
        if (!p) {
            continue;
        }
        CardZoneLogic *sz = p->getZones().value(QString::fromLatin1(ZoneNames::STACK), nullptr);
        if (!sz) {
            continue;
        }
        for (CardItem *c : sz->getCards()) {
            if (!c) {
                continue;
            }
            const int cid = c->getId();
            for (Player *op : pm->getPlayers()) {
                if (!op || !op->getPlayerInfo()) {
                    continue;
                }
                if (handler->engineOidForCardId(op->getPlayerInfo()->getId(), cid) == stackSpellOid) {
                    return c;
                }
            }
        }
    }
    return nullptr;
}

ArrowTarget *resolveRuledSpellTarget(AbstractGame *ag,
                                     TabGame *tab,
                                     const GameEventHandler *handler,
                                     const QSet<quint32> &stackOids,
                                     quint32 targetOid)
{
    if (!ag || !handler) {
        return nullptr;
    }
    PlayerManager *pm = ag->getPlayerManager();
    // Only Cockatrice seat ids count as player targets (engine object ids never collide with seats).
    const int seatId = static_cast<int>(targetOid);
    if (pm->getPlayers().contains(seatId)) {
        if (Player *asPlayer = pm->getPlayer(seatId)) {
            return asPlayer->getGraphicsItem()->getPlayerTarget();
        }
    }
    if (stackOids.contains(targetOid)) {
        return resolveStackCardItemByEngineOid(ag, tab, handler, pm, targetOid);
    }
    // Counterspell target: the countered spell's oid may already be removed from `ruledStackObjectIds` in the same
    // batch as StackResolved, while StackPushed.targets still reference it — resolve via map + physical stack zone.
    const int sidProbe = handler->cardIdForEngineOid(targetOid);
    if (sidProbe >= 0) {
        if (CardItem *stk = findStackCardByServerId(ag, sidProbe)) {
            if (stk->getZone() &&
                stk->getZone()->getName().compare(QStringLiteral("stack"), Qt::CaseInsensitive) == 0) {
                if (tab) {
                    if (CardItem *vis = tab->findVisibleStackSpellCardItem(sidProbe)) {
                        return vis;
                    }
                }
                return stk;
            }
        }
    }
    return findBattlefieldCardByEngineOid(ag, handler, targetOid);
}
} // namespace

GameEventHandler::GameEventHandler(AbstractGame *_game) : QObject(_game), game(_game)
{
}

bool GameEventHandler::isRuledLandPlayLegalForHandIndex(int handIndex) const
{
    return legalRuledLandPlayHandIndices.contains(handIndex);
}

int GameEventHandler::getRuledLandPlayHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = getRuledLandPlayHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> GameEventHandler::getRuledLandPlayHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalRuledLandPlayIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

bool GameEventHandler::isRuledSpellCastLegalForHandIndex(int handIndex) const
{
    return legalRuledSpellCastHandIndices.contains(handIndex);
}

int GameEventHandler::getRuledSpellCastHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = getRuledSpellCastHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> GameEventHandler::getRuledSpellCastHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalRuledSpellCastIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

int GameEventHandler::resolveEngineHandIndexFromLegalSlots(const CardItem *card,
                                                           const QList<int> &sortedLegalHandIndices) const
{
    if (!card || !card->getZone()) {
        return -1;
    }
    const CardZoneLogic *zone = card->getZone();
    Player *handPlayer = zone->getPlayer();
    const int clickedId = card->getId();
    if (handPlayer && handPlayer->getPlayerInfo()) {
        const int pid = handPlayer->getPlayerInfo()->getId();
        const auto mit = ruledOwnedCardToEngineHandSlot.constFind(makeOwnedCardKey(pid, clickedId));
        if (mit != ruledOwnedCardToEngineHandSlot.constEnd()) {
            const int mapped = mit.value();
            if (mapped >= 0 && sortedLegalHandIndices.contains(mapped)) {
                return mapped;
            }
        }
    }
    for (int h : sortedLegalHandIndices) {
        if (h < 0 || h >= zone->getCards().size()) {
            continue;
        }
        const CardItem *slot = zone->getCards().at(h);
        if (slot && slot->getId() == clickedId) {
            return h;
        }
    }
    const int handIndex = zone->getCards().indexOf(const_cast<CardItem *>(card));
    if (handIndex >= 0 && sortedLegalHandIndices.contains(handIndex)) {
        return handIndex;
    }
    return -1;
}

int GameEventHandler::resolveRuledSpellCastHandIndexForClickedCard(const CardItem *card) const
{
    if (!card) {
        return -1;
    }
    return resolveEngineHandIndexFromLegalSlots(card, getRuledSpellCastHandIndicesForCardName(card->getName()));
}

int GameEventHandler::resolveRuledLandPlayHandIndexForClickedCard(const CardItem *card) const
{
    if (!card) {
        return -1;
    }
    return resolveEngineHandIndexFromLegalSlots(card, getRuledLandPlayHandIndicesForCardName(card->getName()));
}

bool GameEventHandler::isRuledCleanupDiscardLegalForHandIndex(int handIndex) const
{
    return legalRuledCleanupDiscardHandIndices.contains(handIndex);
}

int GameEventHandler::getRuledCleanupDiscardHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = getRuledCleanupDiscardHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> GameEventHandler::getRuledCleanupDiscardHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalRuledCleanupDiscardIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

int GameEventHandler::resolveRuledCleanupDiscardHandIndexForClickedCard(const CardItem *card) const
{
    if (!card) {
        return -1;
    }
    return resolveEngineHandIndexFromLegalSlots(card, getRuledCleanupDiscardHandIndicesForCardName(card->getName()));
}

bool GameEventHandler::isRuledOpeningBottomLegalForHandIndex(int handIndex) const
{
    return legalRuledOpeningBottomHandIndices.contains(handIndex);
}

int GameEventHandler::resolveRuledOpeningBottomHandIndexForClickedCard(const CardItem *card) const
{
    if (!card) {
        return -1;
    }
    QList<int> legal = legalRuledOpeningBottomHandIndices.values();
    std::sort(legal.begin(), legal.end());
    return resolveEngineHandIndexFromLegalSlots(card, legal);
}

bool GameEventHandler::localPlayerMustCleanupDiscard() const
{
    return !legalRuledCleanupDiscardHandIndices.isEmpty();
}

int GameEventHandler::ruledCleanupDiscardRequiredCount() const
{
    const int n = legalRuledCleanupDiscardHandIndices.size();
    if (n <= 7) {
        return 0;
    }
    return n - 7;
}

int GameEventHandler::ruledCleanupDiscardSelectedCount() const
{
    return cleanupDiscardSelectedIndices.size();
}

bool GameEventHandler::isRuledCleanupDiscardHandIndexSelected(int handIndex) const
{
    return cleanupDiscardSelectedIndices.contains(handIndex);
}

void GameEventHandler::toggleRuledCleanupDiscardHandIndex(int ruledHandIndex)
{
    if (!isRuledCleanupDiscardLegalForHandIndex(ruledHandIndex)) {
        return;
    }
    const int need = ruledCleanupDiscardRequiredCount();
    if (need <= 0) {
        return;
    }
    if (cleanupDiscardSelectedIndices.contains(ruledHandIndex)) {
        cleanupDiscardSelectedIndices.remove(ruledHandIndex);
    } else if (cleanupDiscardSelectedIndices.size() < need) {
        cleanupDiscardSelectedIndices.insert(ruledHandIndex);
    }
    emit ruledCleanupDiscardUiChanged(need, cleanupDiscardSelectedIndices.size());
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearRuledCleanupDiscardSelection(bool emitUiChange)
{
    if (cleanupDiscardSelectedIndices.isEmpty()) {
        return;
    }
    cleanupDiscardSelectedIndices.clear();
    if (emitUiChange) {
        emit ruledCleanupDiscardUiChanged(ruledCleanupDiscardRequiredCount(), 0);
        emit ruledCombatStateChanged();
    }
}

QList<int> GameEventHandler::ruledCleanupDiscardSelectedIndicesSorted() const
{
    QList<int> out;
    out.reserve(cleanupDiscardSelectedIndices.size());
    for (int x : cleanupDiscardSelectedIndices) {
        out.append(x);
    }
    std::sort(out.begin(), out.end());
    return out;
}

void GameEventHandler::notifyRuledHandUiChanged()
{
    emit ruledCombatStateChanged();
}

void GameEventHandler::pruneCleanupDiscardSelectionAndEmitUi()
{
    if (legalRuledCleanupDiscardHandIndices.isEmpty()) {
        cleanupDiscardSelectedIndices.clear();
        emit ruledCleanupDiscardUiChanged(0, 0);
        emit ruledCombatStateChanged();
        return;
    }
    for (auto it = cleanupDiscardSelectedIndices.begin(); it != cleanupDiscardSelectedIndices.end();) {
        if (!legalRuledCleanupDiscardHandIndices.contains(*it)) {
            it = cleanupDiscardSelectedIndices.erase(it);
        } else {
            ++it;
        }
    }
    emit ruledCleanupDiscardUiChanged(ruledCleanupDiscardRequiredCount(), cleanupDiscardSelectedIndices.size());
    emit ruledCombatStateChanged();
}

void GameEventHandler::emitLocalRuledLog(const QString &message)
{
    emit ruledEnginePromptFeed(message);
}

bool GameEventHandler::localPlayerIsRuledActive() const
{
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    if (localId < 0 || currentRuledActivePlayerId < 0) {
        return false;
    }
    if (currentRuledCombatPhase == RuledCombatPhase::DeclareAttackers) {
        return localId == currentRuledActivePlayerId && !attackersSubmittedThisStep;
    }
    return localId == currentRuledActivePlayerId;
}

bool GameEventHandler::localPlayerIsRuledDefender() const
{
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    if (localId < 0 || currentRuledActivePlayerId < 0) {
        return false;
    }
    if (currentRuledCombatPhase == RuledCombatPhase::DeclareBlockers) {
        return localId != currentRuledActivePlayerId && !blockersSubmittedThisStep;
    }
    return localId != currentRuledActivePlayerId;
}

void GameEventHandler::togglePendingAttacker(quint32 engineOid)
{
    if (engineOid == 0) {
        return;
    }
    if (pendingAttackerOids.contains(engineOid)) {
        pendingAttackerOids.remove(engineOid);
    } else {
        pendingAttackerOids.insert(engineOid);
    }
    syncRuledAttackersPreviewToServer();
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearPendingAttackers()
{
    if (pendingAttackerOids.isEmpty()) {
        return;
    }
    pendingAttackerOids.clear();
    syncRuledAttackersPreviewToServer();
    emit ruledCombatStateChanged();
}

void GameEventHandler::toggleStagedBlocker(quint32 blockerOid)
{
    if (stagedBlockerOids.contains(blockerOid)) {
        stagedBlockerOids.remove(blockerOid);
    } else {
        stagedBlockerOids.insert(blockerOid);
    }
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearStagedBlockers()
{
    if (stagedBlockerOids.isEmpty()) {
        return;
    }
    stagedBlockerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::pairStagedBlockerToAttacker(quint32 attackerOid)
{
    if (stagedBlockerOids.isEmpty() || attackerOid == 0) {
        return;
    }
    if (!currentAttackerOids.contains(attackerOid)) {
        return;
    }
    for (quint32 blockerOid : std::as_const(stagedBlockerOids)) {
        pendingBlocks.insert(blockerOid, attackerOid);
    }
    stagedBlockerOids.clear();
    syncRuledBlockersPreviewToServer();
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearPendingBlocks()
{
    if (pendingBlocks.isEmpty() && stagedBlockerOids.isEmpty() && committedBlocks.isEmpty()) {
        return;
    }
    pendingBlocks.clear();
    committedBlocks.clear();
    stagedBlockerOids.clear();
    syncRuledBlockersPreviewToServer();
    emit ruledCombatStateChanged();
}

quint32 GameEventHandler::currentCombatDamageAttackerOid() const
{
    if (currentCombatDamageAttackerIdx < 0 ||
        currentCombatDamageAttackerIdx >= combatDamagePendingAttackers.size()) {
        return 0;
    }
    return combatDamagePendingAttackers.at(currentCombatDamageAttackerIdx);
}

quint32 GameEventHandler::assignedCombatDamageForBlocker(quint32 blockerOid) const
{
    return pendingCombatDamageByBlocker.value(blockerOid, 0);
}

int GameEventHandler::ruledCombatPowerForCreatureOid(quint32 engineOid) const
{
    const int fromEngine = engineOidBattlefieldPower.value(engineOid, 0);
    if (fromEngine > 0) {
        return fromEngine;
    }
    if (!game || engineOid == 0) {
        return 0;
    }
    if (CardItem *c = findBattlefieldCardByEngineOid(game, this, engineOid)) {
        int pow = 0;
        int tough = 0;
        if (parseCreaturePt(c->getPT(), &pow, &tough)) {
            return pow;
        }
    }
    return 0;
}

int GameEventHandler::ruledCombatToughnessForCreatureOid(quint32 engineOid) const
{
    const int fromEngine = engineOidBattlefieldToughness.value(engineOid, 0);
    if (fromEngine > 0) {
        return fromEngine;
    }
    if (!game || engineOid == 0) {
        return 1;
    }
    if (CardItem *c = findBattlefieldCardByEngineOid(game, this, engineOid)) {
        int pow = 0;
        int tough = 0;
        if (parseCreaturePt(c->getPT(), &pow, &tough) && tough > 0) {
            return tough;
        }
    }
    return 1;
}

void GameEventHandler::seedDefaultCombatDamageForCurrentAttacker()
{
    if (!game || !game->getGameMetaInfo()->proto().ruled_game() || !localPlayerIsRuledActive()) {
        return;
    }
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0) {
        return;
    }
    const QList<quint32> blockers = committedBlockerGroups.value(curAtt);
    if (blockers.size() < 2) {
        return;
    }
    const int power = ruledCombatPowerForCreatureOid(curAtt);
    if (power <= 0) {
        return;
    }
    for (quint32 blk : blockers) {
        pendingCombatDamageByBlocker.remove(blk);
    }
    int remaining = power;
    for (int i = 0; i < blockers.size(); ++i) {
        const quint32 blk = blockers.at(i);
        if (i == blockers.size() - 1) {
            if (remaining > 0) {
                pendingCombatDamageByBlocker.insert(blk, static_cast<quint32>(remaining));
            }
            break;
        }
        const int lethal =
            qMax(1, ruledCombatToughnessForCreatureOid(blk) - engineOidMarkedDamage.value(blk, 0));
        const int assign = qMin(remaining, lethal);
        remaining -= assign;
        if (assign > 0) {
            pendingCombatDamageByBlocker.insert(blk, static_cast<quint32>(assign));
        }
    }
    emit ruledCombatDamageUiChanged();
    emit ruledCombatStateChanged();
}

void GameEventHandler::bumpBlockerCombatDamage(quint32 blockerOid, int delta)
{
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0 || delta == 0) {
        return;
    }
    const QList<quint32> &blockers = committedBlockerGroups.value(curAtt);
    if (!blockers.contains(blockerOid)) {
        return;
    }
    const int power = ruledCombatPowerForCreatureOid(curAtt);
    if (power <= 0) {
        return;
    }
    const quint32 cur = pendingCombatDamageByBlocker.value(blockerOid, 0);
    qint64 next = static_cast<qint64>(cur) + delta;
    if (next < 0) {
        next = 0;
    }
    if (next > power) {
        next = power;
    }
    if (next == 0) {
        pendingCombatDamageByBlocker.remove(blockerOid);
    } else {
        pendingCombatDamageByBlocker.insert(blockerOid, static_cast<quint32>(next));
    }
    emit ruledCombatDamageUiChanged();
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearCombatDamageAssignmentState()
{
    combatDamagePendingAttackers.clear();
    currentCombatDamageAttackerIdx = -1;
    pendingCombatDamageByBlocker.clear();
}

QString GameEventHandler::currentCombatDamageAttackerDisplayName() const
{
    const quint32 att = currentCombatDamageAttackerOid();
    if (att == 0 || !game) {
        return {};
    }
    if (CardItem *c = findBattlefieldCardByEngineOid(game, this, att)) {
        return c->getName();
    }
    return tr("creature");
}

int GameEventHandler::currentCombatDamageAttackerPower() const
{
    const quint32 att = currentCombatDamageAttackerOid();
    if (att == 0) {
        return 0;
    }
    return ruledCombatPowerForCreatureOid(att);
}

int GameEventHandler::localCombatDamageAssignedTotal() const
{
    int sum = 0;
    for (auto it = pendingCombatDamageByBlocker.constBegin(); it != pendingCombatDamageByBlocker.constEnd(); ++it) {
        sum += static_cast<int>(it.value());
    }
    return sum;
}

bool GameEventHandler::localCombatDamageAssignmentLegal() const
{
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0) {
        return false;
    }
    const int power = currentCombatDamageAttackerPower();
    if (power <= 0) {
        return false;
    }
    const QList<quint32> blockers = committedBlockerGroups.value(curAtt);
    int sum = 0;
    for (quint32 blk : blockers) {
        sum += static_cast<int>(pendingCombatDamageByBlocker.value(blk, 0));
    }
    return sum == power;
}

void GameEventHandler::sendGameCommand(PendingCommand *pend, int playerId)
{
    AbstractClient *client = game->getClientForPlayer(playerId);
    if (!client)
        return;

    connect(pend, &PendingCommand::finished, this, &GameEventHandler::commandFinished);
    client->sendCommand(pend);
}

void GameEventHandler::sendGameCommand(const google::protobuf::Message &command, int playerId)
{
    AbstractClient *client = game->getClientForPlayer(playerId);
    if (!client)
        return;

    if (game->getGameMetaInfo()->proto().ruled_game() && dynamic_cast<const Command_NextTurn *>(&command)) {
        ruled::v1::RuledCommand ruledCommand;
        // "Pass Turn" is currently the ruled-mode pass-priority button.
        // Always issue pass_priority so AP/NAP cadence is respected on empty stack too.
        ruledCommand.mutable_pass_priority();
        std::string payload;
        if (!ruledCommand.SerializeToString(&payload)) {
            return;
        }
        Command_RuledPayload cmd;
        cmd.set_payload(payload);
        PendingCommand *pend = prepareGameCommand(cmd);
        connect(pend, &PendingCommand::finished, this, &GameEventHandler::commandFinished);
        client->sendCommand(pend);
        return;
    }

    PendingCommand *pend = prepareGameCommand(command);
    connect(pend, &PendingCommand::finished, this, &GameEventHandler::commandFinished);
    client->sendCommand(pend);
}

void GameEventHandler::commandFinished(const Response &response)
{
    if (response.response_code() == Response::RespChatFlood)
        emit gameFlooded();
}

PendingCommand *GameEventHandler::prepareGameCommand(const ::google::protobuf::Message &cmd)
{
    CommandContainer cont;
    cont.set_game_id(static_cast<google::protobuf::uint32>(game->getGameMetaInfo()->gameId()));
    GameCommand *c = cont.add_game_command();
    c->GetReflection()->MutableMessage(c, cmd.GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(cmd);
    return new PendingCommand(cont);
}

PendingCommand *GameEventHandler::prepareGameCommand(const QList<const ::google::protobuf::Message *> &cmdList)
{
    CommandContainer cont;
    cont.set_game_id(static_cast<google::protobuf::uint32>(game->getGameMetaInfo()->gameId()));
    for (auto i : cmdList) {
        GameCommand *c = cont.add_game_command();
        c->GetReflection()->MutableMessage(c, i->GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(*i);
        delete i;
    }
    return new PendingCommand(cont);
}

void GameEventHandler::processGameEventContainer(const GameEventContainer &cont,
                                                 AbstractClient *client,
                                                 EventProcessingOptions options)
{
    Q_UNUSED(client);
    const GameEventContext &context = cont.context();
    emit containerProcessingStarted(context);

    const int eventListSize = cont.event_list_size();
    for (int i = 0; i < eventListSize; ++i) {
        const GameEvent &event = cont.event_list(i);
        const int playerId = event.player_id();
        const auto eventType = static_cast<GameEvent::GameEventType>(getPbExtension(event));

        if (cont.has_forced_by_judge()) {
            auto id = cont.forced_by_judge();
            Player *judgep = game->getPlayerManager()->getPlayers().value(id, nullptr);
            if (judgep) {
                emit setContextJudgeName(judgep->getPlayerInfo()->getName());
            } else if (game->getPlayerManager()->getSpectators().contains(id)) {
                emit setContextJudgeName(
                    QString::fromStdString(game->getPlayerManager()->getSpectators().value(id).name()));
            }
        }

        if (game->getPlayerManager()->getSpectators().contains(playerId)) {
            switch (eventType) {
                case GameEvent::GAME_SAY:
                    eventSpectatorSay(event.GetExtension(Event_GameSay::ext), playerId, context);
                    break;
                case GameEvent::LEAVE:
                    eventSpectatorLeave(event.GetExtension(Event_Leave::ext), playerId, context);
                    break;
                default:
                    break;
            }
        } else {
            switch (eventType) {
                case GameEvent::GAME_STATE_CHANGED:
                    eventGameStateChanged(event.GetExtension(Event_GameStateChanged::ext), playerId, context);
                    break;
                case GameEvent::PLAYER_PROPERTIES_CHANGED:
                    eventPlayerPropertiesChanged(event.GetExtension(Event_PlayerPropertiesChanged::ext), playerId,
                                                 context);
                    break;
                case GameEvent::JOIN:
                    eventJoin(event.GetExtension(Event_Join::ext), playerId, context);
                    break;
                case GameEvent::LEAVE:
                    eventLeave(event.GetExtension(Event_Leave::ext), playerId, context);
                    break;
                case GameEvent::KICKED:
                    eventKicked(event.GetExtension(Event_Kicked::ext), playerId, context);
                    break;
                case GameEvent::GAME_HOST_CHANGED:
                    eventGameHostChanged(event.GetExtension(Event_GameHostChanged::ext), playerId, context);
                    break;
                case GameEvent::GAME_CLOSED:
                    eventGameClosed(event.GetExtension(Event_GameClosed::ext), playerId, context);
                    break;
                case GameEvent::SET_ACTIVE_PLAYER:
                    eventSetActivePlayer(event.GetExtension(Event_SetActivePlayer::ext), playerId, context);
                    break;
                case GameEvent::SET_ACTIVE_PHASE:
                    eventSetActivePhase(event.GetExtension(Event_SetActivePhase::ext), playerId, context);
                    break;
                case GameEvent::REVERSE_TURN:
                    eventReverseTurn(event.GetExtension(Event_ReverseTurn::ext), playerId, context);
                    break;
                case GameEvent::RULED_PAYLOAD: {
                    const Event_RuledPayload &ruled = event.GetExtension(Event_RuledPayload::ext);
                    ruled::v1::RuledEventBatch batch;
                    legalRuledLandPlayHandIndices.clear();
                    legalRuledLandPlayIndicesByCardName.clear();
                    legalRuledSpellCastHandIndices.clear();
                    legalRuledSpellCastIndicesByCardName.clear();
                    legalRuledCleanupDiscardHandIndices.clear();
                    legalRuledCleanupDiscardIndicesByCardName.clear();
                    legalRuledOpeningBottomHandIndices.clear();
                    ruledOpeningPickSeatIds.clear();
                    ruledOpeningUiKind = RuledOpeningUiKind::None;
                    ruledOwnedCardToEngineHandSlot.clear();
                    if (batch.ParseFromString(ruled.payload())) {
                        QString timeline;
                        QString promptFeed;
                        bool combatStateDirty = false;
                        bool battlefieldMapDirty = false;
                        bool ruledStackTrackingDirty = false;
                        for (const auto &e : batch.events()) {
                            if (e.has_log()) {
                                const QString logLine =
                                    QString::fromStdString(e.log().text()).trimmed();
                                if (!logLine.isEmpty()) {
                                    timeline += logLine + QLatin1Char('\n');
                                }
                            }
                            if (e.has_phase_changed()) {
                                const auto &pc = e.phase_changed();
                                lastRuledEnginePhaseSlug = QString::fromStdString(pc.phase());
                                // Phase is already reflected by Event_SetActivePhase from the server
                                // (toolbar highlight + logSetActivePhase); do not duplicate here.
                                // Reaching a new phase guarantees the previous stack emptied.
                                ruledStackObjectIds.clear();
                                ruledStackTargetsByStackOid.clear();
                                ruledStackTrackingDirty = true;
                                if (game->getGameState()->getActivePlayer() != pc.active_player_id()) {
                                    game->getGameState()->setActivePlayer(pc.active_player_id());
                                }
                                const int mappedPhase = mapRuledPhaseSlugToToolbarPhase(QString::fromStdString(pc.phase()));
                                if (mappedPhase >= 0) {
                                    game->getGameState()->setCurrentPhase(mappedPhase);
                                }
                                const RuledCombatPhase combatPhase =
                                    mapRuledPhaseSlugToCombatPhase(QString::fromStdString(pc.phase()));
                                if (combatPhase != currentRuledCombatPhase ||
                                    currentRuledActivePlayerId != pc.active_player_id()) {
                                    const RuledCombatPhase previousCombatPhase = currentRuledCombatPhase;
                                    currentRuledCombatPhase = combatPhase;
                                    currentRuledActivePlayerId = pc.active_player_id();
                                    // Phase transitions reset any local pending selections.
                                    pendingAttackerOids.clear();
                                    pendingBlocks.clear();
                                    remoteBlockPreviewPairs.clear();
                                    remoteAttackerPreviewOids.clear();
                                    stagedBlockerOids.clear();
                                    // Keep committed block assignments visible/interactive while
                                    // progressing from declare blockers -> assign combat damage ->
                                    // combat damage. Clear only when leaving combat.
                                    const auto isCombatPhase = [](RuledCombatPhase phase) {
                                        return phase == RuledCombatPhase::DeclareAttackers ||
                                               phase == RuledCombatPhase::DeclareBlockers ||
                                               phase == RuledCombatPhase::AssignCombatDamage ||
                                               phase == RuledCombatPhase::CombatDamage;
                                    };
                                    if (!isCombatPhase(previousCombatPhase) || !isCombatPhase(combatPhase)) {
                                        committedBlocks.clear();
                                    }
                                    if (combatPhase == RuledCombatPhase::DeclareAttackers) {
                                        attackersSubmittedThisStep = false;
                                    } else if (combatPhase == RuledCombatPhase::DeclareBlockers) {
                                        blockersSubmittedThisStep = false;
                                    } else if (combatPhase == RuledCombatPhase::None) {
                                        attackersSubmittedThisStep = false;
                                        blockersSubmittedThisStep = false;
                                    }
                                    if (combatPhase == RuledCombatPhase::None) {
                                        currentAttackerOids.clear();
                                        remoteAttackerPreviewOids.clear();
                                        clearCombatDamageAssignmentState();
                                        committedBlockerGroups.clear();
                                    }
                                    if (combatPhase == RuledCombatPhase::AssignCombatDamage && localPlayerIsRuledActive()) {
                                        seedDefaultCombatDamageForCurrentAttacker();
                                    }
                                    combatStateDirty = true;
                                }
                            }
                            if (e.has_priority_changed()) {
                                game->getGameState()->setPriorityPlayer(e.priority_changed().player_id());
                                promptFeed += QStringLiteral("Priority: P%1\n")
                                                   .arg(e.priority_changed().player_id());
                            }
                            if (e.has_stack_pushed()) {
                                const auto &sp = e.stack_pushed();
                                ruledStackObjectIds.insert(sp.object_id());
                                QVector<quint32> tlist;
                                tlist.reserve(sp.targets_size());
                                for (int ti = 0; ti < sp.targets_size(); ++ti) {
                                    tlist.append(static_cast<quint32>(sp.targets(ti).object_id()));
                                }
                                ruledStackTargetsByStackOid.insert(sp.object_id(), tlist);
                                ruledStackTrackingDirty = true;
                            }
                            if (e.has_stack_resolved()) {
                                const quint32 rid = e.stack_resolved().object_id();
                                // Countered spells leave the engine stack without their own StackResolved;
                                // remove this spell's stack targets (e.g. the countered object id) so
                                // ruledStackObjectIds matches the real stack (pass button + stack window).
                                const QVector<quint32> spellTargets = ruledStackTargetsByStackOid.value(rid);
                                ruledStackObjectIds.remove(rid);
                                ruledStackTargetsByStackOid.remove(rid);
                                for (quint32 t : spellTargets) {
                                    ruledStackObjectIds.remove(t);
                                }
                                ruledStackTrackingDirty = true;
                            }
                            if (e.has_battlefield_object_map()) {
                                ownerCardIdToEngineOid.clear();
                                engineOidToCardId.clear();
                                engineOidOwner.clear();
                                engineOidSummoningSick.clear();
                                engineOidMarkedDamage.clear();
                                engineOidBattlefieldPower.clear();
                                engineOidBattlefieldToughness.clear();
                                QSet<quint32> validOids;
                                for (const auto &entry : e.battlefield_object_map().entries()) {
                                    validOids.insert(entry.engine_object_id());
                                    engineOidOwner.insert(entry.engine_object_id(), entry.player_id());
                                    engineOidSummoningSick.insert(entry.engine_object_id(), entry.summoning_sick());
                                    if (entry.server_card_id() >= 0) {
                                        ownerCardIdToEngineOid.insert(makeOwnedCardKey(entry.player_id(), entry.server_card_id()),
                                                                      entry.engine_object_id());
                                        engineOidToCardId.insert(entry.engine_object_id(), entry.server_card_id());
                                    }
                                }
                                auto pruneByKnownOid = [&validOids](QHash<quint32, quint32> &pairs) {
                                    for (auto it = pairs.begin(); it != pairs.end();) {
                                        if (!validOids.contains(it.key()) || !validOids.contains(it.value())) {
                                            it = pairs.erase(it);
                                        } else {
                                            ++it;
                                        }
                                    }
                                };
                                pruneByKnownOid(pendingBlocks);
                                pruneByKnownOid(committedBlocks);
                                pruneByKnownOid(remoteBlockPreviewPairs);
                                for (auto it = remoteAttackerPreviewOids.begin();
                                     it != remoteAttackerPreviewOids.end();) {
                                    if (!validOids.contains(*it)) {
                                        it = remoteAttackerPreviewOids.erase(it);
                                    } else {
                                        ++it;
                                    }
                                }
                                for (auto it = stagedBlockerOids.begin(); it != stagedBlockerOids.end();) {
                                    if (!validOids.contains(*it)) {
                                        it = stagedBlockerOids.erase(it);
                                    } else {
                                        ++it;
                                    }
                                }
                                battlefieldMapDirty = true;
                                combatStateDirty = true;
                            }
                            if (e.has_hand_slot_map()) {
                                for (int hi = 0; hi < e.hand_slot_map().entries_size(); ++hi) {
                                    const auto &ent = e.hand_slot_map().entries(hi);
                                    ruledOwnedCardToEngineHandSlot.insert(
                                        makeOwnedCardKey(ent.player_id(), ent.server_card_id()),
                                        static_cast<int>(ent.hand_index()));
                                }
                            }
                            if (e.has_zone_view()) {
                                engineOidMarkedDamage.clear();
                                engineOidBattlefieldPower.clear();
                                engineOidBattlefieldToughness.clear();
                                for (const auto &p : e.zone_view().per_player()) {
                                    const int count = std::min(p.battlefield_object_id_size(), p.battlefield_damage_size());
                                    for (int zdi = 0; zdi < count; ++zdi) {
                                        const quint32 oid = p.battlefield_object_id(zdi);
                                        const int damage = static_cast<int>(p.battlefield_damage(zdi));
                                        if (oid != 0 && damage > 0) {
                                            engineOidMarkedDamage.insert(oid, damage);
                                        }
                                    }
                                    const int nPow = std::min(p.battlefield_object_id_size(), p.battlefield_power_size());
                                    for (int pi = 0; pi < nPow; ++pi) {
                                        const quint32 oid = p.battlefield_object_id(pi);
                                        if (oid != 0) {
                                            engineOidBattlefieldPower.insert(oid,
                                                                              static_cast<int>(p.battlefield_power(pi)));
                                        }
                                    }
                                    const int nTough =
                                        std::min(p.battlefield_object_id_size(), p.battlefield_toughness_size());
                                    for (int ti = 0; ti < nTough; ++ti) {
                                        const quint32 oid = p.battlefield_object_id(ti);
                                        if (oid != 0) {
                                            engineOidBattlefieldToughness.insert(
                                                oid, static_cast<int>(p.battlefield_toughness(ti)));
                                        }
                                    }
                                }
                                battlefieldMapDirty = true;
                            }
                            if (e.has_attackers_declared()) {
                                currentAttackerOids.clear();
                                for (const auto oid : e.attackers_declared().attacker_object_ids()) {
                                    currentAttackerOids.insert(oid);
                                }
                                // Active player's pending picks are now committed; clear them.
                                pendingAttackerOids.clear();
                                remoteAttackerPreviewOids.clear();
                                attackersSubmittedThisStep = true;
                                combatStateDirty = true;
                            }
                            if (e.has_attackers_preview()) {
                                const int declId = e.attackers_preview().declaring_player_id();
                                if (declId != game->getPlayerManager()->getLocalPlayerId()) {
                                    remoteAttackerPreviewOids.clear();
                                    for (int ai = 0; ai < e.attackers_preview().attacker_object_ids_size(); ++ai) {
                                        remoteAttackerPreviewOids.insert(
                                            static_cast<quint32>(e.attackers_preview().attacker_object_ids(ai)));
                                    }
                                }
                                combatStateDirty = true;
                            }
                            if (e.has_blockers_declared()) {
                                committedBlocks.clear();
                                pendingBlocks.clear();
                                remoteBlockPreviewPairs.clear();
                                stagedBlockerOids.clear();
                                committedBlockerGroups.clear();
                                for (int bpi = 0; bpi < e.blockers_declared().block_pairs_size(); ++bpi) {
                                    const auto &bp = e.blockers_declared().block_pairs(bpi);
                                    const auto attOid = static_cast<quint32>(bp.attacker_id());
                                    const auto blkOid = static_cast<quint32>(bp.blocker_id());
                                    committedBlocks.insert(blkOid, attOid);
                                    committedBlockerGroups[attOid].append(blkOid);
                                }
                                // Queue attackers that need explicit combat damage assignment (2+ blockers).
                                clearCombatDamageAssignmentState();
                                for (auto it = committedBlockerGroups.constBegin();
                                     it != committedBlockerGroups.constEnd(); ++it) {
                                    if (it.value().size() > 1) {
                                        combatDamagePendingAttackers.append(it.key());
                                    }
                                }
                                if (!combatDamagePendingAttackers.isEmpty()) {
                                    currentCombatDamageAttackerIdx = 0;
                                    if (localPlayerIsRuledActive()) {
                                        seedDefaultCombatDamageForCurrentAttacker();
                                    }
                                }
                                blockersSubmittedThisStep = true;
                                combatStateDirty = true;
                            }
                            if (e.has_combat_damage_assigned()) {
                                const quint32 doneAtt = e.combat_damage_assigned().attacker_id();
                                if (currentCombatDamageAttackerIdx >= 0 &&
                                    currentCombatDamageAttackerIdx < combatDamagePendingAttackers.size() &&
                                    combatDamagePendingAttackers.at(currentCombatDamageAttackerIdx) == doneAtt) {
                                    pendingCombatDamageByBlocker.clear();
                                    currentCombatDamageAttackerIdx++;
                                    if (localPlayerIsRuledActive() &&
                                        currentCombatDamageAttackerIdx < combatDamagePendingAttackers.size()) {
                                        seedDefaultCombatDamageForCurrentAttacker();
                                    }
                                }
                                combatStateDirty = true;
                            }
                            if (e.has_blockers_preview()) {
                                const int declId = e.blockers_preview().declaring_player_id();
                                if (declId != game->getPlayerManager()->getLocalPlayerId()) {
                                    remoteBlockPreviewPairs.clear();
                                    for (int bpi = 0; bpi < e.blockers_preview().block_pairs_size(); ++bpi) {
                                        const auto &bp = e.blockers_preview().block_pairs(bpi);
                                        remoteBlockPreviewPairs.insert(static_cast<quint32>(bp.blocker_id()),
                                                                       static_cast<quint32>(bp.attacker_id()));
                                    }
                                }
                                combatStateDirty = true;
                            }
                            if (e.has_life_changed()) {
                                const auto &lc = e.life_changed();
                                const QString lifeLine = QStringLiteral("Life: P%1 is now %2 (%3)\n")
                                                             .arg(lc.player_id())
                                                             .arg(lc.new_total())
                                                             .arg(lc.delta());
                                timeline += lifeLine;
                            }
                        }
                        const auto lit =
                            batch.legal_by_player().find(game->getPlayerManager()->getLocalPlayerId());
                        if (lit != batch.legal_by_player().end()) {
                            const ParsedRuledLandActions parsed = parseRuledLandActions(lit->second);
                            legalRuledLandPlayHandIndices = parsed.handIndices;
                            legalRuledLandPlayIndicesByCardName = parsed.handIndicesByCardName;
                            const ParsedRuledCastActions parsedCast = parseRuledCastActions(lit->second);
                            legalRuledSpellCastHandIndices = parsedCast.handIndices;
                            legalRuledSpellCastIndicesByCardName = parsedCast.handIndicesByCardName;
                            const ParsedRuledLandActions parsedCleanup = parseRuledCleanupDiscardActions(lit->second);
                            legalRuledCleanupDiscardHandIndices = parsedCleanup.handIndices;
                            legalRuledCleanupDiscardIndicesByCardName = parsedCleanup.handIndicesByCardName;
                            ruledOpeningUiKind = RuledOpeningUiKind::None;
                            ruledOpeningPickSeatIds.clear();
                            legalRuledOpeningBottomHandIndices.clear();
                            static const QRegularExpression openingBottomRe(
                                QStringLiteral(R"(^Put .+ on bottom \(opening, hand idx (\d+)\)$)"));
                            for (const auto &l : lit->second.labels()) {
                                const QString qs = QString::fromStdString(l);
                                if (const QRegularExpressionMatch bm = openingBottomRe.match(qs); bm.hasMatch()) {
                                    bool ok = false;
                                    const int hi = bm.captured(1).toInt(&ok);
                                    if (ok) {
                                        legalRuledOpeningBottomHandIndices.insert(hi);
                                    }
                                }
                            }
                            if (!legalRuledOpeningBottomHandIndices.isEmpty()) {
                                ruledOpeningUiKind = RuledOpeningUiKind::BottomLibrary;
                            } else {
                                for (const auto &l : lit->second.labels()) {
                                    const QString qs = QString::fromStdString(l);
                                    if (qs == QLatin1String("Keep opening hand (opening)")) {
                                        ruledOpeningUiKind = RuledOpeningUiKind::MulliganChoice;
                                        break;
                                    }
                                }
                            }
                            if (ruledOpeningUiKind == RuledOpeningUiKind::None) {
                                for (const auto &l : lit->second.labels()) {
                                    const QString qs = QString::fromStdString(l);
                                    if (qs == QLatin1String("You start (opening pick)") ||
                                        qs == QLatin1String("Opponent starts (opening pick)")) {
                                        ruledOpeningUiKind = RuledOpeningUiKind::ChooseFirst;
                                        break;
                                    }
                                }
                            }
                            promptFeed += tr("Legal actions:\n");
                            for (const auto &l : lit->second.labels()) {
                                promptFeed += QStringLiteral(" — %1\n").arg(QString::fromStdString(l));
                            }
                        } else {
                            legalRuledLandPlayHandIndices.clear();
                            legalRuledLandPlayIndicesByCardName.clear();
                            legalRuledSpellCastHandIndices.clear();
                            legalRuledSpellCastIndicesByCardName.clear();
                            legalRuledCleanupDiscardHandIndices.clear();
                            legalRuledCleanupDiscardIndicesByCardName.clear();
                            legalRuledOpeningBottomHandIndices.clear();
                            ruledOpeningPickSeatIds.clear();
                            ruledOpeningUiKind = RuledOpeningUiKind::None;
                        }
                        pruneCleanupDiscardSelectionAndEmitUi();
                        if (ruledStackTrackingDirty) {
                            emit ruledStackHasItemsChanged(!ruledStackObjectIds.isEmpty());
                        }
                        emit ruledEngineTimeline(timeline);
                        emit ruledEnginePromptFeed(promptFeed);
                        emit ruledOpeningUiChanged();
                        if (battlefieldMapDirty) {
                            emit ruledBattlefieldMapUpdated();
                        }
                        if (combatStateDirty) {
                            emit ruledCombatStateChanged();
                        }
                        // Defer so stack window / zone views finish layout before we resolve CardItem positions.
                        QTimer::singleShot(0, this, [this] { syncRuledSpellTargetingArrows(); });
                    }
                    break;
                }

                default: {
                    Player *player = game->getPlayerManager()->getPlayers().value(playerId, 0);
                    if (!player) {
                        qCWarning(GameEventHandlerLog) << "unhandled game event: invalid player id";
                        break;
                    }
                    player->getPlayerEventHandler()->processGameEvent(eventType, event, context, options);
                    emitUserEvent();
                }
            }
        }
    }
    emit containerProcessingDone();
}

void GameEventHandler::handleNextTurn()
{
    sendGameCommand(Command_NextTurn());
}

namespace {
void sendRuledCommandFromHandler(GameEventHandler *handler,
                                 AbstractGame *game,
                                 const ruled::v1::RuledCommand &ruledCommand)
{
    AbstractClient *client = game->getClientForPlayer(-1);
    if (!client) {
        return;
    }
    std::string payload;
    if (!ruledCommand.SerializeToString(&payload)) {
        return;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    PendingCommand *pend = handler->prepareGameCommand(cmd);
    QObject::connect(pend, &PendingCommand::finished, handler, &GameEventHandler::commandFinished);
    client->sendCommand(pend);
}
} // namespace

void GameEventHandler::confirmCombatDamageForCurrentAttacker()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0) {
        return;
    }
    if (!localCombatDamageAssignmentLegal()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *acd = ruledCommand.mutable_assign_combat_damage();
    acd->set_attacker_id(curAtt);
    const QList<quint32> blockers = committedBlockerGroups.value(curAtt);
    for (quint32 blk : blockers) {
        auto *pair = acd->add_assignments();
        pair->set_blocker_id(blk);
        pair->set_damage(pendingCombatDamageByBlocker.value(blk, 0));
    }
    sendRuledCommandFromHandler(this, game, ruledCommand);
    emit ruledCombatStateChanged();
}

void GameEventHandler::syncRuledBlockersPreviewToServer()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    if (currentRuledCombatPhase != RuledCombatPhase::DeclareBlockers) {
        return;
    }
    if (blockersSubmittedThisStep) {
        return;
    }
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    if (localId < 0 || currentRuledActivePlayerId < 0) {
        return;
    }
    if (localId == currentRuledActivePlayerId) {
        return;
    }

    ruled::v1::RuledCommand ruledCommand;
    auto *preview = ruledCommand.mutable_preview_declare_blockers();
    for (auto it = pendingBlocks.constBegin(); it != pendingBlocks.constEnd(); ++it) {
        auto *pair = preview->add_block_pairs();
        pair->set_blocker_id(it.key());
        pair->set_attacker_id(it.value());
    }
    sendRuledCommandFromHandler(this, game, ruledCommand);
}

void GameEventHandler::syncRuledAttackersPreviewToServer()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    if (currentRuledCombatPhase != RuledCombatPhase::DeclareAttackers) {
        return;
    }
    if (attackersSubmittedThisStep) {
        return;
    }
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    if (localId < 0 || currentRuledActivePlayerId < 0) {
        return;
    }
    if (localId != currentRuledActivePlayerId) {
        return;
    }

    ruled::v1::RuledCommand ruledCommand;
    auto *preview = ruledCommand.mutable_preview_declare_attackers();
    for (const quint32 oid : pendingAttackerOids) {
        preview->add_creature_ids(oid);
    }
    sendRuledCommandFromHandler(this, game, ruledCommand);
}

void GameEventHandler::handleConfirmRuledAttackers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *declare = ruledCommand.mutable_declare_attackers();
    for (const quint32 oid : pendingAttackerOids) {
        declare->add_creature_ids(oid);
    }
    sendRuledCommandFromHandler(this, game, ruledCommand);
    attackersSubmittedThisStep = true;
    pendingAttackerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleSkipRuledAttackers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_declare_attackers();
    sendRuledCommandFromHandler(this, game, ruledCommand);
    attackersSubmittedThisStep = true;
    pendingAttackerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleConfirmRuledBlockers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *declare = ruledCommand.mutable_declare_blockers();
    for (auto it = pendingBlocks.constBegin(); it != pendingBlocks.constEnd(); ++it) {
        auto *pair = declare->add_block_pairs();
        pair->set_blocker_id(it.key());
        pair->set_attacker_id(it.value());
    }
    sendRuledCommandFromHandler(this, game, ruledCommand);
    blockersSubmittedThisStep = true;
    committedBlocks = pendingBlocks;
    pendingBlocks.clear();
    stagedBlockerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleSkipRuledBlockers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_declare_blockers();
    sendRuledCommandFromHandler(this, game, ruledCommand);
    blockersSubmittedThisStep = true;
    pendingBlocks.clear();
    committedBlocks.clear();
    stagedBlockerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleRuledOpeningPickFirstSeat(int seatId)
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_choose_starting_player()->set_starting_player_id(seatId);
    sendRuledCommandFromHandler(this, game, ruledCommand);
}

void GameEventHandler::handleRuledOpeningMulliganKeep()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_mulligan()->set_keep(true);
    sendRuledCommandFromHandler(this, game, ruledCommand);
}

void GameEventHandler::handleRuledOpeningMulliganRedraw()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_mulligan()->set_keep(false);
    sendRuledCommandFromHandler(this, game, ruledCommand);
}

void GameEventHandler::handleReverseTurn()
{
    sendGameCommand(Command_ReverseTurn());
}

void GameEventHandler::handleActiveLocalPlayerConceded()
{
    sendGameCommand(Command_Concede());
}

void GameEventHandler::handleActiveLocalPlayerUnconceded()
{
    sendGameCommand(Command_Unconcede());
}

void GameEventHandler::handleActivePhaseChanged(int phase)
{
    Command_SetActivePhase cmd;
    cmd.set_phase(static_cast<google::protobuf::uint32>(phase));
    sendGameCommand(cmd);
}

void GameEventHandler::handleGameLeft()
{
    sendGameCommand(Command_LeaveGame());
}

void GameEventHandler::handleChatMessageSent(const QString &chatMessage)
{
    Command_GameSay cmd;
    cmd.set_message(chatMessage.toStdString());
    sendGameCommand(cmd);
}

void GameEventHandler::handleArrowDeletion(int arrowId)
{
    Command_DeleteArrow cmd;
    cmd.set_arrow_id(arrowId);
    sendGameCommand(cmd);
}

void GameEventHandler::eventSpectatorSay(const Event_GameSay &event,
                                         int eventPlayerId,
                                         const GameEventContext & /*context*/)
{
    const ServerInfo_User &userInfo = game->getPlayerManager()->getSpectators().value(eventPlayerId);
    emit logSpectatorSay(userInfo, QString::fromStdString(event.message()));
}

void GameEventHandler::eventSpectatorLeave(const Event_Leave &event,
                                           int eventPlayerId,
                                           const GameEventContext & /*context*/)
{
    emit logSpectatorLeave(game->getPlayerManager()->getSpectatorName(eventPlayerId), getLeaveReason(event.reason()));

    emit spectatorLeft(eventPlayerId);

    game->getPlayerManager()->removeSpectator(eventPlayerId);

    emitUserEvent();
}

void GameEventHandler::eventGameStateChanged(const Event_GameStateChanged &event,
                                             int /*eventPlayerId*/,
                                             const GameEventContext & /*context*/)
{
    const int playerListSize = event.player_list_size();

    QVector<QPair<int, QPair<QString, QString>>> opponentDecksToDisplay;

    for (int i = 0; i < playerListSize; ++i) {
        const ServerInfo_Player &playerInfo = event.player_list(i);
        const ServerInfo_PlayerProperties &prop = playerInfo.properties();
        const int playerId = prop.player_id();
        QString playerName = QString::fromStdString(prop.user_info().name());
        emit addPlayerToAutoCompleteList("@" + playerName);
        if (prop.spectator()) {
            if (!game->getPlayerManager()->getSpectators().contains(playerId)) {
                game->getPlayerManager()->addSpectator(playerId, prop);
                emit spectatorJoined(prop);
            }
        } else {
            Player *player = game->getPlayerManager()->getPlayers().value(playerId, 0);
            if (!player) {
                player = game->getPlayerManager()->addPlayer(playerId, prop.user_info());
                emit playerJoined(prop);
            }
            player->processPlayerInfo(playerInfo);
            if (player->getPlayerInfo()->getLocal()) {
                emit localPlayerDeckSelected(player, playerId, playerInfo);
            } else {
                if (!game->getGameMetaInfo()->proto().share_decklists_on_load()) {
                    continue;
                }

                opponentDecksToDisplay.append(
                    qMakePair(playerId, qMakePair(playerName, QString::fromStdString(playerInfo.deck_list()))));
            }
        }
    }

    processCardAttachmentsForPlayers(event);

    emit remotePlayersDecksSelected(opponentDecksToDisplay);

    game->getGameState()->setGameTime(event.seconds_elapsed());

    if (event.game_started() && !game->getGameMetaInfo()->started()) {
        game->getGameState()->setResuming(!game->getGameState()->isGameStateKnown());
        game->getGameMetaInfo()->setStarted(event.game_started());
        if (game->getGameState()->isGameStateKnown())
            emit logGameStart();
        game->getGameState()->setActivePlayer(event.active_player_id());
        game->getGameState()->setCurrentPhase(event.active_phase());
    } else if (!event.game_started() && game->getGameMetaInfo()->started()) {
        game->getGameState()->setCurrentPhase(-1);
        game->getGameState()->setActivePlayer(-1);
        game->getGameMetaInfo()->setStarted(false);
        emit gameStopped();
    }
    game->getGameState()->setGameStateKnown(true);
    emitUserEvent();
}

void GameEventHandler::processCardAttachmentsForPlayers(const Event_GameStateChanged &event)
{
    for (int i = 0; i < event.player_list_size(); ++i) {
        const ServerInfo_Player &playerInfo = event.player_list(i);
        const ServerInfo_PlayerProperties &prop = playerInfo.properties();
        if (!prop.spectator()) {
            Player *player = game->getPlayerManager()->getPlayers().value(prop.player_id(), 0);
            if (!player)
                continue;
            player->processCardAttachment(playerInfo);
        }
    }
}

void GameEventHandler::eventPlayerPropertiesChanged(const Event_PlayerPropertiesChanged &event,
                                                    int eventPlayerId,
                                                    const GameEventContext &context)
{
    Player *player = game->getPlayerManager()->getPlayers().value(eventPlayerId, 0);
    if (!player)
        return;
    const ServerInfo_PlayerProperties &prop = event.player_properties();
    emit playerPropertiesChanged(prop, eventPlayerId);

    const auto contextType = static_cast<GameEventContext::ContextType>(getPbExtension(context));
    switch (contextType) {
        case GameEventContext::READY_START: {
            bool ready = prop.ready_start();
            if (player->getPlayerInfo()->getLocal())
                emit localPlayerReadyStateChanged(player->getPlayerInfo()->getId(), ready);
            if (ready) {
                emit logReadyStart(player);
            } else {
                emit logNotReadyStart(player);
            }
            break;
        }
        case GameEventContext::CONCEDE: {
            player->setConceded(true);

            QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
            while (playerIterator.hasNext())
                playerIterator.next().value()->updateZones();

            emit logConcede(eventPlayerId);

            break;
        }
        case GameEventContext::UNCONCEDE: {
            player->setConceded(false);

            QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
            while (playerIterator.hasNext())
                playerIterator.next().value()->updateZones();

            emit logUnconcede(eventPlayerId);

            break;
        }
        case GameEventContext::DECK_SELECT: {
            Context_DeckSelect deckSelect = context.GetExtension(Context_DeckSelect::ext);
            emit logDeckSelect(player, QString::fromStdString(deckSelect.deck_hash()), deckSelect.sideboard_size());
            if (game->getGameMetaInfo()->proto().share_decklists_on_load() && deckSelect.has_deck_list() &&
                eventPlayerId != game->getPlayerManager()->getLocalPlayerId()) {
                emit remotePlayerDeckSelected(QString::fromStdString(deckSelect.deck_list()), eventPlayerId,
                                              player->getPlayerInfo()->getName());
            }
            break;
        }
        case GameEventContext::SET_SIDEBOARD_LOCK: {
            if (player->getPlayerInfo()->getLocal()) {
                emit localPlayerSideboardLocked(player->getPlayerInfo()->getId(), prop.sideboard_locked());
            }
            emit logSideboardLockSet(player, prop.sideboard_locked());
            break;
        }
        case GameEventContext::CONNECTION_STATE_CHANGED: {
            emit logConnectionStateChanged(player, prop.ping_seconds() != -1);
            break;
        }
        default:;
    }
}

void GameEventHandler::eventJoin(const Event_Join &event, int /*eventPlayerId*/, const GameEventContext & /*context*/)
{
    const ServerInfo_PlayerProperties &playerInfo = event.player_properties();
    const int playerId = playerInfo.player_id();
    QString playerName = QString::fromStdString(playerInfo.user_info().name());
    emit addPlayerToAutoCompleteList(playerName);

    if (game->getPlayerManager()->getPlayers().contains(playerId))
        return;

    if (playerInfo.spectator()) {
        game->getPlayerManager()->addSpectator(playerId, playerInfo);
        emit logJoinSpectator(playerName);
        emit spectatorJoined(playerInfo);
    } else {
        Player *newPlayer = game->getPlayerManager()->addPlayer(playerId, playerInfo.user_info());
        emit logJoinPlayer(newPlayer);
        emit playerJoined(playerInfo);
    }

    emitUserEvent();
}

QString GameEventHandler::getLeaveReason(Event_Leave::LeaveReason reason)
{
    switch (reason) {
        case Event_Leave::USER_KICKED:
            return tr("kicked by game host or moderator");
            break;
        case Event_Leave::USER_LEFT:
            return tr("player left the game");
            break;
        case Event_Leave::USER_DISCONNECTED:
            return tr("player disconnected from server");
            break;
        case Event_Leave::OTHER:
        default:
            return tr("reason unknown");
            break;
    }
}
void GameEventHandler::eventLeave(const Event_Leave &event, int eventPlayerId, const GameEventContext & /*context*/)
{
    Player *player = game->getPlayerManager()->getPlayers().value(eventPlayerId, 0);
    if (!player)
        return;

    player->clear();
    emit playerLeft(eventPlayerId);

    emit logLeave(player, getLeaveReason(event.reason()));

    game->getPlayerManager()->removePlayer(eventPlayerId);

    player->deleteLater();

    // Rearrange all remaining zones so that attachment relationship updates take place
    QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
    while (playerIterator.hasNext())
        playerIterator.next().value()->updateZones();

    emitUserEvent();
}

void GameEventHandler::eventKicked(const Event_Kicked & /*event*/,
                                   int /*eventPlayerId*/,
                                   const GameEventContext & /*context*/)
{
    emit gameClosed();
    emit logKicked();
    emit playerKicked();
    emitUserEvent();
}

void GameEventHandler::eventReverseTurn(const Event_ReverseTurn &event,
                                        int eventPlayerId,
                                        const GameEventContext & /*context*/)
{
    Player *player = game->getPlayerManager()->getPlayers().value(eventPlayerId, 0);
    if (!player)
        return;

    emit logTurnReversed(player, event.reversed());
}

void GameEventHandler::eventGameHostChanged(const Event_GameHostChanged & /*event*/,
                                            int eventPlayerId,
                                            const GameEventContext & /*context*/)
{
    game->getGameState()->setHostId(eventPlayerId);
}

void GameEventHandler::eventGameClosed(const Event_GameClosed & /*event*/,
                                       int /*eventPlayerId*/,
                                       const GameEventContext & /*context*/)
{
    game->getGameMetaInfo()->setStarted(false);
    game->getGameState()->setGameClosed(true);
    emit gameClosed();
    emit logGameClosed();
    emitUserEvent();
}

void GameEventHandler::eventSetActivePlayer(const Event_SetActivePlayer &event,
                                            int /*eventPlayerId*/,
                                            const GameEventContext & /*context*/)
{
    game->getGameState()->setActivePlayer(event.active_player_id());
    Player *player = game->getPlayerManager()->getPlayer(event.active_player_id());
    if (!player)
        return;
    emit logActivePlayer(player);
    emitUserEvent();
}

void GameEventHandler::eventSetActivePhase(const Event_SetActivePhase &event,
                                           int /*eventPlayerId*/,
                                           const GameEventContext & /*context*/)
{
    const int phase = event.phase();
    if (game->getGameState()->getCurrentPhase() != phase) {
        emit logActivePhaseChanged(phase);
    }
    game->getGameState()->setCurrentPhase(phase);
    emitUserEvent();
}

void GameEventHandler::refreshRuledSpellTargetArrows()
{
    syncRuledSpellTargetingArrows();
}

void GameEventHandler::clearRuledSpellTargetArrows()
{
    for (const auto &pr : ruledSpellTargetSyntheticArrows) {
        if (pr.first) {
            pr.first->delArrow(pr.second);
        }
    }
    ruledSpellTargetSyntheticArrows.clear();
}

void GameEventHandler::syncRuledSpellTargetingArrows()
{
    if (!game || !game->getGameMetaInfo()->proto().ruled_game()) {
        clearRuledSpellTargetArrows();
        return;
    }

    clearRuledSpellTargetArrows();

    static const QColor spellTargetRed(220, 40, 40);

    for (auto it = ruledStackTargetsByStackOid.constBegin(); it != ruledStackTargetsByStackOid.constEnd(); ++it) {
        const quint32 stackOid = it.key();
        if (!ruledStackObjectIds.contains(stackOid)) {
            continue;
        }
        const int spellServerId = cardIdForEngineOid(stackOid);
        TabGame *tab = game->getTab();
        CardItem *startCard = tab ? tab->findVisibleStackSpellCardItem(spellServerId) : nullptr;
        if (!startCard) {
            startCard = findStackCardByServerId(game, spellServerId);
        }
        if (!startCard) {
            continue;
        }
        Player *arrowOwner = startCard->getZone()->getPlayer();
        if (!arrowOwner) {
            continue;
        }

        const QVector<quint32> targets = it.value();
        for (int ti = 0; ti < targets.size(); ++ti) {
            ArrowTarget *tgt =
                resolveRuledSpellTarget(game, tab, this, ruledStackObjectIds, targets.at(ti));
            if (!tgt || tgt == startCard) {
                continue;
            }
            const int aid = nextRuledSpellTargetArrowId--;
            ArrowItem *arr = arrowOwner->addArrow(aid, startCard, tgt, spellTargetRed);
            if (!arr) {
                continue;
            }
            arr->setAcceptedMouseButtons(Qt::NoButton);
            ruledSpellTargetSyntheticArrows.append(qMakePair(arrowOwner, aid));
        }
    }
}
