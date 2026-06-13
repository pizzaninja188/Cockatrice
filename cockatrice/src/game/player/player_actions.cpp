#include "player_actions.h"

#include "../game/game_event_handler.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "../../interface/widgets/utility/get_text_with_max.h"
#include "../board/abstract_counter.h"
#include "../board/card_item.h"
#include "../client/settings/card_counter_settings.h"
#include "../dialogs/dlg_move_top_cards_until.h"
#include "../dialogs/dlg_roll_dice.h"
#include "../zones/hand_zone.h"
#include "../zones/logic/view_zone_logic.h"
#include "../zones/table_zone.h"
#include "card_menu_action_type.h"

#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/card/relation/card_relation.h>
#include <libcockatrice/protocol/pb/command_attach_card.pb.h>
#include <libcockatrice/protocol/pb/command_change_zone_properties.pb.h>
#include <libcockatrice/protocol/pb/command_create_token.pb.h>
#include <libcockatrice/protocol/pb/command_draw_cards.pb.h>
#include <libcockatrice/protocol/pb/command_flip_card.pb.h>
#include <libcockatrice/protocol/pb/command_game_say.pb.h>
#include <libcockatrice/protocol/pb/command_inc_counter.pb.h>
#include <libcockatrice/protocol/pb/command_move_card.pb.h>
#include <libcockatrice/protocol/pb/command_mulligan.pb.h>
#include <libcockatrice/protocol/pb/command_reveal_cards.pb.h>
#include <libcockatrice/protocol/pb/command_roll_die.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/command_set_card_attr.pb.h>
#include <libcockatrice/protocol/pb/command_set_card_counter.pb.h>
#include <libcockatrice/protocol/pb/command_shuffle.pb.h>
#include <libcockatrice/protocol/pb/command_undo_draw.pb.h>
#include <libcockatrice/protocol/pb/context_move_card.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/utility/expression.h>
#include <libcockatrice/utility/trice_limits.h>
#include <libcockatrice/utility/zone_names.h>

#include <QInputDialog>
#include <QMenu>
#include <QMessageBox>
#include <QPushButton>

// milliseconds in between triggers of the move top cards until action
static constexpr int MOVE_TOP_CARD_UNTIL_INTERVAL = 100;

namespace {

// Builds a Command_RuledPayload that decrements the engine's mana pool by 1 for the given counter
// color. Returns nullptr if the counter name doesn't map to a known mana color (nothing to send).
Command_RuledPayload *buildManaPoolDecrementPayload(const QString &counterName)
{
    ruled::v1::RuledCommand rc;
    auto *m = rc.mutable_add_mana_to_pool();
    const QString n = counterName.toLower();
    if (n == QLatin1String("w")) {
        m->set_w(-1);
    } else if (n == QLatin1String("u")) {
        m->set_u(-1);
    } else if (n == QLatin1String("b")) {
        m->set_b(-1);
    } else if (n == QLatin1String("r")) {
        m->set_r(-1);
    } else if (n == QLatin1String("g")) {
        m->set_g(-1);
    } else if (n == QLatin1String("c") || n == QLatin1String("x")) {
        m->set_c(-1);
    } else {
        return nullptr;
    }
    std::string payload;
    if (!rc.SerializeToString(&payload)) {
        return nullptr;
    }
    auto *cmd = new Command_RuledPayload;
    cmd->set_payload(payload);
    return cmd;
}

// +1 for each pip; mirrors server `cmdIncCounter` positive delta → engine pool (no Cockatrice counter).
Command_RuledPayload *buildManaPoolIncrementPayload(const QString &counterName)
{
    ruled::v1::RuledCommand rc;
    auto *m = rc.mutable_add_mana_to_pool();
    const QString n = counterName.toLower();
    if (n == QLatin1String("w")) {
        m->set_w(1);
    } else if (n == QLatin1String("u")) {
        m->set_u(1);
    } else if (n == QLatin1String("b")) {
        m->set_b(1);
    } else if (n == QLatin1String("r")) {
        m->set_r(1);
    } else if (n == QLatin1String("g")) {
        m->set_g(1);
    } else if (n == QLatin1String("c") || n == QLatin1String("x")) {
        m->set_c(1);
    } else {
        return nullptr;
    }
    std::string payload;
    if (!rc.SerializeToString(&payload)) {
        return nullptr;
    }
    auto *cmd = new Command_RuledPayload;
    cmd->set_payload(payload);
    return cmd;
}
} // namespace

PlayerActions::PlayerActions(Player *_player)
    : QObject(_player), player(_player), lastTokenTableRow(0), movingCardsUntil(false)
{
    moveTopCardTimer = new QTimer(this);
    moveTopCardTimer->setInterval(MOVE_TOP_CARD_UNTIL_INTERVAL);
    moveTopCardTimer->setSingleShot(true);
    connect(moveTopCardTimer, &QTimer::timeout, [this]() { actMoveTopCardToPlay(); });
}

QMap<QChar, int> PlayerActions::parseSimpleManaCost(const QString &manaCost)
{
    QMap<QChar, int> parsed;
    auto addSymbol = [&parsed](QChar c) {
        const QChar sym = c.toUpper();
        if (QStringLiteral("WUBRGCX").contains(sym)) {
            parsed[sym] += 1;
        }
    };

    bool inBraces = false;
    QString token;
    for (QChar c : manaCost) {
        if (c == '{') {
            inBraces = true;
            token.clear();
            continue;
        }
        if (c == '}') {
            inBraces = false;
            // Numeric tokens are generic mana of any length ({1}, {4}, {10}); only fall back
            // to single-symbol parsing for non-numeric tokens ({G}, {C}, {X}). The previous
            // size()==1 check routed single digits to addSymbol, which silently dropped them.
            bool ok = false;
            const int generic = token.toInt(&ok);
            if (ok) {
                if (generic > 0) {
                    parsed['X'] += generic;
                }
            } else if (token.size() == 1) {
                addSymbol(token.at(0));
            }
            token.clear();
            continue;
        }
        if (inBraces) {
            token.append(c);
            continue;
        }
        if (c.isDigit()) {
            parsed['X'] += c.digitValue();
            continue;
        }
        addSymbol(c);
    }
    return parsed;
}

QString PlayerActions::formatSimpleManaCost(const QMap<QChar, int> &cost)
{
    // Render in canonical Scryfall brace form ({4}{G}{G}). The brackets double as a
    // placeholder for real mana symbols later, so they are kept rather than stripped.
    QString out;
    const int generic = cost.value('X', 0);
    if (generic > 0) {
        out += QStringLiteral("{%1}").arg(generic);
    }
    for (QChar c : QStringLiteral("WUBRGC")) {
        const int count = cost.value(c, 0);
        for (int i = 0; i < count; ++i) {
            out += QStringLiteral("{%1}").arg(c);
        }
    }
    return out;
}

QVector<PlayerActions::RuledFlexPip> PlayerActions::parseFlexPips(const QString &manaCost)
{
    // Walk the Scryfall brace groups in order so each pip's index matches the engine's
    // ManaCost pip order. Flexible pips (CR 107.4d–f) contain a slash: {G/U} hybrid,
    // {2/W} mono-hybrid, {C/P} Phyrexian. Everything else just advances the index.
    QVector<RuledFlexPip> out;
    const QString validColors = QStringLiteral("WUBRG");
    quint32 index = 0;
    bool inBraces = false;
    QString token;
    for (QChar c : manaCost) {
        if (c == '{') {
            inBraces = true;
            token.clear();
            continue;
        }
        if (c == '}') {
            inBraces = false;
            const int slash = token.indexOf('/');
            if (slash > 0) {
                const QString left = token.left(slash).toUpper();
                const QString right = token.mid(slash + 1).toUpper();
                RuledFlexPip pip;
                pip.pipIndex = index;
                bool numeric = false;
                const int leftNum = left.toInt(&numeric);
                if (right == QLatin1String("P") && left.size() == 1 && validColors.contains(left)) {
                    pip.phyrexian = true;
                    pip.colorA = left.at(0);
                    out.append(pip);
                } else if (numeric && right.size() == 1 && validColors.contains(right)) {
                    pip.generic = leftNum;
                    pip.colorA = right.at(0);
                    out.append(pip);
                } else if (left.size() == 1 && right.size() == 1 && validColors.contains(left) &&
                           validColors.contains(right)) {
                    pip.colorA = left.at(0);
                    pip.colorB = right.at(0);
                    out.append(pip);
                }
                // Unrecognized slash forms (e.g. {G/U/P}, {S}) are left for the engine to reject.
            }
            ++index;
            token.clear();
            continue;
        }
        if (inBraces) {
            token.append(c);
        }
    }
    return out;
}

bool PlayerActions::resolveFlexiblePipsForPendingSpell(const QString &rawCost, const QString &cardName)
{
    const QVector<RuledFlexPip> flexPips = parseFlexPips(rawCost);
    for (const RuledFlexPip &pip : flexPips) {
        QMessageBox box;
        box.setIcon(QMessageBox::Question);
        box.setWindowTitle(tr("Pay hybrid mana"));
        QPushButton *colorBtn = nullptr;
        QPushButton *altBtn = nullptr;
        if (pip.phyrexian) {
            // CR 107.4f: pay the color OR 2 life.
            box.setText(tr("Pay {%1/P} for %2:").arg(pip.colorA).arg(cardName));
            colorBtn = box.addButton(tr("Pay {%1}").arg(pip.colorA), QMessageBox::AcceptRole);
            altBtn = box.addButton(tr("Pay 2 life"), QMessageBox::AcceptRole);
        } else if (pip.generic > 0) {
            // CR 107.4e: mono-hybrid — pay the color OR N generic.
            box.setText(tr("Pay {%1/%2} for %3:").arg(pip.generic).arg(pip.colorA).arg(cardName));
            colorBtn = box.addButton(tr("Pay {%1}").arg(pip.colorA), QMessageBox::AcceptRole);
            altBtn = box.addButton(tr("Pay {%1} generic").arg(pip.generic), QMessageBox::AcceptRole);
        } else {
            // CR 107.4d: hybrid — pay either color.
            box.setText(tr("Pay {%1/%2} for %3:").arg(pip.colorA).arg(pip.colorB).arg(cardName));
            colorBtn = box.addButton(tr("Pay {%1}").arg(pip.colorA), QMessageBox::AcceptRole);
            altBtn = box.addButton(tr("Pay {%1}").arg(pip.colorB), QMessageBox::AcceptRole);
        }
        QPushButton *cancelBtn = box.addButton(QMessageBox::Cancel);
        box.exec();
        QAbstractButton *clicked = box.clickedButton();
        if (clicked == cancelBtn || clicked == nullptr) {
            return false;
        }
        if (clicked == colorBtn) {
            pendingRuledSpellCast.remainingCost[pip.colorA.toUpper()] += 1;
        } else if (clicked == altBtn) {
            if (pip.phyrexian) {
                pendingRuledSpellCast.lifePipIndices.append(pip.pipIndex);
            } else if (pip.generic > 0) {
                pendingRuledSpellCast.remainingCost[QChar('X')] += pip.generic;
            } else {
                pendingRuledSpellCast.remainingCost[pip.colorB.toUpper()] += 1;
            }
        }
    }
    return true;
}

QString PlayerActions::pendingRuledSpellPromptText() const
{
    if (!pendingRuledSpellCast.valid || pendingRuledSpellCast.waitingForTarget) {
        return {};
    }
    int total = 0;
    for (auto it = pendingRuledSpellCast.remainingCost.constBegin(); it != pendingRuledSpellCast.remainingCost.constEnd();
         ++it) {
        total += it.value();
    }
    if (total == 0) {
        return {};
    }
    return tr("Pay mana for %1: %2 remaining (click mana counters).")
        .arg(pendingRuledSpellCast.cardName, formatSimpleManaCost(pendingRuledSpellCast.remainingCost));
}

void PlayerActions::clearPendingRuledSpellCast()
{
    const bool hadTargeting = pendingRuledSpellCast.valid && pendingRuledSpellCast.waitingForTarget;
    const bool hadPending = pendingRuledSpellCast.valid;
    pendingRuledSpellCast = PendingRuledSpellCast{};
    if (hadTargeting) {
        emit ruledSpellTargetingChanged(false, {});
    }
    if (hadPending) {
        emit ruledSpellCastPendingChanged(false);
    }
}
void PlayerActions::cancelPendingRuledSpellCast()
{
    if (!pendingRuledSpellCast.valid) {
        return;
    }
    const QString cardName = pendingRuledSpellCast.cardName;

    QList<const ::google::protobuf::Message *> cmdList;

    // Refund mana paid toward the spell (last-in first-out) BEFORE undoing taps so counters
    // never dip below zero. Payments were delta=-1; refund is delta=+1.
    for (int i = manaPaymentCounterIds.size() - 1; i >= 0; --i) {
        const int cid = manaPaymentCounterIds[i];
        auto *counterCmd = new Command_IncCounter;
        counterCmd->set_counter_id(cid);
        counterCmd->set_delta(1);
        cmdList.append(counterCmd);
    }
    manaPaymentCounterIds.clear();

    // Undo any lands tapped for mana after the spell was initiated.
    for (int i = midCastLandTapStack.size() - 1; i >= 0; --i) {
        const LandTapUndoEntry &entry = midCastLandTapStack[i];
        CardItem *card = player->getTableZone()->getCards().findCard(entry.cardId);
        if (card) {
            card->setTapped(false, true);
            auto *attrCmd = new Command_SetCardAttr;
            attrCmd->set_zone(ZoneNames::TABLE);
            attrCmd->set_card_id(entry.cardId);
            attrCmd->set_attribute(AttrTapped);
            attrCmd->set_attr_value("0");
            cmdList.append(attrCmd);
        }
        if (entry.counterId >= 0) {
            if (auto *counter = player->getCounters().value(entry.counterId, nullptr)) {
                counter->setValue(counter->getValue() - 1);
            }
            auto *counterCmd = new Command_IncCounter;
            counterCmd->set_counter_id(entry.counterId);
            counterCmd->set_delta(-1);
            cmdList.append(counterCmd);
            if (player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
                if (Command_RuledPayload *poolCmd = buildManaPoolDecrementPayload(entry.counterName)) {
                    cmdList.append(poolCmd);
                }
            }
        } else if (player->getGame()->getGameMetaInfo()->proto().ruled_game() && !entry.counterName.isEmpty()) {
            // Land paid spell cost without IncCounter; engine pool was incremented — remove it on cancel.
            if (Command_RuledPayload *poolCmd = buildManaPoolDecrementPayload(entry.counterName)) {
                cmdList.append(poolCmd);
            }
        }
    }
    midCastLandTapStack.clear();

    if (!cmdList.isEmpty()) {
        sendGameCommand(prepareGameCommand(cmdList));
    }

    clearPendingRuledSpellCast();
    emit landTapUndoAvailableChanged(!landTapUndoStack.isEmpty());
    player->getGame()->getGameEventHandler()->emitLocalRuledLog(tr("Canceled casting %1.").arg(cardName));
}

void PlayerActions::recordLandTapUndo(int cardId, const QString &counterName, int counterId)
{
    if (pendingRuledSpellCast.valid || pendingActivatedAbility.waitingForMana) {
        midCastLandTapStack.append({cardId, counterName, counterId});
        return;
    }
    const bool hadEntries = !landTapUndoStack.isEmpty();
    landTapUndoStack.append({cardId, counterName, counterId});
    if (!hadEntries) {
        emit landTapUndoAvailableChanged(true);
    }
}

void PlayerActions::undoLastLandTap()
{
    if (landTapUndoStack.isEmpty()) {
        return;
    }
    const LandTapUndoEntry entry = landTapUndoStack.takeLast();

    QList<const ::google::protobuf::Message *> cmdList;

    CardItem *card = player->getTableZone()->getCards().findCard(entry.cardId);
    if (card) {
        card->setTapped(false, true);
        auto *attrCmd = new Command_SetCardAttr;
        attrCmd->set_zone(ZoneNames::TABLE);
        attrCmd->set_card_id(entry.cardId);
        attrCmd->set_attribute(AttrTapped);
        attrCmd->set_attr_value("0");
        cmdList.append(attrCmd);
    }

    if (entry.counterId >= 0) {
        if (auto *counter = player->getCounters().value(entry.counterId, nullptr)) {
            counter->setValue(counter->getValue() - 1);
        }
        auto *counterCmd = new Command_IncCounter;
        counterCmd->set_counter_id(entry.counterId);
        counterCmd->set_delta(-1);
        cmdList.append(counterCmd);
        if (player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
            if (Command_RuledPayload *poolCmd = buildManaPoolDecrementPayload(entry.counterName)) {
                cmdList.append(poolCmd);
            }
        }
    }

    if (!cmdList.isEmpty()) {
        sendGameCommand(prepareGameCommand(cmdList));
    }

    emit landTapUndoAvailableChanged(!landTapUndoStack.isEmpty());
}

void PlayerActions::clearLandTapUndoStack()
{
    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();
    if (landTapUndoStack.isEmpty()) {
        return;
    }
    landTapUndoStack.clear();
    emit landTapUndoAvailableChanged(false);
}


bool PlayerActions::completePendingRuledSpellCast()
{
    if (!pendingRuledSpellCast.valid || pendingRuledSpellCast.handIndex < 0) {
        clearPendingRuledSpellCast();
        return false;
    }
    if (pendingRuledSpellCast.waitingForTarget) {
        return false;
    }

    ruled::v1::RuledCommand ruledCommand;
    auto *cast = ruledCommand.mutable_cast_spell();
    cast->set_hand_card_index(pendingRuledSpellCast.handIndex);
    cast->set_x_value(static_cast<quint32>(pendingRuledSpellCast.xValue));
    for (const quint32 targetOid : pendingRuledSpellCast.selectedTargetOids) {
        auto *target = cast->add_targets();
        target->set_object_id(targetOid);
    }
    // CR 107.4f: Phyrexian pips the player chose to pay with life.
    for (const quint32 pipIndex : pendingRuledSpellCast.lifePipIndices) {
        auto *flex = cast->add_flex_payments();
        flex->set_pip_index(pipIndex);
        flex->set_pay_life(true);
    }
    std::string payload;
    if (!ruledCommand.SerializeToString(&payload)) {
        clearPendingRuledSpellCast();
        return false;
    }

    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    sendGameCommand(cmd);

    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();
    clearLandTapUndoStack();
    clearPendingRuledSpellCast();
    return true;
}


bool PlayerActions::completeActivateAbility()
{
    if (!pendingActivatedAbility.valid || pendingActivatedAbility.waitingForTarget ||
        pendingActivatedAbility.waitingForMana) {
        return false;
    }

    ruled::v1::RuledCommand cmd;
    auto *aa = cmd.mutable_activate_ability();
    aa->set_permanent_id(pendingActivatedAbility.permanentOid);
    aa->set_ability_index(static_cast<uint32_t>(pendingActivatedAbility.abilityIndex));
    if (pendingActivatedAbility.needsTarget) {
        auto *tref = aa->add_targets();
        tref->set_object_id(pendingActivatedAbility.selectedTargetOid);
    }
    std::string payload;
    if (!cmd.SerializeToString(&payload)) {
        pendingActivatedAbility = {};
        return false;
    }
    Command_RuledPayload ruledPayload;
    ruledPayload.set_payload(payload);
    sendGameCommand(ruledPayload);

    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();
    clearLandTapUndoStack();
    emit ruledAbilityActivationPendingChanged(false);
    pendingActivatedAbility = {};
    return true;
}

bool PlayerActions::tryReducePendingAbilityRemainingCostOnePip(bool colorlessMana, QChar coloredMana)
{
    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForMana) {
        return false;
    }
    if (colorlessMana) {
        if (pendingActivatedAbility.remainingCost.value('X', 0) > 0) {
            pendingActivatedAbility.remainingCost['X'] -= 1;
        } else if (pendingActivatedAbility.remainingCost.value('C', 0) > 0) {
            pendingActivatedAbility.remainingCost['C'] -= 1;
        } else {
            return false;
        }
    } else {
        const QChar sym = coloredMana.toUpper();
        if (pendingActivatedAbility.remainingCost.value(sym, 0) > 0) {
            pendingActivatedAbility.remainingCost[sym] -= 1;
        } else if (pendingActivatedAbility.remainingCost.value('X', 0) > 0) {
            pendingActivatedAbility.remainingCost['X'] -= 1;
        } else {
            return false;
        }
    }
    return true;
}

void PlayerActions::finishPendingAbilityManaPaymentStep()
{
    int totalRemaining = 0;
    for (auto it = pendingActivatedAbility.remainingCost.constBegin();
         it != pendingActivatedAbility.remainingCost.constEnd(); ++it) {
        totalRemaining += it.value();
    }
    if (totalRemaining == 0) {
        pendingActivatedAbility.waitingForMana = false;
        completeActivateAbility();
        return;
    }
    emit ruledAbilityManaPromptChanged();
    player->getGame()->getGameEventHandler()->emitLocalRuledLog(
        tr("Pay mana for %1: %2 remaining (tap your lands).")
            .arg(pendingActivatedAbility.cardName,
                 formatSimpleManaCost(pendingActivatedAbility.remainingCost)));
}

bool PlayerActions::tryReducePendingSpellRemainingCostOnePip(bool colorlessMana, QChar coloredMana)
{
    if (!pendingRuledSpellCast.valid || pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (colorlessMana) {
        if (pendingRuledSpellCast.remainingCost.value('X', 0) > 0) {
            pendingRuledSpellCast.remainingCost['X'] -= 1;
        } else if (pendingRuledSpellCast.remainingCost.value('C', 0) > 0) {
            pendingRuledSpellCast.remainingCost['C'] -= 1;
        } else {
            return false;
        }
    } else {
        const QChar sym = coloredMana.toUpper();
        if (pendingRuledSpellCast.remainingCost.value(sym, 0) > 0) {
            pendingRuledSpellCast.remainingCost[sym] -= 1;
        } else if (pendingRuledSpellCast.remainingCost.value('X', 0) > 0) {
            pendingRuledSpellCast.remainingCost['X'] -= 1;
        } else {
            return false;
        }
    }
    return true;
}

void PlayerActions::finishPendingSpellManaPaymentStep()
{
    int totalRemaining = 0;
    for (auto it = pendingRuledSpellCast.remainingCost.constBegin(); it != pendingRuledSpellCast.remainingCost.constEnd();
         ++it) {
        totalRemaining += it.value();
    }
    if (totalRemaining == 0) {
        completePendingRuledSpellCast();
        return;
    }
    emit ruledSpellManaPromptChanged();
    player->getGame()->getGameEventHandler()->emitLocalRuledLog(
        tr("Pay mana for %1: %2 remaining (click mana counters).")
            .arg(pendingRuledSpellCast.cardName, formatSimpleManaCost(pendingRuledSpellCast.remainingCost)));
}

QPair<bool, bool> PlayerActions::tryConsumeLandManaPipTowardPendingSpell(const QString &manaCounterName)
{
    if (manaCounterName.trimmed().isEmpty()) {
        return {false, false};
    }
    const QString rawLower = manaCounterName.trimmed().toLower();
    const bool colorlessOnly = (rawLower == QLatin1String("x") || rawLower == QLatin1String("c"));
    QChar sym;
    if (!colorlessOnly) {
        if (rawLower.size() != 1) {
            return {false, false};
        }
        const QChar c = rawLower.at(0).toUpper();
        if (!QStringLiteral("WUBRGC").contains(c)) {
            return {false, false};
        }
        sym = c;
    } else {
        sym = QChar();
    }

    if (!tryReducePendingSpellRemainingCostOnePip(colorlessOnly, sym)) {
        return {false, false};
    }
    int totalRemaining = 0;
    for (auto it = pendingRuledSpellCast.remainingCost.constBegin(); it != pendingRuledSpellCast.remainingCost.constEnd();
         ++it) {
        totalRemaining += it.value();
    }
    return {true, totalRemaining == 0};
}

void PlayerActions::afterRuledLandTapsAppliedForSpellMana(bool completeCast, bool partialCostRemainPrompt)
{
    if (completeCast) {
        completePendingRuledSpellCast();
    } else if (partialCostRemainPrompt) {
        emit ruledSpellManaPromptChanged();
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("Pay mana for %1: %2 remaining (click mana counters).")
                .arg(pendingRuledSpellCast.cardName, formatSimpleManaCost(pendingRuledSpellCast.remainingCost)));
    }
}

QPair<bool, bool> PlayerActions::tryConsumeLandManaPipTowardPendingAbility(const QString &manaCounterName)
{
    if (manaCounterName.trimmed().isEmpty()) {
        return {false, false};
    }
    const QString rawLower = manaCounterName.trimmed().toLower();
    const bool colorlessOnly = (rawLower == QLatin1String("x") || rawLower == QLatin1String("c"));
    QChar sym;
    if (!colorlessOnly) {
        if (rawLower.size() != 1) {
            return {false, false};
        }
        const QChar c = rawLower.at(0).toUpper();
        if (!QStringLiteral("WUBRGC").contains(c)) {
            return {false, false};
        }
        sym = c;
    } else {
        sym = QChar();
    }

    if (!tryReducePendingAbilityRemainingCostOnePip(colorlessOnly, sym)) {
        return {false, false};
    }
    int totalRemaining = 0;
    for (auto it = pendingActivatedAbility.remainingCost.constBegin();
         it != pendingActivatedAbility.remainingCost.constEnd(); ++it) {
        totalRemaining += it.value();
    }
    return {true, totalRemaining == 0};
}

void PlayerActions::afterRuledLandTapsAppliedForAbilityMana(bool completeActivation, bool partialCostRemainPrompt)
{
    if (completeActivation) {
        pendingActivatedAbility.waitingForMana = false;
        completeActivateAbility();
    } else if (partialCostRemainPrompt) {
        emit ruledAbilityManaPromptChanged();
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("Pay mana for %1: %2 remaining (tap your lands).")
                .arg(pendingActivatedAbility.cardName,
                     formatSimpleManaCost(pendingActivatedAbility.remainingCost)));
    }
}

bool PlayerActions::tryPayRuledAbilityWithCounter(const QString &counterName)
{
    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForMana) {
        return false;
    }
    const QString rawLower = counterName.trimmed().toLower();
    const bool colorlessOnly = (rawLower == QLatin1String("x") || rawLower == QLatin1String("c"));
    QChar sym;
    if (!colorlessOnly) {
        const QString n = counterName.trimmed().toUpper();
        if (n.size() != 1 || !QStringLiteral("WUBRGC").contains(n.at(0))) {
            return false;
        }
        sym = n.at(0);
    } else {
        sym = QChar();
    }

    int counterId = -1;
    for (auto it = player->getCounters().constBegin(); it != player->getCounters().constEnd(); ++it) {
        if (it.value() &&
            it.value()->getName().trimmed().compare(counterName.trimmed(), Qt::CaseInsensitive) == 0) {
            counterId = it.key();
            break;
        }
    }
    if (counterId < 0) {
        return false;
    }

    if (!tryReducePendingAbilityRemainingCostOnePip(colorlessOnly, sym)) {
        return false;
    }

    manaPaymentCounterIds.append(counterId);
    Command_IncCounter cmd;
    cmd.set_counter_id(counterId);
    cmd.set_delta(-1);
    sendGameCommand(cmd);
    finishPendingAbilityManaPaymentStep();
    return true;
}

void PlayerActions::cancelPendingActivatedAbility()
{
    if (!pendingActivatedAbility.valid) {
        return;
    }
    const QString abilityText = pendingActivatedAbility.abilityText;
    const QString cardName = pendingActivatedAbility.cardName;

    QList<const ::google::protobuf::Message *> cmdList;

    for (int i = manaPaymentCounterIds.size() - 1; i >= 0; --i) {
        const int cid = manaPaymentCounterIds[i];
        auto *counterCmd = new Command_IncCounter;
        counterCmd->set_counter_id(cid);
        counterCmd->set_delta(1);
        cmdList.append(counterCmd);
    }
    manaPaymentCounterIds.clear();

    for (int i = midCastLandTapStack.size() - 1; i >= 0; --i) {
        const LandTapUndoEntry &entry = midCastLandTapStack[i];
        CardItem *card = player->getTableZone()->getCards().findCard(entry.cardId);
        if (card) {
            card->setTapped(false, true);
            auto *attrCmd = new Command_SetCardAttr;
            attrCmd->set_zone(ZoneNames::TABLE);
            attrCmd->set_card_id(entry.cardId);
            attrCmd->set_attribute(AttrTapped);
            attrCmd->set_attr_value("0");
            cmdList.append(attrCmd);
        }
        if (entry.counterId >= 0) {
            if (auto *counter = player->getCounters().value(entry.counterId, nullptr)) {
                counter->setValue(counter->getValue() - 1);
            }
            auto *counterCmd = new Command_IncCounter;
            counterCmd->set_counter_id(entry.counterId);
            counterCmd->set_delta(-1);
            cmdList.append(counterCmd);
            if (player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
                if (Command_RuledPayload *poolCmd = buildManaPoolDecrementPayload(entry.counterName)) {
                    cmdList.append(poolCmd);
                }
            }
        } else if (player->getGame()->getGameMetaInfo()->proto().ruled_game() &&
                   !entry.counterName.isEmpty()) {
            if (Command_RuledPayload *poolCmd = buildManaPoolDecrementPayload(entry.counterName)) {
                cmdList.append(poolCmd);
            }
        }
    }
    midCastLandTapStack.clear();

    if (!cmdList.isEmpty()) {
        sendGameCommand(prepareGameCommand(cmdList));
    }

    emit ruledActivatedAbilityTargetPendingChanged(false, {});
    emit ruledAbilityActivationPendingChanged(false);
    pendingActivatedAbility = {};
    emit landTapUndoAvailableChanged(!landTapUndoStack.isEmpty());
    player->getGame()->getGameEventHandler()->emitLocalRuledLog(
        tr("Canceled activating %1.").arg(cardName.isEmpty() ? abilityText : cardName));
}

QString PlayerActions::pendingRuledAbilityPromptText() const
{
    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForMana) {
        return {};
    }
    int total = 0;
    for (auto it = pendingActivatedAbility.remainingCost.constBegin();
         it != pendingActivatedAbility.remainingCost.constEnd(); ++it) {
        total += it.value();
    }
    if (total == 0) {
        return {};
    }
    return tr("Pay mana for %1: %2 remaining (tap your lands).")
        .arg(pendingActivatedAbility.cardName,
             formatSimpleManaCost(pendingActivatedAbility.remainingCost));
}

Command_RuledPayload *PlayerActions::newRuledPayloadAddManaToPoolForLandName(const QString &manaCounterName)
{
    if (!player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        return nullptr;
    }
    return buildManaPoolIncrementPayload(manaCounterName);
}

bool PlayerActions::tryPayRuledSpellWithCounter(const QString &counterName)
{
    if (!pendingRuledSpellCast.valid) {
        return false;
    }
    // Cast flow picks targets before mana (see tryStartRuledSpellCast). Paying mana here while
    // still waiting for a target would complete the cast with no targets and burn pool counters.
    if (pendingRuledSpellCast.waitingForTarget) {
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("Choose a target for %1 before paying mana.").arg(pendingRuledSpellCast.cardName));
        return false;
    }
    const QString rawLower = counterName.trimmed().toLower();
    const bool colorlessOnly = (rawLower == QLatin1String("x") || rawLower == QLatin1String("c"));
    QChar sym;
    if (!colorlessOnly) {
        const QString n = counterName.trimmed().toUpper();
        if (n.size() != 1 || !QStringLiteral("WUBRGC").contains(n.at(0))) {
            return false;
        }
        sym = n.at(0);
    } else {
        sym = QChar();
    }

    int counterId = -1;
    for (auto it = player->getCounters().constBegin(); it != player->getCounters().constEnd(); ++it) {
        if (it.value() && it.value()->getName().trimmed().compare(counterName.trimmed(), Qt::CaseInsensitive) == 0) {
            counterId = it.key();
            break;
        }
    }
    if (counterId < 0) {
        return false;
    }

    if (!tryReducePendingSpellRemainingCostOnePip(colorlessOnly, sym)) {
        return false;
    }

    manaPaymentCounterIds.append(counterId);
    Command_IncCounter cmd;
    cmd.set_counter_id(counterId);
    cmd.set_delta(-1);
    sendGameCommand(cmd);
    finishPendingSpellManaPaymentStep();
    return true;
}

bool PlayerActions::tryPlayRuledLand(CardItem *card)
{
    if (!card || !player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    if (!card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
        return false;
    }

    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }

    const int ruledHandIndex = player->getGame()->getGameEventHandler()->resolveRuledLandPlayHandIndexForClickedCard(
        card);
    if (ruledHandIndex < 0) {
        return false;
    }

    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_play_land()->set_hand_card_index(ruledHandIndex);
    std::string payload;
    if (!ruledCommand.SerializeToString(&payload)) {
        return false;
    }

    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    sendGameCommand(cmd);
    clearLandTapUndoStack();
    return true;
}

bool PlayerActions::tryRuledOpeningBottomCard(CardItem *card)
{
    if (!card || !player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::HAND || card->getZone()->getPlayer() != player) {
        return false;
    }
    GameEventHandler *handler = player->getGame()->getGameEventHandler();
    if (!handler || handler->getRuledOpeningUiKind() != GameEventHandler::RuledOpeningUiKind::BottomLibrary) {
        return false;
    }
    const int ruledHandIndex = handler->resolveRuledOpeningBottomHandIndexForClickedCard(card);
    if (ruledHandIndex < 0 || !handler->isRuledOpeningBottomLegalForHandIndex(ruledHandIndex)) {
        return false;
    }
    handler->toggleRuledOpeningBottomHandIndex(ruledHandIndex);
    return true;
}

bool PlayerActions::tryToggleRuledCleanupDiscard(CardItem *card)
{
    if (!card || !player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::HAND || card->getZone()->getPlayer() != player) {
        return false;
    }
    GameEventHandler *handler = player->getGame()->getGameEventHandler();
    if (!handler || !handler->localPlayerMustCleanupDiscard()) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }
    const int ruledHandIndex = handler->resolveRuledCleanupDiscardHandIndexForClickedCard(card);
    if (ruledHandIndex < 0 || !handler->isRuledCleanupDiscardLegalForHandIndex(ruledHandIndex)) {
        return false;
    }
    handler->toggleRuledCleanupDiscardHandIndex(ruledHandIndex);
    return true;
}

bool PlayerActions::sendRuledCleanupDiscardBatchIfComplete()
{
    if (!player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }
    GameEventHandler *h = player->getGame()->getGameEventHandler();
    if (!h || !h->localPlayerMustCleanupDiscard()) {
        return false;
    }
    const int need = h->ruledCleanupDiscardRequiredCount();
    if (need <= 0 || h->ruledCleanupDiscardSelectedCount() != need) {
        return false;
    }
    const QList<int> idx = h->ruledCleanupDiscardSelectedIndicesSorted();
    h->clearRuledCleanupDiscardSelection(false);
    h->notifyRuledHandUiChanged();

    ruled::v1::RuledCommand ruledCommand;
    auto *d = ruledCommand.mutable_discard_to_hand_size();
    if (need == 1) {
        d->set_hand_card_index(static_cast<quint32>(idx.first()));
    } else {
        for (int i : idx) {
            d->add_hand_card_indices(static_cast<quint32>(i));
        }
    }
    std::string payload;
    if (!ruledCommand.SerializeToString(&payload)) {
        return false;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    sendGameCommand(cmd);
    return true;
}

bool PlayerActions::tryStartRuledSpellCast(CardItem *card)
{
    const int handIndex = card && card->getZone() ? card->getZone()->getCards().indexOf(card) : -1;
    if (!card || !player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    if (card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
        return false;
    }

    if (handIndex < 0) {
        return false;
    }
    GameEventHandler *const geh = player->getGame()->getGameEventHandler();
    const int ruledHandIndex = geh->resolveRuledSpellCastHandIndexForClickedCard(card);
    if (ruledHandIndex < 0) {
        return false;
    }
    if (!player->getGame()->getGameEventHandler()->isRuledSpellCastLegalForHandIndex(ruledHandIndex)) {
        return false;
    }
    if (pendingRuledSpellCast.valid && pendingRuledSpellCast.waitingForTarget &&
        pendingRuledSpellCast.handIndex == ruledHandIndex) {
        cancelPendingRuledSpellCast();
        return true;
    }

    // Timing legality (sorcery vs. instant speed, flash, combat-declaration locks, priority) is
    // decided by the engine and surfaced via isRuledSpellCastLegalForHandIndex above — the single
    // source of truth. We deliberately do NOT re-gate by card type here: doing so would block
    // flash creatures (CR 702.8b) and any future card that grants instant speed to a non-instant
    // spell. If the engine offered this hand index as castable, the click is allowed.

    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();
    clearPendingRuledSpellCast();
    pendingRuledSpellCast.valid = true;
    pendingRuledSpellCast.handIndex = ruledHandIndex;
    pendingRuledSpellCast.cardName = card->getName();
    pendingRuledSpellCast.remainingCost = parseSimpleManaCost(card->getCardInfo().getManaCost());
    pendingRuledSpellCast.selectedTargetOids.clear();
    pendingRuledSpellCast.xValue = 0;

    // CR 107.3 / 601.2b: if the cost has an {X} pip, the value of X is chosen first — before
    // targets and before paying costs. parseSimpleManaCost folds each {X} into the generic
    // bucket as a single pip, so once X is chosen we top that bucket up to xPips * X generic.
    const QString rawCost = card->getCardInfo().getManaCost();
    const int xPips = rawCost.count(QStringLiteral("{X}"));
    if (xPips > 0) {
        bool ok = false;
        const int chosenX = QInputDialog::getInt(
            nullptr, tr("Choose X"), tr("Value of X for %1:").arg(card->getName()), 0, 0, 99, 1, &ok);
        if (!ok) {
            clearPendingRuledSpellCast();
            return true; // user cancelled the cast at the X prompt
        }
        pendingRuledSpellCast.xValue = chosenX;
        // Each {X} pip already contributed 1 to the generic bucket; convert that to chosenX.
        pendingRuledSpellCast.remainingCost[QChar('X')] += xPips * (chosenX - 1);
        if (pendingRuledSpellCast.remainingCost.value(QChar('X'), 0) <= 0) {
            pendingRuledSpellCast.remainingCost.remove(QChar('X'));
        }
    }

    // CR 107.4d–f: resolve each hybrid/mono-hybrid/Phyrexian pip into concrete mana or life
    // before targeting. The choices fold into remainingCost (and lifePipIndices for Phyrexian
    // life) so the existing mana-payment flow handles them unchanged.
    if (!resolveFlexiblePipsForPendingSpell(rawCost, card->getName())) {
        clearPendingRuledSpellCast();
        return true; // user cancelled at a hybrid-pip prompt
    }

    pendingRuledSpellCast.waitingForTarget = geh->isRuledSpellCastNeedsTargetForHandIndex(ruledHandIndex);
    emit landTapUndoAvailableChanged(false);
    emit ruledSpellCastPendingChanged(true);

    if (pendingRuledSpellCast.waitingForTarget) {
        emit ruledSpellTargetingChanged(true, pendingRuledSpellCast.cardName);
        if (card->getName().trimmed().compare(QStringLiteral("Lightning Bolt"), Qt::CaseInsensitive) == 0) {
            player->getGame()->getGameEventHandler()->emitLocalRuledLog(
                tr("Cast %1 selected. Click a player's portrait or a creature, or press Cancel.").arg(card->getName()));
        } else {
            player->getGame()->getGameEventHandler()->emitLocalRuledLog(
                tr("Cast %1 selected. Select a target card, or press Cancel.").arg(card->getName()));
        }
        return true;
    }

    int totalRequired = 0;
    for (auto it = pendingRuledSpellCast.remainingCost.constBegin(); it != pendingRuledSpellCast.remainingCost.constEnd();
         ++it) {
        totalRequired += it.value();
    }
    if (totalRequired == 0) {
        return completePendingRuledSpellCast();
    }

    player->getGame()->getGameEventHandler()->emitLocalRuledLog(
        tr("Cast %1 selected. Pay mana by clicking counters: %2.")
            .arg(card->getName(), formatSimpleManaCost(pendingRuledSpellCast.remainingCost)));
    return true;
}

bool PlayerActions::tryHandleRuledSpellTargetClick(CardItem *card)
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (!card || !card->getZone()) {
        return true;
    }
    if (!player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        clearPendingRuledSpellCast();
        return false;
    }

    const QString zoneName = card->getZone()->getName();
    const bool isOnBattlefield = (zoneName == ZoneNames::TABLE);
    const bool isOnStack = (zoneName == ZoneNames::STACK);
    if (!isOnBattlefield && !isOnStack) {
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("Select a target on the battlefield (or stack), or press Cancel."));
        return true;
    }

    auto *handler = player->getGame()->getGameEventHandler();
    const int ownerPlayerId = card && card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 targetOid = handler ? handler->engineOidForCardId(ownerPlayerId, card->getId()) : 0;
    if (targetOid == 0) {
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("That target is not selectable yet. Select another target or cancel %1.")
                .arg(pendingRuledSpellCast.cardName));
        return true;
    }
    const int slot = pendingRuledSpellCast.handIndex;
    const bool valid = isOnBattlefield ? handler->isValidSpellTarget(slot, targetOid)
                                       : handler->isValidSpellStackTarget(slot, targetOid);
    if (!valid) {
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("That is not a legal target for %1.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    pendingRuledSpellCast.selectedTargetOids = {targetOid};
    pendingRuledSpellCast.waitingForTarget = false;
    emit ruledSpellTargetingChanged(false, {});

    int totalRequired = 0;
    for (auto it = pendingRuledSpellCast.remainingCost.constBegin(); it != pendingRuledSpellCast.remainingCost.constEnd();
         ++it) {
        totalRequired += it.value();
    }
    if (totalRequired == 0) {
        return completePendingRuledSpellCast();
    }

    player->getGame()->getGameEventHandler()->emitLocalRuledLog(
        tr("Target selected for %1. Pay mana by clicking counters: %2.")
            .arg(pendingRuledSpellCast.cardName, formatSimpleManaCost(pendingRuledSpellCast.remainingCost)));
    return true;
}

namespace
{
} // namespace

bool PlayerActions::isAwaitingRuledPlayerTargetSelection() const
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    auto *handler = player->getGame()->getGameEventHandler();
    if (!handler) {
        return false;
    }
    const int slot = pendingRuledSpellCast.handIndex;
    return handler->canSpellTargetSelf(slot) || handler->canSpellTargetOpponent(slot);
}

bool PlayerActions::isAwaitingRuledAbilityOrTriggerPlayerTarget() const
{
    if (pendingActivatedAbility.valid && pendingActivatedAbility.waitingForTarget) {
        return true;
    }
    auto *handler = player->getGame()->getGameEventHandler();
    return handler && handler->hasPendingTriggerTarget();
}

bool PlayerActions::tryHandleRuledSpellTargetPlayerClick(Player *targetPlayer)
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (!targetPlayer || !player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        clearPendingRuledSpellCast();
        return false;
    }

    if (!isAwaitingRuledPlayerTargetSelection()) {
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("%1 does not target players.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    const int targetPlayerId = targetPlayer->getPlayerInfo()->getId();
    if (targetPlayerId < 0) {
        return true;
    }

    auto *handler = player->getGame()->getGameEventHandler();
    const int slot = pendingRuledSpellCast.handIndex;
    const bool isSelf = (targetPlayerId == player->getPlayerInfo()->getId());
    if (isSelf && !handler->canSpellTargetSelf(slot)) {
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("%1 must target an opponent.").arg(pendingRuledSpellCast.cardName));
        return true;
    }
    if (!isSelf && !handler->canSpellTargetOpponent(slot)) {
        player->getGame()->getGameEventHandler()->emitLocalRuledLog(
            tr("%1 cannot target opponents.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    const quint32 targetOid = static_cast<quint32>(targetPlayerId);
    pendingRuledSpellCast.selectedTargetOids = {targetOid};
    pendingRuledSpellCast.waitingForTarget = false;
    emit ruledSpellTargetingChanged(false, {});

    int totalRequired = 0;
    for (auto it = pendingRuledSpellCast.remainingCost.constBegin(); it != pendingRuledSpellCast.remainingCost.constEnd();
         ++it) {
        totalRequired += it.value();
    }
    if (totalRequired == 0) {
        return completePendingRuledSpellCast();
    }

    player->getGame()->getGameEventHandler()->emitLocalRuledLog(
        tr("Target selected for %1. Pay mana by clicking counters: %2.")
            .arg(pendingRuledSpellCast.cardName, formatSimpleManaCost(pendingRuledSpellCast.remainingCost)));
    return true;
}

void PlayerActions::playCard(CardItem *card, bool faceDown)
{
    if (card == nullptr) {
        return;
    }

    Command_MoveCard cmd;
    cmd.set_start_player_id(card->getZone()->getPlayer()->getPlayerInfo()->getId());
    cmd.set_start_zone(card->getZone()->getName().toStdString());
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    CardToMove *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(card->getId());

    ExactCard exactCard = card->getCard();
    if (!exactCard) {
        return;
    }
    const CardInfo &info = exactCard.getInfo();

    if (!faceDown && tryPlayRuledLand(card)) {
        return;
    }
    if (!faceDown && tryStartRuledSpellCast(card)) {
        return;
    }

    int tableRow = info.getUiAttributes().tableRow;
    bool playToStack = SettingsCache::instance().getPlayToStack();
    QString currentZone = card->getZone()->getName();
    if (!faceDown && currentZone == ZoneNames::STACK && tableRow == 3) {
        cmd.set_target_zone(ZoneNames::GRAVE);
        cmd.set_x(0);
        cmd.set_y(0);
    } else if (!faceDown && ((!playToStack && tableRow == 3) ||
                             ((playToStack && tableRow != 0) && currentZone != ZoneNames::STACK))) {
        cmd.set_target_zone(ZoneNames::STACK);
        cmd.set_x(-1);
        cmd.set_y(0);
    } else {
        tableRow = faceDown ? 2 : info.getUiAttributes().tableRow;
        QPoint gridPoint = QPoint(-1, TableZone::tableRowToGridY(tableRow));
        cardToMove->set_face_down(faceDown);
        if (!faceDown) {
            cardToMove->set_pt(info.getPowTough().toStdString());
        }
        cardToMove->set_tapped(!faceDown && info.getUiAttributes().cipt);
        if (tableRow != 3)
            cmd.set_target_zone(ZoneNames::TABLE);
        cmd.set_x(gridPoint.x());
        cmd.set_y(gridPoint.y());
    }
    sendGameCommand(cmd);
}

/**
 * Like {@link PlayerActions::playCard}, but forces the card to be played to the table zone.
 * Cards with tablerow 3 (the stack) will be played to tablerow 1 (the noncreatures row).
 */
void PlayerActions::playCardToTable(const CardItem *card, bool faceDown)
{
    if (card == nullptr) {
        return;
    }

    Command_MoveCard cmd;
    cmd.set_start_player_id(card->getZone()->getPlayer()->getPlayerInfo()->getId());
    cmd.set_start_zone(card->getZone()->getName().toStdString());
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    CardToMove *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(card->getId());

    ExactCard exactCard = card->getCard();
    if (!exactCard) {
        return;
    }

    const CardInfo &info = exactCard.getInfo();

    int tableRow = faceDown ? 2 : info.getUiAttributes().tableRow;
    QPoint gridPoint = QPoint(-1, TableZone::tableRowToGridY(tableRow));
    cardToMove->set_face_down(faceDown);
    if (!faceDown) {
        cardToMove->set_pt(info.getPowTough().toStdString());
    }
    cardToMove->set_tapped(!faceDown && info.getUiAttributes().cipt);
    cmd.set_target_zone(ZoneNames::TABLE);
    cmd.set_x(gridPoint.x());
    cmd.set_y(gridPoint.y());
    sendGameCommand(cmd);
}

void PlayerActions::actViewLibrary()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::DECK, -1);
}

void PlayerActions::actViewHand()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::HAND, -1);
}

/**
 * @brief The sortHand actions only pass along a single SortOption in its data.
 * This method fills out the rest of the sort priority list given that option.
 * @param option The single sort option
 * @return The sort priority list
 */
static QList<CardList::SortOption> expandSortOption(CardList::SortOption option)
{
    switch (option) {
        case CardList::SortByName:
            return {};
        case CardList::SortByMainType:
            return {CardList::SortByMainType, CardList::SortByManaValue};
        case CardList::SortByManaValue:
            return {CardList::SortByManaValue, CardList::SortByColors};
        default:
            return {option};
    }
}

void PlayerActions::actSortHand()
{
    auto *action = qobject_cast<QAction *>(sender());
    CardList::SortOption option = static_cast<CardList::SortOption>(action->data().toInt());

    QList<CardList::SortOption> sortOptions = expandSortOption(option);

    static QList defaultOptions = {CardList::SortByName, CardList::SortByPrinting};

    player->getGraphicsItem()->getHandZoneGraphicsItem()->sortHand(sortOptions + defaultOptions);
}

void PlayerActions::actViewTopCards()
{
    int deckSize = player->getDeckZone()->getCards().size();
    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("View top cards of library"),
                                      tr("Number of cards: (max. %1)").arg(deckSize), defaultNumberTopCards, 1,
                                      deckSize, 1, &ok);
    if (ok) {
        defaultNumberTopCards = number;
        player->getGameScene()->toggleZoneView(player, ZoneNames::DECK, number);
    }
}

void PlayerActions::actViewBottomCards()
{
    int deckSize = player->getDeckZone()->getCards().size();
    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("View bottom cards of library"),
                                      tr("Number of cards: (max. %1)").arg(deckSize), defaultNumberBottomCards, 1,
                                      deckSize, 1, &ok);
    if (ok) {
        defaultNumberBottomCards = number;
        player->getGameScene()->toggleZoneView(player, ZoneNames::DECK, number, true);
    }
}

void PlayerActions::actAlwaysRevealTopCard()
{
    Command_ChangeZoneProperties cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_always_reveal_top_card(player->getPlayerMenu()->getLibraryMenu()->isAlwaysRevealTopCardChecked());

    sendGameCommand(cmd);
}

void PlayerActions::actAlwaysLookAtTopCard()
{
    Command_ChangeZoneProperties cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_always_look_at_top_card(player->getPlayerMenu()->getLibraryMenu()->isAlwaysLookAtTopCardChecked());

    sendGameCommand(cmd);
}

void PlayerActions::actOpenDeckInDeckEditor()
{
    emit player->openDeckEditor({.deckList = player->getDeck()});
}

void PlayerActions::actViewGraveyard()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::GRAVE, -1);
}

void PlayerActions::actViewRfg()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::EXILE, -1);
}

void PlayerActions::actViewSideboard()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::SIDEBOARD, -1);
}

void PlayerActions::actShuffle()
{
    sendGameCommand(Command_Shuffle());
}

void PlayerActions::actShuffleTop()
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Shuffle top cards of library"),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberTopCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }

    defaultNumberTopCards = number;

    Command_Shuffle cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_start(0);
    cmd.set_end(number - 1); // inclusive, the indexed card at end will be shuffled

    sendGameCommand(cmd);
}

void PlayerActions::actShuffleBottom()
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Shuffle bottom cards of library"),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberBottomCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }

    defaultNumberBottomCards = number;

    Command_Shuffle cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_start(-number);
    cmd.set_end(-1);

    sendGameCommand(cmd);
}

void PlayerActions::actDrawCard()
{
    Command_DrawCards cmd;
    cmd.set_number(1);
    sendGameCommand(cmd);
}

void PlayerActions::actMulligan()
{
    int startSize = SettingsCache::instance().getStartingHandSize();
    int handSize = player->getHandZone()->getCards().size();
    int deckSize = player->getDeckZone()->getCards().size() + handSize;

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Draw hand"),
                                      tr("Number of cards: (max. %1)").arg(deckSize) + '\n' +
                                          tr("0 and lower are in comparison to current hand size"),
                                      startSize, -handSize, deckSize, 1, &ok);

    if (!ok) {
        return;
    }

    if (number < 1) {
        number = handSize + number;
    }

    doMulligan(number);
    SettingsCache::instance().setStartingHandSize(number);
}

void PlayerActions::actMulliganSameSize()
{
    int handSize = player->getHandZone()->getCards().size();
    doMulligan(handSize);
}

void PlayerActions::actMulliganMinusOne()
{
    int handSize = player->getHandZone()->getCards().size();
    int targetSize = qMax(1, handSize - 1);
    doMulligan(targetSize);
}

void PlayerActions::doMulligan(int number)
{
    if (number < 1) {
        return;
    }

    Command_Mulligan cmd;
    cmd.set_number(number);
    sendGameCommand(cmd);
}

void PlayerActions::actDrawCards()
{
    int deckSize = player->getDeckZone()->getCards().size();
    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Draw cards"),
                                      tr("Number of cards: (max. %1)").arg(deckSize), defaultNumberTopCards, 1,
                                      deckSize, 1, &ok);
    if (ok) {
        defaultNumberTopCards = number;
        Command_DrawCards cmd;
        cmd.set_number(static_cast<google::protobuf::uint32>(number));
        sendGameCommand(cmd);
    }
}

void PlayerActions::actUndoDraw()
{
    sendGameCommand(Command_UndoDraw());
}

void PlayerActions::cmdSetTopCard(Command_MoveCard &cmd)
{
    cmd.set_start_zone(ZoneNames::DECK);
    auto *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(0);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
}

void PlayerActions::cmdSetBottomCard(Command_MoveCard &cmd)
{
    CardZoneLogic *zone = player->getDeckZone();
    int lastCard = zone->getCards().size() - 1;
    cmd.set_start_zone(ZoneNames::DECK);
    auto *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(lastCard);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
}

void PlayerActions::actMoveTopCardToGrave()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::GRAVE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardToExile()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::EXILE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardsToGrave()
{
    moveTopCardsTo(ZoneNames::GRAVE, tr("grave"), false);
}

void PlayerActions::actMoveTopCardsToGraveFaceDown()
{
    moveTopCardsTo(ZoneNames::GRAVE, tr("grave"), true);
}

void PlayerActions::actMoveTopCardsToExile()
{
    moveTopCardsTo(ZoneNames::EXILE, tr("exile"), false);
}

void PlayerActions::actMoveTopCardsToExileFaceDown()
{
    moveTopCardsTo(ZoneNames::EXILE, tr("exile"), true);
}

void PlayerActions::moveTopCardsTo(const QString &targetZone, const QString &zoneDisplayName, bool faceDown)
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Move top cards to %1").arg(zoneDisplayName),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberTopCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }
    defaultNumberTopCards = number;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(targetZone.toStdString());
    cmd.set_x(0);
    cmd.set_y(0);

    for (int i = number - 1; i >= 0; --i) {
        auto card = cmd.mutable_cards_to_move()->add_card();
        card->set_card_id(i);
        if (faceDown) {
            card->set_face_down(true);
        }
    }

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardsUntil()
{
    stopMoveTopCardsUntil();

    DlgMoveTopCardsUntil dlg(player->getGame()->getTab(), movingCardsUntilOptions);
    if (!dlg.exec()) {
        return;
    }

    auto expr = dlg.getExpr();
    movingCardsUntilOptions = dlg.getOptions();

    if (player->getDeckZone()->getCards().empty()) {
        stopMoveTopCardsUntil();
    } else {
        movingCardsUntilFilter = FilterString(expr);
        movingCardsUntilCounter = movingCardsUntilOptions.numberOfHits;
        movingCardsUntil = true;
        actMoveTopCardToPlay();
    }
}

void PlayerActions::moveOneCardUntil(CardItem *card)
{
    moveTopCardTimer->stop();

    const bool isMatch = card && movingCardsUntilFilter.check(card->getCard().getCardPtr());

    if (isMatch && movingCardsUntilOptions.autoPlay) {
        // Directly calling playCard will deadlock, since we are already in the middle of processing an event.
        // Use QTimer::singleShot to queue up the playCard on the event loop.
        QTimer::singleShot(0, this, [card, this] { playCard(card, false); });
    }

    if (player->getDeckZone()->getCards().empty() || !card) {
        stopMoveTopCardsUntil();
    } else if (isMatch) {
        --movingCardsUntilCounter;
        if (movingCardsUntilCounter > 0) {
            moveTopCardTimer->start();
        } else {
            stopMoveTopCardsUntil();
        }
    } else {
        moveTopCardTimer->start();
    }
}

/**
 * @brief Immediately stops any ongoing `play top card to stack until...` process, resetting all variables involved.
 */
void PlayerActions::stopMoveTopCardsUntil()
{
    moveTopCardTimer->stop();
    movingCardsUntilCounter = 0;
    movingCardsUntil = false;
}

void PlayerActions::actMoveTopCardToBottom()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::DECK);
    cmd.set_x(-1); // bottom of deck
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardToPlay()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::STACK);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardToPlayFaceDown()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    CardToMove *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(0);
    cardToMove->set_face_down(true);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(ZoneNames::TABLE);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToGrave()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::GRAVE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToExile()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::EXILE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardsToGrave()
{
    moveBottomCardsTo(ZoneNames::GRAVE, tr("grave"), false);
}

void PlayerActions::actMoveBottomCardsToGraveFaceDown()
{
    moveBottomCardsTo(ZoneNames::GRAVE, tr("grave"), true);
}

void PlayerActions::actMoveBottomCardsToExile()
{
    moveBottomCardsTo(ZoneNames::EXILE, tr("exile"), false);
}

void PlayerActions::actMoveBottomCardsToExileFaceDown()
{
    moveBottomCardsTo(ZoneNames::EXILE, tr("exile"), true);
}

void PlayerActions::moveBottomCardsTo(const QString &targetZone, const QString &zoneDisplayName, bool faceDown)
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Move bottom cards to %1").arg(zoneDisplayName),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberBottomCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }
    defaultNumberBottomCards = number;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(targetZone.toStdString());
    cmd.set_x(0);
    cmd.set_y(0);

    for (int i = maxCards - number; i < maxCards; ++i) {
        auto card = cmd.mutable_cards_to_move()->add_card();
        card->set_card_id(i);
        if (faceDown) {
            card->set_face_down(true);
        }
    }

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToTop()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::DECK);
    cmd.set_x(0); // top of deck
    cmd.set_y(0);

    sendGameCommand(cmd);
}

/**
 * Selects all cards in the given zone.
 *
 * @param zone The zone to select from
 * @param filter A predicate to filter which cards are selected. Defaults to always returning true.
 */
static void selectCardsInZone(
    const CardZoneLogic *zone,
    std::function<bool(const CardItem *)> filter = [](const CardItem *) { return true; })
{
    if (!zone) {
        return;
    }

    for (auto &cardItem : zone->getCards()) {
        if (cardItem && filter(cardItem)) {
            cardItem->setSelected(true);
        }
    }
}

void PlayerActions::actSelectAll()
{
    const CardItem *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    selectCardsInZone(card->getZone());
}

void PlayerActions::actSelectRow()
{
    const CardItem *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    auto isSameRow = [card](const CardItem *cardItem) {
        return qAbs(card->scenePos().y() - cardItem->scenePos().y()) < 50;
    };
    selectCardsInZone(card->getZone(), isSameRow);
}

void PlayerActions::actSelectColumn()
{
    const CardItem *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    auto isSameColumn = [card](const CardItem *cardItem) { return cardItem->x() == card->x(); };
    selectCardsInZone(card->getZone(), isSameColumn);
}

void PlayerActions::actDrawBottomCard()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::HAND);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actDrawBottomCards()
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Draw bottom cards"),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberBottomCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    } else if (number > maxCards) {
        number = maxCards;
    }
    defaultNumberBottomCards = number;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(ZoneNames::HAND);
    cmd.set_x(0);
    cmd.set_y(0);

    for (int i = maxCards - number; i < maxCards; ++i) {
        cmd.mutable_cards_to_move()->add_card()->set_card_id(i);
    }

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToPlay()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::STACK);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToPlayFaceDown()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    CardZoneLogic *zone = player->getDeckZone();
    int lastCard = zone->getCards().size() - 1;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    auto *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(lastCard);
    cardToMove->set_face_down(true);

    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(ZoneNames::TABLE);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actUntapAll()
{
    Command_SetCardAttr cmd;
    cmd.set_zone(ZoneNames::TABLE);
    cmd.set_attribute(AttrTapped);
    cmd.set_attr_value("0");

    sendGameCommand(cmd);
}

void PlayerActions::actRollDie()
{
    DlgRollDice dlg(player->getGame()->getTab());
    if (!dlg.exec()) {
        return;
    }

    Command_RollDie cmd;
    cmd.set_sides(dlg.getDieSideCount());
    cmd.set_count(dlg.getDiceToRollCount());
    sendGameCommand(cmd);
}

void PlayerActions::actCreateToken()
{
    DlgCreateToken dlg(player->getPlayerMenu()->getUtilityMenu()->getPredefinedTokens(), player->getGame()->getTab());
    if (!dlg.exec()) {
        return;
    }

    lastTokenInfo = dlg.getTokenInfo();

    ExactCard correctedCard = CardDatabaseManager::query()->guessCard({lastTokenInfo.name, lastTokenInfo.providerId});
    if (correctedCard) {
        lastTokenInfo.name = correctedCard.getName();
        lastTokenTableRow = TableZone::tableRowToGridY(correctedCard.getInfo().getUiAttributes().tableRow);
        if (lastTokenInfo.pt.isEmpty()) {
            lastTokenInfo.pt = correctedCard.getInfo().getPowTough();
        }
    }

    player->getPlayerMenu()->getUtilityMenu()->setAndEnableCreateAnotherTokenAction(
        tr("C&reate another %1 token").arg(lastTokenInfo.name));
    actCreateAnotherToken();
}

void PlayerActions::actCreateAnotherToken()
{
    if (lastTokenInfo.name.isEmpty()) {
        return;
    }

    Command_CreateToken cmd;
    cmd.set_zone(ZoneNames::TABLE);
    cmd.set_card_name(lastTokenInfo.name.toStdString());
    cmd.set_card_provider_id(lastTokenInfo.providerId.toStdString());
    cmd.set_color(lastTokenInfo.color.toStdString());
    cmd.set_pt(lastTokenInfo.pt.toStdString());
    cmd.set_annotation(lastTokenInfo.annotation.toStdString());
    cmd.set_destroy_on_zone_change(lastTokenInfo.destroy);
    cmd.set_face_down(lastTokenInfo.faceDown);
    cmd.set_x(-1);
    cmd.set_y(lastTokenTableRow);

    sendGameCommand(cmd);
}

void PlayerActions::setLastToken(CardInfoPtr cardInfo)
{
    if (cardInfo == nullptr) {
        return;
    }

    UtilityMenu *utilityMenu = player->getPlayerMenu()->getUtilityMenu();
    if (utilityMenu == nullptr || !utilityMenu->createAnotherTokenActionExists()) {
        return;
    }

    lastTokenInfo = {.name = cardInfo->getName(),
                     .color = cardInfo->getColors().isEmpty() ? QString() : cardInfo->getColors().left(1).toLower(),
                     .pt = cardInfo->getPowTough(),
                     .annotation = SettingsCache::instance().getAnnotateTokens() ? cardInfo->getText() : "",
                     .destroy = true,
                     .providerId =
                         SettingsCache::instance().cardOverrides().getCardPreferenceOverride(cardInfo->getName())};

    lastTokenTableRow = TableZone::tableRowToGridY(cardInfo->getUiAttributes().tableRow);

    utilityMenu->setAndEnableCreateAnotherTokenAction(tr("C&reate another %1 token").arg(lastTokenInfo.name));
}

void PlayerActions::actCreatePredefinedToken()
{
    auto *action = static_cast<QAction *>(sender());
    CardInfoPtr cardInfo = CardDatabaseManager::query()->getCardInfo(action->text());
    if (!cardInfo) {
        return;
    }

    setLastToken(cardInfo);

    actCreateAnotherToken();
}

void PlayerActions::actCreateRelatedCard()
{
    const CardItem *sourceCard = player->getGame()->getActiveCard();
    if (!sourceCard) {
        return;
    }
    auto *action = static_cast<QAction *>(sender());
    // If there is a better way of passing a CardRelation through a QAction, please add it here.
    auto relatedCards = sourceCard->getCardInfo().getAllRelatedCards();
    CardRelation *cardRelation = relatedCards.at(action->data().toInt());

    /*
     * If we make a token via "Token: TokenName"
     * then let's allow it to be created via "create another token"
     */
    if (createRelatedFromRelation(sourceCard, cardRelation) && cardRelation->getCanCreateAnother()) {
        ExactCard relatedCard = CardDatabaseManager::query()->getCardFromSameSet(cardRelation->getName(),
                                                                                 sourceCard->getCard().getPrinting());
        setLastToken(relatedCard.getCardPtr());
    }
}

void PlayerActions::actCreateAllRelatedCards()
{
    const CardItem *sourceCard = player->getGame()->getActiveCard();
    if (!sourceCard) {
        return;
    }

    auto relatedCards = sourceCard->getCardInfo().getAllRelatedCards();
    if (relatedCards.isEmpty()) {
        return;
    }

    CardRelation *cardRelation = nullptr;
    int tokensTypesCreated = 0;

    if (relatedCards.length() == 1) {
        cardRelation = relatedCards.at(0);
        if (createRelatedFromRelation(sourceCard, cardRelation)) {
            ++tokensTypesCreated;
        }
    } else {
        QList<CardRelation *> nonExcludedRelatedCards;
        QString dbName;
        for (CardRelation *cardRelationTemp : relatedCards) {
            if (!cardRelationTemp->getIsCreateAllExclusion() && !cardRelationTemp->getDoesAttach()) {
                nonExcludedRelatedCards.append(cardRelationTemp);
            }
        }
        switch (nonExcludedRelatedCards.length()) {
            case 1: // if nonExcludedRelatedCards == 1
                cardRelation = nonExcludedRelatedCards.at(0);
                if (createRelatedFromRelation(sourceCard, cardRelation)) {
                    ++tokensTypesCreated;
                }
                break;
            // If all are marked "Exclude", then treat the situation as if none of them are.
            // We won't accept "garbage in, garbage out", here.
            case 0: // else if nonExcludedRelatedCards == 0
                for (CardRelation *cardRelationAll : relatedCards) {
                    if (!cardRelationAll->getDoesAttach() && !cardRelationAll->getIsVariable()) {
                        dbName = cardRelationAll->getName();
                        bool persistent = cardRelationAll->getIsPersistent();
                        for (int i = 0; i < cardRelationAll->getDefaultCount(); ++i) {
                            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
                        }
                        ++tokensTypesCreated;
                        if (tokensTypesCreated == 1) {
                            cardRelation = cardRelationAll;
                        }
                    }
                }
                break;
            default: // else
                for (CardRelation *cardRelationNotExcluded : nonExcludedRelatedCards) {
                    if (!cardRelationNotExcluded->getDoesAttach() && !cardRelationNotExcluded->getIsVariable()) {
                        dbName = cardRelationNotExcluded->getName();
                        bool persistent = cardRelationNotExcluded->getIsPersistent();
                        for (int i = 0; i < cardRelationNotExcluded->getDefaultCount(); ++i) {
                            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
                        }
                        ++tokensTypesCreated;
                        if (tokensTypesCreated == 1) {
                            cardRelation = cardRelationNotExcluded;
                        }
                    }
                }
                break;
        }
    }

    /*
     * If we made at least one token via "Create All Tokens"
     * then assign the first to the "Create another" shortcut.
     */
    if (cardRelation != nullptr && cardRelation->getCanCreateAnother()) {
        CardInfoPtr cardInfo = CardDatabaseManager::query()->getCardInfo(cardRelation->getName());
        setLastToken(cardInfo);
    }
}

bool PlayerActions::createRelatedFromRelation(const CardItem *sourceCard, const CardRelation *cardRelation)
{
    if (sourceCard == nullptr || cardRelation == nullptr) {
        return false;
    }
    QString dbName = cardRelation->getName();
    bool persistent = cardRelation->getIsPersistent();
    if (cardRelation->getIsVariable()) {
        bool ok;
        player->setDialogSemaphore(true);
        int count = QInputDialog::getInt(player->getGame()->getTab(), tr("Create tokens"), tr("Number:"),
                                         cardRelation->getDefaultCount(), 1, MAX_TOKENS_PER_DIALOG, 1, &ok);
        player->setDialogSemaphore(false);
        if (!ok) {
            return false;
        }
        for (int i = 0; i < count; ++i) {
            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
        }
    } else if (cardRelation->getDefaultCount() > 1) {
        for (int i = 0; i < cardRelation->getDefaultCount(); ++i) {
            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
        }
    } else {
        auto attachType = cardRelation->getAttachType();

        // move card onto table first if attaching from some other zone
        // we only do this for AttachTo because cross-zone TransformInto is already handled server-side
        if (attachType == CardRelationType::AttachTo && sourceCard->getZone()->getName() != ZoneNames::TABLE) {
            playCardToTable(sourceCard, false);
        }

        createCard(sourceCard, dbName, attachType, persistent);
    }
    return true;
}

void PlayerActions::createCard(const CardItem *sourceCard,
                               const QString &dbCardName,
                               CardRelationType attachType,
                               bool persistent)
{
    CardInfoPtr cardInfo = CardDatabaseManager::query()->getCardInfo(dbCardName);

    if (cardInfo == nullptr || sourceCard == nullptr) {
        return;
    }

    QPoint gridPoint = QPoint(-1, TableZone::tableRowToGridY(cardInfo->getUiAttributes().tableRow));

    // create the token for the related card
    Command_CreateToken cmd;
    cmd.set_zone(ZoneNames::TABLE);
    cmd.set_card_name(cardInfo->getName().toStdString());
    switch (cardInfo->getColors().size()) {
        case 0:
            cmd.set_color("");
            break;
        case 1:
            cmd.set_color("m");
            break;
        default:
            cmd.set_color(cardInfo->getColors().left(1).toLower().toStdString());
            break;
    }

    cmd.set_pt(cardInfo->getPowTough().toStdString());
    if (SettingsCache::instance().getAnnotateTokens()) {
        cmd.set_annotation(cardInfo->getText().toStdString());
    } else {
        cmd.set_annotation("");
    }
    cmd.set_destroy_on_zone_change(!persistent);
    cmd.set_x(gridPoint.x());
    cmd.set_y(gridPoint.y());

    ExactCard relatedCard =
        CardDatabaseManager::query()->getCardFromSameSet(cardInfo->getName(), sourceCard->getCard().getPrinting());

    switch (attachType) {
        case CardRelationType::DoesNotAttach:
            cmd.set_target_zone(ZoneNames::TABLE);
            cmd.set_card_provider_id(relatedCard.getPrinting().getUuid().toStdString());
            break;

        case CardRelationType::AttachTo:
            cmd.set_target_zone(ZoneNames::TABLE); // We currently only support creating tokens on the table
            cmd.set_card_provider_id(relatedCard.getPrinting().getUuid().toStdString());
            cmd.set_target_card_id(sourceCard->getId());
            cmd.set_target_mode(Command_CreateToken::ATTACH_TO);
            break;

        case CardRelationType::TransformInto:
            // allow cards to directly transform on stack
            cmd.set_zone(sourceCard->getZone()->getName() == ZoneNames::STACK ? ZoneNames::STACK : ZoneNames::TABLE);
            // Transform card zone changes are handled server-side
            cmd.set_target_zone(sourceCard->getZone()->getName().toStdString());
            cmd.set_target_card_id(sourceCard->getId());
            cmd.set_target_mode(Command_CreateToken::TRANSFORM_INTO);
            cmd.set_card_provider_id(sourceCard->getProviderId().toStdString());
            break;
    }

    sendGameCommand(cmd);
}

void PlayerActions::actSayMessage()
{
    auto *a = qobject_cast<QAction *>(sender());
    Command_GameSay cmd;
    cmd.set_message(a->text().toStdString());
    sendGameCommand(cmd);
}

void PlayerActions::setCardAttrHelper(const GameEventContext &context,
                                      CardItem *card,
                                      CardAttribute attribute,
                                      const QString &avalue,
                                      bool allCards,
                                      EventProcessingOptions options)
{
    if (card == nullptr) {
        return;
    }

    bool moveCardContext = context.HasExtension(Context_MoveCard::ext);
    switch (attribute) {
        case AttrTapped: {
            bool tapped = avalue == "1";
            const bool isLand =
                CardDatabaseManager::query()->cardRefIsLandForBulkUntap(card->getCardRef(), card->getFaceDown());
            const bool shouldPreventUntap = !tapped && card->getDoesntUntap() && allCards && !isLand;
            if (!shouldPreventUntap) {
                if (!allCards) {
                    emit logSetTapped(player, card, tapped);
                }
                bool canAnimate = !options.testFlag(SKIP_TAP_ANIMATION) && !moveCardContext;
                card->setTapped(tapped, canAnimate);
            }
            break;
        }
        case AttrAttacking: {
            card->setAttacking(avalue == "1");
            break;
        }
        case AttrFaceDown: {
            card->setFaceDown(avalue == "1");
            break;
        }
        case AttrColor: {
            card->setColor(avalue);
            break;
        }
        case AttrAnnotation: {
            emit logSetAnnotation(player, card, avalue);
            card->setAnnotation(avalue);
            break;
        }
        case AttrDoesntUntap: {
            bool value = (avalue == "1");
            emit logSetDoesntUntap(player, card, value);
            card->setDoesntUntap(value);
            break;
        }
        case AttrPT: {
            emit logSetPT(player, card, avalue);
            card->setPT(avalue);
            break;
        }
    }
}

void PlayerActions::actMoveCardXCardsFromTop()
{
    int deckSize = player->getDeckZone()->getCards().size() + 1; // add the card to move to the deck
    bool ok;
    int number =
        QInputDialog::getInt(player->getGame()->getTab(), tr("Place card X cards from top of library"),
                             tr("Which position should this card be placed:") + "\n" + tr("(max. %1)").arg(deckSize),
                             defaultNumberTopCardsToPlaceBelow, 1, deckSize, 1, &ok);
    number -= 1; // indexes start at 0

    if (!ok) {
        return;
    }

    defaultNumberTopCardsToPlaceBelow = number;

    QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
    if (sel.isEmpty()) {
        return;
    }

    QList<CardItem *> cardList;
    while (!sel.isEmpty()) {
        cardList.append(qgraphicsitem_cast<CardItem *>(sel.takeFirst()));
    }

    QList<const ::google::protobuf::Message *> commandList;
    ListOfCardsToMove idList;
    for (const auto &i : cardList) {
        idList.add_card()->set_card_id(i->getId());
    }

    int startPlayerId = cardList[0]->getZone()->getPlayer()->getPlayerInfo()->getId();
    QString startZone = cardList[0]->getZone()->getName();

    auto *cmd = new Command_MoveCard;
    cmd->set_start_player_id(startPlayerId);
    cmd->set_start_zone(startZone.toStdString());
    cmd->mutable_cards_to_move()->CopyFrom(idList);
    cmd->set_target_player_id(player->getPlayerInfo()->getId());
    cmd->set_target_zone(ZoneNames::DECK);
    cmd->set_x(number);
    cmd->set_y(0);
    commandList.append(cmd);

    if (player->getPlayerInfo()->local) {
        sendGameCommand(prepareGameCommand(commandList));
    } else {
        player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList));
    }
}

void PlayerActions::actIncPT(int deltaP, int deltaT)
{
    int playerid = player->getPlayerInfo()->getId();

    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        QString pt = card->getPT();
        const auto ptList = parsePT(pt);
        QString newpt;
        if (ptList.isEmpty()) {
            newpt = QString::number(deltaP) + (deltaT ? "/" + QString::number(deltaT) : "");
        } else if (ptList.size() == 1) {
            newpt = QString::number(ptList.at(0).toInt() + deltaP) + (deltaT ? "/" + QString::number(deltaT) : "");
        } else {
            newpt =
                QString::number(ptList.at(0).toInt() + deltaP) + "/" + QString::number(ptList.at(1).toInt() + deltaT);
        }

        auto *cmd = new Command_SetCardAttr;
        cmd->set_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrPT);
        cmd->set_attr_value(newpt.toStdString());
        commandList.append(cmd);

        if (player->getPlayerInfo()->getLocal()) {
            playerid = card->getZone()->getPlayer()->getPlayerInfo()->getId();
        }
    }

    player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList), playerid);
}

void PlayerActions::actResetPT()
{
    int playerid = player->getPlayerInfo()->getId();
    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        QString ptString;
        if (!card->getFaceDown()) { // leave the pt empty if the card is face down
            ExactCard ec = card->getCard();
            if (ec) {
                ptString = ec.getInfo().getPowTough();
            }
        }
        if (ptString == card->getPT()) {
            continue;
        }
        QString zoneName = card->getZone()->getName();
        auto *cmd = new Command_SetCardAttr;
        cmd->set_zone(zoneName.toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrPT);
        cmd->set_attr_value(ptString.toStdString());
        commandList.append(cmd);

        if (player->getPlayerInfo()->getLocal()) {
            playerid = card->getZone()->getPlayer()->getPlayerInfo()->getId();
        }
    }

    if (!commandList.empty()) {
        player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList), playerid);
    }
}

QVariantList PlayerActions::parsePT(const QString &pt)
{
    QVariantList ptList = QVariantList();
    if (!pt.isEmpty()) {
        int sep = pt.indexOf('/');
        if (sep == 0) {
            ptList.append(QVariant(pt.mid(1))); // cut off starting '/' and take full string
        } else {
            int start = 0;
            for (;;) {
                QString item = pt.mid(start, sep - start);
                if (item.isEmpty()) {
                    ptList.append(QVariant(QString()));
                } else if (item[0] == '+') {
                    ptList.append(QVariant(item.mid(1).toInt())); // add as int
                } else if (item[0] == '-') {
                    ptList.append(QVariant(item.toInt())); // add as int
                } else {
                    ptList.append(QVariant(item)); // add as qstring
                }
                if (sep == -1) {
                    break;
                }
                start = sep + 1;
                sep = pt.indexOf('/', start);
            }
        }
    }
    return ptList;
}

void PlayerActions::actSetPT()
{
    QString oldPT;
    int playerid = player->getPlayerInfo()->getId();

    auto sel = player->getGameScene()->selectedItems();
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        if (!card->getPT().isEmpty()) {
            oldPT = card->getPT();
        }
    }
    bool ok;
    player->setDialogSemaphore(true);
    QString pt = getTextWithMax(player->getGame()->getTab(), tr("Change power/toughness"), tr("Change stats to:"),
                                QLineEdit::Normal, oldPT, &ok);
    player->setDialogSemaphore(false);
    if (player->clearCardsToDelete() || !ok) {
        return;
    }

    const auto ptList = parsePT(pt);
    bool empty = ptList.isEmpty();

    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        auto *cmd = new Command_SetCardAttr;
        QString newpt = QString();
        if (!empty) {
            const auto oldpt = parsePT(card->getPT());
            int ptIter = 0;
            for (const auto &_item : ptList) {
#if (QT_VERSION >= QT_VERSION_CHECK(6, 0, 0))
                if (_item.typeId() == QMetaType::Type::Int) {
#else
                if (_item.type() == QVariant::Int) {
#endif
                    int oldItem = ptIter < oldpt.size() ? oldpt.at(ptIter).toInt() : 0;
                    newpt += '/' + QString::number(oldItem + _item.toInt());
                } else {
                    newpt += '/' + _item.toString();
                }
                ++ptIter;
            }
            newpt = newpt.mid(1);
        }

        cmd->set_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrPT);
        cmd->set_attr_value(newpt.toStdString());
        commandList.append(cmd);

        if (player->getPlayerInfo()->local) {
            playerid = card->getZone()->getPlayer()->getPlayerInfo()->getId();
        }
    }

    player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList), playerid);
}

void PlayerActions::actDrawArrow()
{
    auto *card = player->getGame()->getActiveCard();
    if (card) {
        card->drawArrow(Qt::red);
    }
}

void PlayerActions::actIncP()
{
    actIncPT(1, 0);
}

void PlayerActions::actDecP()
{
    actIncPT(-1, 0);
}

void PlayerActions::actIncT()
{
    actIncPT(0, 1);
}

void PlayerActions::actDecT()
{
    actIncPT(0, -1);
}

void PlayerActions::actIncPT()
{
    actIncPT(1, 1);
}

void PlayerActions::actDecPT()
{
    actIncPT(-1, -1);
}

void PlayerActions::actFlowP()
{
    actIncPT(1, -1);
}

void PlayerActions::actFlowT()
{
    actIncPT(-1, 1);
}

void AnnotationDialog::keyPressEvent(QKeyEvent *event)
{
    if (event->key() == Qt::Key_Return && event->modifiers() & Qt::ControlModifier) {
        event->accept();
        accept();
        return;
    }
    QInputDialog::keyPressEvent(event);
}

void PlayerActions::actSetAnnotation()
{
    QString oldAnnotation;
    auto sel = player->getGameScene()->selectedItems();
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        if (!card->getAnnotation().isEmpty()) {
            oldAnnotation = card->getAnnotation();
        }
    }

    player->setDialogSemaphore(true);
    AnnotationDialog *dialog = new AnnotationDialog(player->getGame()->getTab());
    dialog->setOptions(QInputDialog::UsePlainTextEditForTextInput);
    dialog->setWindowTitle(tr("Set annotation"));
    dialog->setLabelText(tr("Please enter the new annotation:"));
    dialog->setTextValue(oldAnnotation);
    bool ok = dialog->exec();
    player->setDialogSemaphore(false);
    if (player->clearCardsToDelete() || !ok) {
        return;
    }
    QString annotation = dialog->textValue().left(MAX_NAME_LENGTH);

    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        auto *cmd = new Command_SetCardAttr;
        cmd->set_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrAnnotation);
        cmd->set_attr_value(annotation.toStdString());
        commandList.append(cmd);
    }
    sendGameCommand(prepareGameCommand(commandList));
}

void PlayerActions::actAttach()
{
    auto *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    card->drawAttachArrow();
}

void PlayerActions::actUnattach()
{
    QList<const ::google::protobuf::Message *> commandList;
    for (QGraphicsItem *item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);

        if (!card->getAttachedTo()) {
            continue;
        }

        auto *cmd = new Command_AttachCard;
        cmd->set_start_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        commandList.append(cmd);
    }
    sendGameCommand(prepareGameCommand(commandList));
}

void PlayerActions::actCardCounterTrigger()
{
    auto *action = static_cast<QAction *>(sender());
    int counterId = action->data().toInt() / 1000;
    QList<const ::google::protobuf::Message *> commandList;
    switch (action->data().toInt() % 1000) {
        case 9: { // increment counter
            for (const auto &item : player->getGameScene()->selectedItems()) {
                auto *card = static_cast<CardItem *>(item);
                if (card->getCounters().value(counterId, 0) < MAX_COUNTERS_ON_CARD) {
                    auto *cmd = new Command_SetCardCounter;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_counter_id(counterId);
                    cmd->set_counter_value(card->getCounters().value(counterId, 0) + 1);
                    commandList.append(cmd);
                }
            }
            break;
        }
        case 10: { // decrement counter
            for (const auto &item : player->getGameScene()->selectedItems()) {
                auto *card = static_cast<CardItem *>(item);
                if (card->getCounters().value(counterId, 0)) {
                    auto *cmd = new Command_SetCardCounter;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_counter_id(counterId);
                    cmd->set_counter_value(card->getCounters().value(counterId, 0) - 1);
                    commandList.append(cmd);
                }
            }
            break;
        }
        case 11: { // set counter with dialog
            player->setDialogSemaphore(true);

            // If a single card is selected, we show the old value in the dialog. Otherwise, we show "x"
            QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
            QString oldValueForDlg = "x";
            if (sel.size() == 1) {
                auto *card = dynamic_cast<CardItem *>(sel.first());
                oldValueForDlg = QString::number(card->getCounters().value(counterId, 0));
            }

            auto &cardCounterSettings = SettingsCache::instance().cardCounters();
            QString counterName = cardCounterSettings.displayName(counterId);

            AbstractCounterDialog dialog(counterName, oldValueForDlg, player->getGame()->getTab());
            int ok = dialog.exec();

            player->setDialogSemaphore(false);
            if (player->clearCardsToDelete() || !ok) {
                return;
            }

            for (const auto &item : sel) {
                auto *card = dynamic_cast<CardItem *>(item);

                int oldValue = card->getCounters().value(counterId, 0);
                Expression exp(oldValue);
                int number = static_cast<int>(exp.parse(dialog.textValue()));

                auto *cmd = new Command_SetCardCounter;
                cmd->set_zone(card->getZone()->getName().toStdString());
                cmd->set_card_id(card->getId());
                cmd->set_counter_id(counterId);
                cmd->set_counter_value(number);
                commandList.append(cmd);
            }
            break;
        }
        default:;
    }
    sendGameCommand(prepareGameCommand(commandList));
}

/**
 * @brief returns true if the zone is a unwritable reveal zone view (eg a card reveal window). Will return false if zone
 * is nullptr.
 */
static bool isUnwritableRevealZone(CardZoneLogic *zone)
{
    if (auto *view = qobject_cast<ZoneViewZoneLogic *>(zone)) {
        return view->getRevealZone() && !view->getWriteableRevealZone();
    }
    return false;
}

void PlayerActions::playSelectedCards(const bool faceDown)
{
    QList<CardItem *> selectedCards;
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        selectedCards.append(card);
    }
    // CardIds will get shuffled downwards when cards leave the deck.
    // We need to iterate through the cards in reverse order so cardIds don't get changed out from under us as we play
    // out the cards one-by-one.
    std::sort(selectedCards.begin(), selectedCards.end(),
              [](const auto &card1, const auto &card2) { return card1->getId() > card2->getId(); });

    for (auto &card : selectedCards) {
        if (card && !isUnwritableRevealZone(card->getZone()) && card->getZone()->getName() != ZoneNames::TABLE) {
            playCard(card, faceDown);
        }
    }
}

void PlayerActions::actPlay()
{
    playSelectedCards(false);
}

void PlayerActions::actPlayFacedown()
{
    playSelectedCards(true);
}

void PlayerActions::actHide()
{
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        if (card && isUnwritableRevealZone(card->getZone())) {
            card->getZone()->removeCard(card);
        }
    }
}

void PlayerActions::actReveal(QAction *action)
{
    const int otherPlayerId = action->data().toInt();

    Command_RevealCards cmd;
    if (otherPlayerId != -1) {
        cmd.set_player_id(otherPlayerId);
    }

    QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
    while (!sel.isEmpty()) {
        const auto *card = qgraphicsitem_cast<CardItem *>(sel.takeFirst());
        if (!cmd.has_zone_name()) {
            cmd.set_zone_name(card->getZone()->getName().toStdString());
        }
        cmd.add_card_id(card->getId());
    }

    sendGameCommand(cmd);
}

void PlayerActions::actRevealHand(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::HAND);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealRandomHandCard(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::HAND);
    cmd.add_card_id(RANDOM_CARD_FROM_ZONE);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealLibrary(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::DECK);

    sendGameCommand(cmd);
}

void PlayerActions::actLendLibrary(int lendToPlayerId)
{
    Command_RevealCards cmd;
    if (lendToPlayerId != -1) {
        cmd.set_player_id(lendToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_grant_write_access(true);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealTopCards(int revealToPlayerId, int amount)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }

    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_top_cards(amount);
    // backward compatibility: servers before #1051 only permits to reveal the first card
    cmd.add_card_id(0);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealRandomGraveyardCard(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::GRAVE);
    cmd.add_card_id(RANDOM_CARD_FROM_ZONE);
    sendGameCommand(cmd);
}

void PlayerActions::cardMenuAction()
{
    auto *a = dynamic_cast<QAction *>(sender());
    QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
    QList<CardItem *> cardList;
    while (!sel.isEmpty()) {
        cardList.append(qgraphicsitem_cast<CardItem *>(sel.takeFirst()));
    }

    QList<const ::google::protobuf::Message *> commandList;
    if (a->data().toInt() <= (int)cmClone) {
        for (const auto &card : cardList) {
            switch (static_cast<CardMenuActionType>(a->data().toInt())) {
                // Leaving both for compatibility with server
                case cmUntap:
                    // fallthrough
                case cmTap: {
                    const bool aboutToTap = !card->getTapped();
                    if (aboutToTap && player->getGame()->getGameMetaInfo()->proto().ruled_game() &&
                        card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
                        const GameEventHandler *handler = player->getGame()->getGameEventHandler();
                        const auto combatPhase = handler->getRuledCombatPhase();
                        const bool locked =
                            (combatPhase == GameEventHandler::RuledCombatPhase::DeclareAttackers &&
                             handler->localPlayerIsRuledActive()) ||
                            (combatPhase == GameEventHandler::RuledCombatPhase::DeclareBlockers &&
                             handler->localPlayerIsRuledDefender());
                        if (locked) {
                            break;
                        }
                    }
                    auto *cmd = new Command_SetCardAttr;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_attribute(AttrTapped);
                    cmd->set_attr_value(std::to_string(1 - static_cast<int>(card->getTapped())));
                    commandList.append(cmd);
                    break;
                }
                case cmDoesntUntap: {
                    auto *cmd = new Command_SetCardAttr;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_attribute(AttrDoesntUntap);
                    cmd->set_attr_value(card->getDoesntUntap() ? "0" : "1");
                    commandList.append(cmd);
                    break;
                }
                case cmFlip: {
                    auto *cmd = new Command_FlipCard;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_face_down(!card->getFaceDown());
                    if (card->getFaceDown()) {
                        ExactCard ec = card->getCard();
                        if (ec) {
                            cmd->set_pt(ec.getInfo().getPowTough().toStdString());
                        }
                    }
                    commandList.append(cmd);
                    break;
                }
                case cmPeek: {
                    auto *cmd = new Command_RevealCards;
                    cmd->set_zone_name(card->getZone()->getName().toStdString());
                    cmd->add_card_id(card->getId());
                    cmd->set_player_id(player->getPlayerInfo()->getId());
                    commandList.append(cmd);
                    break;
                }
                case cmClone: {
                    auto *cmd = new Command_CreateToken;
                    cmd->set_zone(ZoneNames::TABLE);
                    cmd->set_card_name(card->getName().toStdString());
                    cmd->set_card_provider_id(card->getProviderId().toStdString());
                    cmd->set_color(card->getColor().toStdString());
                    cmd->set_pt(card->getPT().toStdString());
                    cmd->set_annotation(card->getAnnotation().toStdString());
                    cmd->set_destroy_on_zone_change(true);
                    cmd->set_x(-1);
                    cmd->set_y(card->getGridPoint().y());
                    commandList.append(cmd);
                    break;
                }
                default:
                    break;
            }
        }
    } else {
        CardZoneLogic *zone = cardList[0]->getZone();
        if (!zone) {
            return;
        }

        Player *startPlayer = zone->getPlayer();
        if (!startPlayer) {
            return;
        }

        int startPlayerId = startPlayer->getPlayerInfo()->getId();
        QString startZone = zone->getName();

        ListOfCardsToMove idList;
        for (const auto &i : cardList) {
            idList.add_card()->set_card_id(i->getId());
        }

        switch (static_cast<CardMenuActionType>(a->data().toInt())) {
            case cmMoveToTopLibrary: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::DECK);
                cmd->set_x(0);
                cmd->set_y(0);

                if (idList.card_size() > 1) {
                    auto *scmd = new Command_Shuffle;
                    scmd->set_zone_name(ZoneNames::DECK);
                    scmd->set_start(0);
                    scmd->set_end(idList.card_size() - 1); // inclusive, the indexed card at end will be shuffled
                    // Server process events backwards, so...
                    commandList.append(scmd);
                }

                commandList.append(cmd);
                break;
            }
            case cmMoveToBottomLibrary: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::DECK);
                cmd->set_x(-1);
                cmd->set_y(0);

                if (idList.card_size() > 1) {
                    auto *scmd = new Command_Shuffle;
                    scmd->set_zone_name(ZoneNames::DECK);
                    scmd->set_start(-idList.card_size());
                    scmd->set_end(-1);
                    // Server process events backwards, so...
                    commandList.append(scmd);
                }

                commandList.append(cmd);
                break;
            }
            case cmMoveToHand: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::HAND);
                cmd->set_x(0);
                cmd->set_y(0);
                commandList.append(cmd);
                break;
            }
            case cmMoveToGraveyard: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::GRAVE);
                cmd->set_x(0);
                cmd->set_y(0);
                commandList.append(cmd);
                break;
            }
            case cmMoveToExile: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::EXILE);
                cmd->set_x(0);
                cmd->set_y(0);
                commandList.append(cmd);
                break;
            }
            case cmMoveToTable: {
                // Each card needs its own command because table row, pt, and cipt vary per card
                for (const auto &card : cardList) {
                    auto *cmd = new Command_MoveCard;
                    cmd->set_start_player_id(startPlayerId);
                    cmd->set_start_zone(startZone.toStdString());
                    cmd->set_target_player_id(player->getPlayerInfo()->getId());
                    cmd->set_target_zone(ZoneNames::TABLE);
                    cmd->set_x(-1);

                    CardToMove *ctm = cmd->mutable_cards_to_move()->add_card();
                    ctm->set_card_id(card->getId());
                    ctm->set_face_down(false);

                    int tableRow = 0;
                    ExactCard exactCard = card->getCard();
                    if (exactCard) {
                        const CardInfo &info = exactCard.getInfo();
                        tableRow = info.getUiAttributes().tableRow;
                        ctm->set_pt(info.getPowTough().toStdString());
                        ctm->set_tapped(info.getUiAttributes().cipt);
                    }

                    cmd->set_y(TableZone::tableRowToGridY(tableRow));
                    commandList.append(cmd);
                }
                break;
            }
            default:
                break;
        }
    }

    if (player->getPlayerInfo()->getLocal()) {
        sendGameCommand(prepareGameCommand(commandList));
    } else {
        player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList));
    }
}

PendingCommand *PlayerActions::prepareGameCommand(const google::protobuf::Message &cmd)
{

    if (player->getPlayerInfo()->getJudge() && !player->getPlayerInfo()->getLocal()) {
        Command_Judge base;
        GameCommand *c = base.add_game_command();
        base.set_target_id(player->getPlayerInfo()->getId());
        c->GetReflection()->MutableMessage(c, cmd.GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(cmd);
        return player->getGame()->getGameEventHandler()->prepareGameCommand(base);
    } else {
        return player->getGame()->getGameEventHandler()->prepareGameCommand(cmd);
    }
}

PendingCommand *PlayerActions::prepareGameCommand(const QList<const ::google::protobuf::Message *> &cmdList)
{
    if (player->getPlayerInfo()->getJudge() && !player->getPlayerInfo()->getLocal()) {
        Command_Judge base;
        base.set_target_id(player->getPlayerInfo()->getId());
        for (int i = 0; i < cmdList.size(); ++i) {
            GameCommand *c = base.add_game_command();
            c->GetReflection()
                ->MutableMessage(c, cmdList[i]->GetDescriptor()->FindExtensionByName("ext"))
                ->CopyFrom(*cmdList[i]);
            delete cmdList[i];
        }
        return player->getGame()->getGameEventHandler()->prepareGameCommand(base);
    } else {
        return player->getGame()->getGameEventHandler()->prepareGameCommand(cmdList);
    }
}

bool PlayerActions::tryRuledActivateAbilityMenu(CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::TABLE) {
        return false;
    }
    if (!player->getGame()->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }
    // Only show the ability menu when the local player actually has priority.
    {
        const int localId = player->getPlayerInfo()->getId();
        const int priorityId = player->getGame()->getGameState()->getPriorityPlayer();
        if (priorityId < 0 || localId != priorityId) {
            return false;
        }
    }
    auto *handler = player->getGame()->getGameEventHandler();
    if (!handler) {
        return false;
    }

    // Suppress the menu while the player is actively declaring attackers/blockers or choosing a target.
    // After submission the step enters a priority window where abilities are legal, so only block
    // during the live declaration window (before the player hits Done).
    {
        using Phase = GameEventHandler::RuledCombatPhase;
        const auto phase = handler->getRuledCombatPhase();
        if (phase == Phase::DeclareAttackers && handler->localPlayerIsRuledActive() &&
            !handler->hasAttackersSubmittedThisStep()) {
            return false;
        }
        if (phase == Phase::DeclareBlockers && handler->localPlayerIsRuledDefender() &&
            !handler->hasBlockersSubmittedThisStep()) {
            return false;
        }
        if (handler->hasPendingTriggerTarget() || pendingActivatedAbility.waitingForTarget ||
            pendingActivatedAbility.waitingForMana || pendingRuledSpellCast.waitingForTarget) {
            return false;
        }
    }

    // Determine engine ObjectId for this card.
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0) {
        return false;
    }

    const QStringList abilityTexts = handler->activatedAbilitiesForOid(oid);
    if (abilityTexts.isEmpty()) {
        return false;
    }

    // Build and show the context menu.
    QMenu menu;
    menu.setTitle(card->getName());
    for (int i = 0; i < abilityTexts.size(); ++i) {
        menu.addAction(abilityTexts[i]);
    }
    QAction *chosen = menu.exec(QCursor::pos());
    if (!chosen) {
        return true; // menu was shown, player cancelled
    }

    const int abilityIndex = abilityTexts.indexOf(chosen->text());
    if (abilityIndex < 0) {
        return true;
    }

    // Engine-authoritative: ability slot key present in valid_targets_by_ability means it needs a target.
    const bool needsTarget = handler->abilityNeedsTarget(oid, abilityIndex);

    // Look up the mana cost from the engine-supplied cost string (e.g. "4", "R", "").
    // This comes directly from AbilityCost in the tricerules registry — no text parsing.
    const QStringList manaCostStrings = handler->activatedAbilityManaCostsForOid(oid);
    const QString manaCostStr = (abilityIndex < manaCostStrings.size())
                                    ? manaCostStrings.at(abilityIndex)
                                    : QString{};
    const QMap<QChar, int> manaCost = parseSimpleManaCost(manaCostStr);
    int totalManaCost = 0;
    for (auto it = manaCost.constBegin(); it != manaCost.constEnd(); ++it) {
        totalManaCost += it.value();
    }
    const bool needsMana = totalManaCost > 0;

    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();

    pendingActivatedAbility.valid = true;
    pendingActivatedAbility.permanentOid = oid;
    pendingActivatedAbility.abilityIndex = abilityIndex;
    pendingActivatedAbility.abilityText = chosen->text();
    pendingActivatedAbility.cardName = card->getName();
    pendingActivatedAbility.needsTarget = needsTarget;
    pendingActivatedAbility.waitingForTarget = needsTarget;
    pendingActivatedAbility.selectedTargetOid = 0;
    pendingActivatedAbility.waitingForMana = false;
    pendingActivatedAbility.remainingCost = manaCost;

    if (needsTarget) {
        // Target first, then mana payment after target is chosen.
        emit ruledActivatedAbilityTargetPendingChanged(true, chosen->text());
        handler->emitLocalRuledLog(tr("Choose a target for: %1").arg(chosen->text()));
    } else if (needsMana) {
        // No target — go straight to mana payment.
        pendingActivatedAbility.waitingForMana = true;
        emit ruledAbilityActivationPendingChanged(true);
        emit ruledAbilityManaPromptChanged();
    } else {
        // No target, no mana cost — send immediately.
        completeActivateAbility();
    }
    return true;
}

bool PlayerActions::tryHandleRuledAbilityTargetClick(CardItem *card)
{
    // Check pending trigger first (higher priority).
    auto *handler = player->getGame()->getGameEventHandler();
    if (handler && handler->hasPendingTriggerTarget()) {
        if (!card || !card->getZone()) {
            return false;
        }
        const QString zoneName = card->getZone()->getName();
        if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK) {
            return false;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (targetOid == 0) {
            return false;
        }
        ruled::v1::RuledCommand cmd;
        cmd.mutable_choose_trigger_target()->set_target_object_id(targetOid);
        std::string payload;
        if (cmd.SerializeToString(&payload)) {
            Command_RuledPayload ruledPayload;
            ruledPayload.set_payload(payload);
            sendGameCommand(ruledPayload);
        }
        return true;
    }

    // Check pending activated ability target.
    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForTarget) {
        return false;
    }
    if (!card || !card->getZone()) {
        return false;
    }
    const QString zoneName = card->getZone()->getName();
    if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK) {
        handler->emitLocalRuledLog(
            tr("Select a target on the battlefield (or stack), or press Cancel."));
        return true;
    }
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (targetOid == 0) {
        handler->emitLocalRuledLog(tr("That target is not selectable yet."));
        return true;
    }
    if (!handler->isValidAbilityTarget(pendingActivatedAbility.permanentOid,
                                       pendingActivatedAbility.abilityIndex, targetOid)) {
        handler->emitLocalRuledLog(
            tr("That is not a legal target for: %1").arg(pendingActivatedAbility.abilityText));
        return true;
    }

    pendingActivatedAbility.selectedTargetOid = targetOid;
    pendingActivatedAbility.waitingForTarget = false;
    emit ruledActivatedAbilityTargetPendingChanged(false, {});

    int totalManaCost = 0;
    for (auto it = pendingActivatedAbility.remainingCost.constBegin();
         it != pendingActivatedAbility.remainingCost.constEnd(); ++it) {
        totalManaCost += it.value();
    }
    if (totalManaCost > 0) {
        pendingActivatedAbility.waitingForMana = true;
        emit ruledAbilityActivationPendingChanged(true);
        emit ruledAbilityManaPromptChanged();
    } else {
        completeActivateAbility();
    }
    return true;
}

bool PlayerActions::tryHandleRuledAbilityTargetPlayerClick(Player *targetPlayer)
{
    auto *handler = player->getGame()->getGameEventHandler();
    if (handler && handler->hasPendingTriggerTarget()) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        ruled::v1::RuledCommand cmd;
        cmd.mutable_choose_trigger_target()->set_target_object_id(targetOid);
        std::string payload;
        if (cmd.SerializeToString(&payload)) {
            Command_RuledPayload ruledPayload;
            ruledPayload.set_payload(payload);
            sendGameCommand(ruledPayload);
        }
        return true;
    }

    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForTarget) {
        return false;
    }
    if (!targetPlayer) {
        return false;
    }
    const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
    const quint32 selfOid = static_cast<quint32>(player->getPlayerInfo()->getId());
    const bool isSelf = (targetOid == selfOid);
    const quint32 permOid = pendingActivatedAbility.permanentOid;
    const int abilityIdx = pendingActivatedAbility.abilityIndex;
    if (isSelf && !handler->canAbilityTargetSelf(permOid, abilityIdx)) {
        handler->emitLocalRuledLog(
            tr("You cannot target yourself with: %1").arg(pendingActivatedAbility.abilityText));
        return true;
    }
    if (!isSelf && !handler->canAbilityTargetOpponent(permOid, abilityIdx)) {
        handler->emitLocalRuledLog(
            tr("You cannot target that player with: %1").arg(pendingActivatedAbility.abilityText));
        return true;
    }
    pendingActivatedAbility.selectedTargetOid = targetOid;
    pendingActivatedAbility.waitingForTarget = false;
    emit ruledActivatedAbilityTargetPendingChanged(false, {});

    int totalManaCost = 0;
    for (auto it = pendingActivatedAbility.remainingCost.constBegin();
         it != pendingActivatedAbility.remainingCost.constEnd(); ++it) {
        totalManaCost += it.value();
    }
    if (totalManaCost > 0) {
        pendingActivatedAbility.waitingForMana = true;
        emit ruledAbilityActivationPendingChanged(true);
        emit ruledAbilityManaPromptChanged();
    } else {
        completeActivateAbility();
    }
    return true;
}

void PlayerActions::sendGameCommand(const google::protobuf::Message &command)
{
    if (player->getPlayerInfo()->getJudge() && !player->getPlayerInfo()->getLocal()) {
        Command_Judge base;
        GameCommand *c = base.add_game_command();
        base.set_target_id(player->getPlayerInfo()->getId());
        c->GetReflection()->MutableMessage(c, command.GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(command);
        player->getGame()->getGameEventHandler()->sendGameCommand(base, player->getPlayerInfo()->getId());
    } else {
        player->getGame()->getGameEventHandler()->sendGameCommand(command, player->getPlayerInfo()->getId());
    }
}

void PlayerActions::sendGameCommand(PendingCommand *pend)
{
    player->getGame()->getGameEventHandler()->sendGameCommand(pend, player->getPlayerInfo()->getId());
}
