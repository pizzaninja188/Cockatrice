#include "player_actions.h"

#include "../../interface/widgets/tabs/tab_game.h"
#include "../../interface/widgets/utility/get_text_with_max.h"
#include "../board/abstract_counter.h"
#include "../board/card_item.h"
#include "../client/settings/card_counter_settings.h"
#include "../dialogs/dlg_move_top_cards_until.h"
#include "../dialogs/dlg_roll_dice.h"
#include "../game/game_event_handler.h"
#include "../ruled/ruled_actions.h"
#include "../ruled/ruled_client_state.h"
#include "../zones/hand_zone.h"
#include "../zones/logic/view_zone_logic.h"
#include "../zones/table_zone.h"
#include "card_menu_action_type.h"

#include <QComboBox>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QLabel>
#include <QMenu>
#include <QVBoxLayout>
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

// milliseconds in between triggers of the move top cards until action
static constexpr int MOVE_TOP_CARD_UNTIL_INTERVAL = 100;

PlayerActions::PlayerActions(Player *_player)
    : QObject(_player), player(_player), lastTokenTableRow(0), movingCardsUntil(false),
      ruledPendingCast(std::make_unique<RuledPendingCast>()), pendingRuledSpellCast(ruledPendingCast->spell),
      pendingActivatedAbility(ruledPendingCast->ability)
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

QVector<RuledFlexPip> PlayerActions::parseFlexPips(const QString &manaCost)
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

bool PlayerActions::flexPipMatchesColor(const RuledFlexPip &pip, QChar color)
{
    const QChar c = color.toUpper();
    if (pip.colorA == c) {
        return true;
    }
    // Only true two-color hybrid pips ({G/U}) accept their second color; mono-hybrid and
    // Phyrexian pips have a single color (their other alternative is generic mana / life).
    return !pip.phyrexian && pip.generic == 0 && !pip.colorB.isNull() && pip.colorB == c;
}

bool PlayerActions::promptFlexiblePipChoices(const QString &fullCost,
                                             const QString &cardName,
                                             const QVector<RuledFlexPip> &flex,
                                             QVector<bool> &choiceIsAlternative)
{
    QDialog dialog;
    dialog.setWindowTitle(tr("Pay hybrid/Phyrexian mana for %1").arg(cardName));
    auto *layout = new QVBoxLayout(&dialog);
    layout->addWidget(new QLabel(tr("Cost: %1").arg(fullCost), &dialog));

    QVector<QComboBox *> combos;
    combos.reserve(flex.size());
    for (const RuledFlexPip &pip : flex) {
        QString pipLabel;
        QString primary; // pay the color (colorA)
        QString alternative;
        if (pip.phyrexian) {
            // CR 107.4f: the color OR 2 life.
            pipLabel = QStringLiteral("{%1/P}").arg(pip.colorA);
            primary = tr("Pay {%1}").arg(pip.colorA);
            alternative = tr("Pay 2 life");
        } else if (pip.generic > 0) {
            // CR 107.4e: mono-hybrid — the color OR N generic.
            pipLabel = QStringLiteral("{%1/%2}").arg(pip.generic).arg(pip.colorA);
            primary = tr("Pay {%1}").arg(pip.colorA);
            alternative = tr("Pay {%1} generic").arg(pip.generic);
        } else {
            // CR 107.4d: hybrid — either color.
            pipLabel = QStringLiteral("{%1/%2}").arg(pip.colorA).arg(pip.colorB);
            primary = tr("Pay {%1}").arg(pip.colorA);
            alternative = tr("Pay {%1}").arg(pip.colorB);
        }
        auto *row = new QHBoxLayout;
        row->addWidget(new QLabel(pipLabel, &dialog));
        auto *combo = new QComboBox(&dialog);
        combo->addItem(primary);     // index 0 -> primary color
        combo->addItem(alternative); // index 1 -> alternative
        row->addWidget(combo, 1);
        layout->addLayout(row);
        combos.append(combo);
    }

    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dialog);
    QObject::connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
    layout->addWidget(buttons);

    if (dialog.exec() != QDialog::Accepted) {
        return false;
    }
    choiceIsAlternative.clear();
    choiceIsAlternative.reserve(combos.size());
    for (QComboBox *combo : combos) {
        choiceIsAlternative.append(combo->currentIndex() == 1);
    }
    return true;
}

void PlayerActions::applyFlexChoicesToCost(QMap<QChar, int> &fixed,
                                           QVector<quint32> &lifePipIndices,
                                           QVector<RuledFlexPip> &flex,
                                           const QVector<bool> &choiceIsAlternative)
{
    for (int i = 0; i < flex.size(); ++i) {
        const RuledFlexPip &pip = flex[i];
        const bool alternative = (i < choiceIsAlternative.size()) && choiceIsAlternative[i];
        if (pip.phyrexian) {
            if (alternative) {
                lifePipIndices.append(pip.pipIndex); // CR 107.4f: pay 2 life
            } else {
                fixed[pip.colorA.toUpper()] += 1;
            }
        } else if (pip.generic > 0) {
            if (alternative) {
                fixed[QChar('X')] += pip.generic; // CR 107.4e: N generic
            } else {
                fixed[pip.colorA.toUpper()] += 1;
            }
        } else {
            fixed[(alternative ? pip.colorB : pip.colorA).toUpper()] += 1; // CR 107.4d
        }
    }
    flex.clear();
}

bool PlayerActions::applyManaPipToFlexibleCost(QMap<QChar, int> &fixed,
                                               QVector<RuledFlexPip> &flex,
                                               bool colorlessMana,
                                               QChar coloredMana)
{
    if (!colorlessMana) {
        const QChar sym = coloredMana.toUpper();
        // 1. A fixed colored demand of this exact color (CR 202.1).
        if (fixed.value(sym, 0) > 0) {
            fixed[sym] -= 1;
            return true;
        }
        // 2. CR 107.4d–f: pay an as-yet-untouched flexible pip's colored alternative. Preferring
        //    untouched pips means a correct-color tap claims a fresh pip rather than topping up a
        //    half-paid mono-hybrid generic — e.g. {2/R}{2/R} with one generic already down, a red
        //    completes the *other* pip and leaves the partial one alone.
        for (int i = 0; i < flex.size(); ++i) {
            if (flex[i].genericPaid == 0 && flexPipMatchesColor(flex[i], sym)) {
                flex.remove(i);
                return true;
            }
        }
    }
    // 3. Fixed generic {N}/{X}: payable by any mana.
    if (fixed.value('X', 0) > 0) {
        fixed['X'] -= 1;
        return true;
    }
    // 4. Fixed colorless {C}: only colorless mana qualifies (CR 107.4c).
    if (colorlessMana && fixed.value('C', 0) > 0) {
        fixed['C'] -= 1;
        return true;
    }
    // 5. CR 107.4e: a mono-hybrid generic alternative ({2/W}), payable by any mana. Top up a
    //    partially-paid pip first (so the mana already spent on it isn't stranded), otherwise
    //    open a fresh one.
    int partialIdx = -1;
    int freshIdx = -1;
    for (int i = 0; i < flex.size(); ++i) {
        if (flex[i].generic <= 0) {
            continue; // hybrid / Phyrexian have no generic alternative
        }
        if (flex[i].genericPaid > 0) {
            if (partialIdx < 0 || flex[i].genericPaid > flex[partialIdx].genericPaid) {
                partialIdx = i;
            }
        } else if (freshIdx < 0) {
            freshIdx = i;
        }
    }
    const int idx = (partialIdx >= 0) ? partialIdx : freshIdx;
    if (idx >= 0) {
        flex[idx].genericPaid += 1;
        if (flex[idx].genericPaid >= flex[idx].generic) {
            flex.remove(idx);
        }
        return true;
    }
    return false;
}

QString PlayerActions::formatRemainingCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex)
{
    QString out = formatSimpleManaCost(fixed);
    for (const RuledFlexPip &pip : flex) {
        if (pip.phyrexian) {
            out += QStringLiteral("{%1/P}").arg(pip.colorA);
        } else if (pip.generic > 0) {
            out += QStringLiteral("{%1/%2}").arg(pip.generic - pip.genericPaid).arg(pip.colorA);
        } else {
            out += QStringLiteral("{%1/%2}").arg(pip.colorA).arg(pip.colorB);
        }
    }
    return out;
}

int PlayerActions::totalRemainingForCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex)
{
    int total = 0;
    for (auto it = fixed.constBegin(); it != fixed.constEnd(); ++it) {
        total += it.value();
    }
    // Every unresolved flexible pip still owes at least one more mana (or, for Phyrexian, 2 life).
    total += flex.size();
    return total;
}

QString PlayerActions::pendingRuledSpellPromptText() const
{
    if (!pendingRuledSpellCast.valid || pendingRuledSpellCast.waitingForTarget ||
        pendingRuledSpellCast.inDamageAllocationMode) {
        return {};
    }
    if (totalRemainingForCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips) == 0) {
        return {};
    }
    return tr("Pay mana for %1: %2 remaining (click mana counters).")
        .arg(pendingRuledSpellCast.cardName,
             formatRemainingCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips));
}

void PlayerActions::clearPendingRuledSpellCast()
{
    const bool hadTargeting = pendingRuledSpellCast.valid && pendingRuledSpellCast.waitingForTarget;
    const bool hadAllocation = pendingRuledSpellCast.valid && pendingRuledSpellCast.inDamageAllocationMode;
    const bool hadPending = pendingRuledSpellCast.valid;
    pendingRuledSpellCast = PendingRuledSpellCast{};
    if (hadTargeting) {
        emit ruledSpellTargetingChanged(false, {});
        emit ruledMultiTargetSelectionUpdated(0, -1);
    }
    if (hadAllocation) {
        player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
    }
    if (hadPending) {
        emit ruledSpellCastPendingChanged(false);
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
    }
    // Every exit from a pending cast runs through here, so this is the one place that has to
    // retract the graveyard-view hint.
    RuledActions::updateGraveyardTargetHint(player, -1, 0);
}

bool PlayerActions::promptForRuledSpellXIfNeeded()
{
    // No X pips, or X already chosen (xPips zeroed below): nothing to do.
    if (pendingRuledSpellCast.xPips <= 0) {
        return true;
    }
    bool ok = false;
    const int chosenX = QInputDialog::getInt(
        nullptr, tr("Choose X"), tr("Value of X for %1:").arg(pendingRuledSpellCast.cardName), 0, 0, 99, 1, &ok);
    if (!ok) {
        clearPendingRuledSpellCast();
        return false; // user cancelled the cast at the X prompt
    }
    pendingRuledSpellCast.xValue = chosenX;
    // Each X pip already contributed 1 to the generic bucket; convert that to chosenX.
    pendingRuledSpellCast.remainingCost[QChar('X')] += pendingRuledSpellCast.xPips * (chosenX - 1);
    if (pendingRuledSpellCast.remainingCost.value(QChar('X'), 0) <= 0) {
        pendingRuledSpellCast.remainingCost.remove(QChar('X'));
    }
    pendingRuledSpellCast.xPips = 0; // guard against double-prompting
    return true;
}

bool PlayerActions::resolvePendingSpellFlexiblePips()
{
    if (pendingRuledSpellCast.flexPips.isEmpty()) {
        return true;
    }
    const QString fullCost = formatRemainingCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips);
    QVector<bool> choices;
    if (!promptFlexiblePipChoices(fullCost, pendingRuledSpellCast.cardName, pendingRuledSpellCast.flexPips, choices)) {
        clearPendingRuledSpellCast();
        return false; // cancelled at the flexible-pip dialog; cast aborted
    }
    applyFlexChoicesToCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.lifePipIndices,
                           pendingRuledSpellCast.flexPips, choices);
    return true;
}

bool PlayerActions::resolvePendingAbilityFlexiblePips()
{
    if (pendingActivatedAbility.flexPips.isEmpty()) {
        return true;
    }
    const QString fullCost =
        formatRemainingCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.flexPips);
    QVector<bool> choices;
    if (!promptFlexiblePipChoices(fullCost, pendingActivatedAbility.cardName, pendingActivatedAbility.flexPips,
                                  choices)) {
        cancelPendingActivatedAbility();
        return false; // cancelled at the flexible-pip dialog; activation aborted
    }
    applyFlexChoicesToCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.lifePipIndices,
                           pendingActivatedAbility.flexPips, choices);
    return true;
}

void PlayerActions::cancelPendingRuledSpellCast()
{
    if (!pendingRuledSpellCast.valid) {
        return;
    }
    const QString cardName = pendingRuledSpellCast.cardName;

    // Restore the mana counters drained pip-by-pip toward this spell. The cast was never sent, so
    // the engine never spent the mana (the pool is engine-owned; the display was only decremented
    // locally — see tryPayRuledSpellWithCounter). Any lands tapped to float mana stay tapped/floated
    // and remain undoable via the engine's UndoManaAbility (the Undo button), not unwound here.
    for (int i = manaPaymentCounterIds.size() - 1; i >= 0; --i) {
        if (auto *counter = player->getCounters().value(manaPaymentCounterIds[i], nullptr)) {
            counter->setValue(counter->getValue() + 1);
        }
    }
    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();

    clearPendingRuledSpellCast();
    emit landTapUndoAvailableChanged(landTapUndoCurrentlyAvailable());
    player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(tr("Canceled casting %1.").arg(cardName));
}

void PlayerActions::recordLandTapUndo(int cardId, const QString &counterName, int counterId)
{
    if (pendingRuledSpellCast.valid || pendingActivatedAbility.valid) {
        midCastLandTapStack.append({cardId, counterName, counterId});
        return;
    }
    const bool hadEntries = !landTapUndoStack.isEmpty();
    landTapUndoStack.append({cardId, counterName, counterId});
    if (!hadEntries) {
        emit landTapUndoAvailableChanged(true);
    }
}

bool PlayerActions::landTapUndoCurrentlyAvailable() const
{
    if (RuledActions::isRuledGame(player->getGame())) {
        return ruledUndoableManaCount > 0;
    }
    return !landTapUndoStack.isEmpty();
}

void PlayerActions::setRuledUndoableManaCount(int count)
{
    const int clamped = count < 0 ? 0 : count;
    if (clamped == ruledUndoableManaCount) {
        return;
    }
    ruledUndoableManaCount = clamped;
    emit landTapUndoAvailableChanged(landTapUndoCurrentlyAvailable());
}

void PlayerActions::undoLastLandTap()
{
    // CR 605 float courtesy: in ruled mode the engine owns tap state and the mana pool, so undo is
    // an engine command (UndoManaAbility) that untaps the source and removes the floated mana. The
    // resulting batch refreshes undoable_mana_abilities, which drives the button back off when 0.
    if (RuledActions::isRuledGame(player->getGame())) {
        if (RuledActions::gameplayInputLocked(player->getGame()) || ruledUndoableManaCount <= 0) {
            return;
        }
        ruled::v1::RuledCommand ruledCommand;
        ruledCommand.mutable_undo_mana_ability();
        std::string payload;
        if (!ruledCommand.SerializeToString(&payload)) {
            return;
        }
        Command_RuledPayload cmd;
        cmd.set_payload(payload);
        sendGameCommand(cmd);
        return;
    }

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
    }

    if (!cmdList.isEmpty()) {
        sendGameCommand(prepareGameCommand(cmdList));
    }

    emit landTapUndoAvailableChanged(landTapUndoCurrentlyAvailable());
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
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false;
    }
    if (!pendingRuledSpellCast.valid || pendingRuledSpellCast.handIndex < 0) {
        clearPendingRuledSpellCast();
        return false;
    }
    if (pendingRuledSpellCast.waitingForTarget) {
        return false;
    }

    ruled::v1::RuledCommand ruledCommand;
    auto *cast = ruledCommand.mutable_cast_spell();
    auto *source = cast->mutable_source();
    switch (pendingRuledSpellCast.source) {
        case RuledCastSource::Hand:
            source->set_hand_index(static_cast<quint32>(pendingRuledSpellCast.handIndex));
            break;
        case RuledCastSource::Graveyard:
            source->set_graveyard_object_id(static_cast<quint32>(pendingRuledSpellCast.handIndex));
            break;
        case RuledCastSource::Exile:
            source->set_exile_object_id(static_cast<quint32>(pendingRuledSpellCast.handIndex));
            break;
    }
    cast->set_x_value(static_cast<quint32>(pendingRuledSpellCast.xValue));
    // CR 709/712/715: which face of a multi-face card to cast (0 for single-face cards).
    cast->set_face_index(static_cast<quint32>(pendingRuledSpellCast.faceIndex));
    if (pendingRuledSpellCast.selectedModes.isEmpty()) {
        for (int i = 0; i < pendingRuledSpellCast.selectedTargetOids.size(); ++i) {
            auto *target = cast->add_targets();
            target->set_object_id(pendingRuledSpellCast.selectedTargetOids.at(i));
            if (i < pendingRuledSpellCast.selectedTargetDamages.size()) {
                target->set_damage_amount(pendingRuledSpellCast.selectedTargetDamages.at(i));
            }
        }
    } else {
        for (const auto &mode : pendingRuledSpellCast.selectedModes) {
            auto *selectedMode = cast->add_selected_modes();
            selectedMode->set_mode_index(static_cast<quint32>(mode.modeIndex));
            for (int i = 0; i < mode.selectedTargetOids.size(); ++i) {
                auto *target = selectedMode->add_targets();
                target->set_object_id(mode.selectedTargetOids.at(i));
                if (i < mode.selectedTargetDamages.size()) {
                    target->set_damage_amount(mode.selectedTargetDamages.at(i));
                }
            }
        }
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
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false;
    }
    if (!pendingActivatedAbility.valid || pendingActivatedAbility.waitingForTarget ||
        pendingActivatedAbility.waitingForCost || pendingActivatedAbility.waitingForMana) {
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
    // CR 107.4f: Phyrexian pips the player chose to pay with life (via self-portrait click).
    for (const quint32 pipIndex : pendingActivatedAbility.lifePipIndices) {
        auto *flex = aa->add_flex_payments();
        flex->set_pip_index(pipIndex);
        flex->set_pay_life(true);
    }
    for (const auto &selection : pendingActivatedAbility.costSelections) {
        auto *costSelection = aa->add_cost_selections();
        costSelection->set_cost_index(static_cast<quint32>(selection.costIndex));
        if (selection.zone == RuledAbilityCostChoiceZone::Hand) {
            costSelection->set_hand_index(selection.selectedId);
        } else {
            costSelection->set_permanent_id(selection.selectedId);
        }
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
    emit ruledAbilityCostPromptChanged();
    pendingActivatedAbility = {};
    return true;
}

void PlayerActions::continuePendingActivatedAbilityAfterChoice()
{
    if (!pendingActivatedAbility.valid || pendingActivatedAbility.waitingForTarget) {
        return;
    }
    if (pendingActivatedAbility.nextCostChoice < pendingActivatedAbility.costChoices.size()) {
        pendingActivatedAbility.waitingForCost = true;
        emit ruledAbilityCostPromptChanged();
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(pendingRuledAbilityCostPromptText());
        return;
    }
    pendingActivatedAbility.waitingForCost = false;
    emit ruledAbilityCostPromptChanged();
    if (!resolvePendingAbilityFlexiblePips()) {
        return;
    }
    if (totalRemainingForCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.flexPips) > 0) {
        pendingActivatedAbility.waitingForMana = true;
        emit ruledAbilityActivationPendingChanged(true);
        emit ruledAbilityManaPromptChanged();
    } else {
        completeActivateAbility();
    }
}

bool PlayerActions::tryReducePendingAbilityRemainingCostOnePip(bool colorlessMana, QChar coloredMana)
{
    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForMana) {
        return false;
    }
    return applyManaPipToFlexibleCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.flexPips,
                                      colorlessMana, coloredMana);
}

void PlayerActions::finishPendingAbilityManaPaymentStep()
{
    if (totalRemainingForCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.flexPips) == 0) {
        pendingActivatedAbility.waitingForMana = false;
        completeActivateAbility();
        return;
    }
    emit ruledAbilityManaPromptChanged();
    player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        tr("Pay mana for %1: %2 remaining (click mana counters).")
            .arg(pendingActivatedAbility.cardName,
                 formatRemainingCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.flexPips)));
}

bool PlayerActions::tryReducePendingSpellRemainingCostOnePip(bool colorlessMana, QChar coloredMana)
{
    if (!pendingRuledSpellCast.valid || pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    return applyManaPipToFlexibleCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips,
                                      colorlessMana, coloredMana);
}

void PlayerActions::finishPendingSpellManaPaymentStep()
{
    if (totalRemainingForCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips) == 0) {
        completePendingRuledSpellCast();
        return;
    }
    emit ruledSpellManaPromptChanged();
    player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        tr("Pay mana for %1: %2 remaining (click mana counters).")
            .arg(pendingRuledSpellCast.cardName,
                 formatRemainingCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips)));
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
        if (it.value() && it.value()->getName().trimmed().compare(counterName.trimmed(), Qt::CaseInsensitive) == 0) {
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
    // CR 106/605: the mana pool is engine-owned and the server rejects client IncCounter on pool
    // counters; the real deduction lands engine-side when the activation is sent (echoed back as
    // ManaPoolUpdated). Reflect the pending spend immediately by decrementing the displayed counter
    // locally so the player sees their pool drain pip-by-pip and can't over-click mana they lack;
    // cancelPendingActivatedAbility restores it if the activation is abandoned.
    if (auto *counter = player->getCounters().value(counterId, nullptr)) {
        counter->setValue(counter->getValue() - 1);
    }
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

    // Restore the mana counters drained pip-by-pip toward this activation. The activation was never
    // sent, so the engine never spent the mana (the pool is engine-owned; the display was only
    // decremented locally — see tryPayRuledAbilityWithCounter). Any lands tapped to float mana stay
    // floated and remain undoable via the engine's UndoManaAbility (the Undo button).
    for (int i = manaPaymentCounterIds.size() - 1; i >= 0; --i) {
        if (auto *counter = player->getCounters().value(manaPaymentCounterIds[i], nullptr)) {
            counter->setValue(counter->getValue() + 1);
        }
    }
    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();

    emit ruledActivatedAbilityTargetPendingChanged(false, {});
    emit ruledAbilityActivationPendingChanged(false);
    emit ruledAbilityCostPromptChanged();
    pendingActivatedAbility = {};
    emit landTapUndoAvailableChanged(landTapUndoCurrentlyAvailable());
    player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        tr("Canceled activating %1.").arg(cardName.isEmpty() ? abilityText : cardName));
}

QString PlayerActions::pendingRuledAbilityPromptText() const
{
    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForMana) {
        return {};
    }
    if (totalRemainingForCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.flexPips) == 0) {
        return {};
    }
    return tr("Pay mana for %1: %2 remaining (click mana counters).")
        .arg(pendingActivatedAbility.cardName,
             formatRemainingCost(pendingActivatedAbility.remainingCost, pendingActivatedAbility.flexPips));
}

Command_RuledPayload *PlayerActions::newRuledPayloadActivateManaAbilityForLand(CardItem *card, QChar desiredColor)
{
    if (!card || !RuledActions::isRuledGame(player->getGame()) ||
        RuledActions::gameplayInputLocked(player->getGame())) {
        return nullptr;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler) {
        return nullptr;
    }
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0) {
        return nullptr;
    }
    // CR 605: pick this permanent's first mana ability (non-empty produced entry) and, when the
    // ability offers multiple options (a dual land), the option that makes the wanted color.
    const QStringList produced = handler->activatedAbilityManaProducedForOid(oid);
    int abilityIndex = -1;
    int optionIndex = 0;
    for (int i = 0; i < produced.size(); ++i) {
        if (produced.at(i).isEmpty()) {
            continue;
        }
        abilityIndex = i;
        if (!desiredColor.isNull()) {
            const QStringList options = produced.at(i).split(QChar('/'));
            for (int o = 0; o < options.size(); ++o) {
                if (options.at(o).contains(desiredColor.toUpper())) {
                    optionIndex = o;
                    break;
                }
            }
        }
        break;
    }
    if (abilityIndex < 0) {
        return nullptr; // not a mana source
    }

    ruled::v1::RuledCommand rc;
    auto *aa = rc.mutable_activate_ability();
    aa->set_permanent_id(oid);
    aa->set_ability_index(static_cast<uint32_t>(abilityIndex));
    aa->set_mana_option_index(static_cast<uint32_t>(optionIndex));
    std::string payload;
    if (!rc.SerializeToString(&payload)) {
        return nullptr;
    }
    auto *cmd = new Command_RuledPayload;
    cmd->set_payload(payload);
    return cmd;
}

bool PlayerActions::tryPayRuledSpellWithCounter(const QString &counterName)
{
    if (!pendingRuledSpellCast.valid) {
        return false;
    }
    // Cast flow picks targets before mana (see tryStartRuledSpellCast). Paying mana here while
    // still waiting for a target would complete the cast with no targets and burn pool counters.
    if (pendingRuledSpellCast.waitingForTarget) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
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
    // CR 106/605: the mana pool is engine-owned and the server rejects client IncCounter on pool
    // counters; the real deduction lands engine-side when the cast is sent (echoed back as
    // ManaPoolUpdated). Reflect the pending spend immediately by decrementing the displayed counter
    // locally so the player sees their pool drain pip-by-pip and can't over-click mana they lack;
    // cancelPendingRuledSpellCast restores it if the cast is abandoned.
    if (auto *counter = player->getCounters().value(counterId, nullptr)) {
        counter->setValue(counter->getValue() - 1);
    }
    finishPendingSpellManaPaymentStep();
    return true;
}

void PlayerActions::autoApplyFloatedManaToPendingCost(const QString &counterName, int amount)
{
    if (amount <= 0) {
        return;
    }
    // CR 605/106: mana produced while a spell or ability is mid-payment is applied straight to that
    // pending cost (it goes "toward the spell", not into the pool) — the pre-engine-owned behavior the
    // player expects when they tap a land after clicking a spell. Each produced pip is routed through
    // the same pay step a pool-counter click uses (reduce the remaining cost AND decrement the displayed
    // counter), so the just-floated mana never lingers visibly in the pool and producing/spending can't
    // double-count. A spell waiting on a target is skipped (mana comes after targets). Pips the pending
    // cost cannot use (wrong color, nothing left to pay) are left floating for later use.
    for (int i = 0; i < amount; ++i) {
        if (pendingRuledSpellCast.valid && !pendingRuledSpellCast.waitingForTarget) {
            if (tryPayRuledSpellWithCounter(counterName)) {
                continue;
            }
        }
        if (pendingActivatedAbility.valid && pendingActivatedAbility.waitingForMana) {
            if (tryPayRuledAbilityWithCounter(counterName)) {
                continue;
            }
        }
        break;
    }
}

bool PlayerActions::isAwaitingRuledAbilityCostSelection() const
{
    return pendingActivatedAbility.valid && pendingActivatedAbility.waitingForCost &&
           pendingActivatedAbility.nextCostChoice < pendingActivatedAbility.costChoices.size();
}

QString PlayerActions::pendingRuledAbilityCostPromptText() const
{
    if (!isAwaitingRuledAbilityCostSelection()) {
        return {};
    }
    const auto &choice = pendingActivatedAbility.costChoices.at(pendingActivatedAbility.nextCostChoice);
    return choice.zone == RuledAbilityCostChoiceZone::Hand
               ? tr("Choose a card to discard for %1.").arg(pendingActivatedAbility.cardName)
               : tr("Choose a permanent to sacrifice for %1.").arg(pendingActivatedAbility.cardName);
}

void PlayerActions::resumePendingRuledPaymentAfterEngineCommand()
{
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return;
    }
    switch (readyRuledPendingPaymentAction(pendingRuledSpellCast, pendingActivatedAbility)) {
        case RuledPendingPaymentAction::CastSpell:
            completePendingRuledSpellCast();
            break;
        case RuledPendingPaymentAction::ActivateAbility:
            pendingActivatedAbility.waitingForMana = false;
            completeActivateAbility();
            break;
        case RuledPendingPaymentAction::None:
            break;
    }
}

bool PlayerActions::sendRuledPlayLand(int handIndex, int faceIndex)
{
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *pl = ruledCommand.mutable_play_land();
    pl->set_hand_card_index(handIndex);
    // CR 712: which face of an MDFC land enters the battlefield (0 = front; default for single-face).
    pl->set_face_index(static_cast<quint32>(faceIndex));
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

bool PlayerActions::tryPlayRuledLand(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }

    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    const int ruledHandIndex = RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_PLAY_LAND, card);
    if (ruledHandIndex < 0) {
        return false; // engine does not offer this card as a land play right now
    }

    // CR 712: an MDFC land (a pathway) shows up in the engine's legal actions as more than one
    // playable face for the same hand slot — front and back. Present a side-picker so the player
    // chooses which land to play; a single-face land plays its one face directly. The whole notion
    // of "which faces are lands and playable" comes from the engine (rules), not the Oracle DB.
    const QVector<RuledFaceOption> faces = geh->handActionFaceOptions(ruled::v1::HAND_ACTION_PLAY_LAND, ruledHandIndex);
    if (faces.size() > 1) {
        return tryRuledLandPlayFaceMenu(card);
    }
    return sendRuledPlayLand(ruledHandIndex, faces.isEmpty() ? 0 : faces.first().faceIndex);
}

bool PlayerActions::tryRuledLandPlayFaceMenu(CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false; // preserve the ordinary right-click inspection menu
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }
    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (!geh) {
        return false;
    }
    const int ruledHandIndex = RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_PLAY_LAND, card);
    if (ruledHandIndex < 0) {
        return false;
    }
    // CR 712: only offer the picker when the engine exposes more than one playable face for this
    // slot (an MDFC land). A single-face land keeps its direct click-to-play and falls through so a
    // right-click still opens the normal card menu.
    const QVector<RuledFaceOption> faces = geh->handActionFaceOptions(ruled::v1::HAND_ACTION_PLAY_LAND, ruledHandIndex);
    if (faces.size() < 2) {
        return false;
    }

    QMenu menu;
    QVector<QAction *> actionsByOption;
    actionsByOption.reserve(faces.size());
    for (const RuledFaceOption &opt : faces) {
        actionsByOption.append(menu.addAction(tr("Play %1").arg(opt.faceName)));
    }
    QAction *chosen = menu.exec(QCursor::pos());
    if (!chosen) {
        return true; // menu was shown, player cancelled
    }
    const int sel = actionsByOption.indexOf(chosen);
    if (sel < 0) {
        return true;
    }
    return sendRuledPlayLand(ruledHandIndex, faces.at(sel).faceIndex);
}

bool PlayerActions::tryRuledOpeningBottomCard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (card->getZone()->getName() != ZoneNames::HAND || card->getZone()->getPlayer() != player) {
        return false;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || handler->getOpeningUiKind() != RuledClientState::RuledOpeningUiKind::BottomLibrary) {
        return false;
    }
    const int ruledHandIndex =
        RuledActions::resolveHandActionIndex(handler, ruled::v1::HAND_ACTION_OPENING_BOTTOM, card);
    if (ruledHandIndex < 0 || !handler->isHandActionLegal(ruled::v1::HAND_ACTION_OPENING_BOTTOM, ruledHandIndex)) {
        return false;
    }
    handler->toggleOpeningBottomHandIndex(ruledHandIndex);
    return true;
}

bool PlayerActions::tryRuledResolutionHandPickCard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || !handler->isResolutionHandPickActive()) {
        return false;
    }
    // Same zone gate the highlight uses — keeping the two in one place is what stops a card that
    // merely shares an id with a candidate from being treated as one.
    if (!RuledActions::isResolutionPickZoneCard(handler, card)) {
        return false;
    }
    const int serverCardId = card->getId();
    if (!handler->isResolutionHandPickCardSelectable(serverCardId)) {
        return false;
    }
    handler->toggleResolutionHandPickCard(serverCardId);
    return true;
}

bool PlayerActions::tryRuledTriggerOrderCard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || !handler->isTriggerOrderPickCard(card->getId())) {
        return false;
    }
    // One click is one placement (CR 603.3b): no toggle, no confirm. The engine answers with this
    // trigger's target prompt or a shorter ordering prompt.
    handler->pickTriggerOrderCard(card->getId());
    return true;
}

bool PlayerActions::tryToggleRuledCleanupDiscard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (card->getZone()->getName() != ZoneNames::HAND || card->getZone()->getPlayer() != player) {
        return false;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || !handler->localPlayerMustCleanupDiscard()) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }
    const int ruledHandIndex =
        RuledActions::resolveHandActionIndex(handler, ruled::v1::HAND_ACTION_CLEANUP_DISCARD, card);
    if (ruledHandIndex < 0 || !handler->isHandActionLegal(ruled::v1::HAND_ACTION_CLEANUP_DISCARD, ruledHandIndex)) {
        return false;
    }
    handler->toggleCleanupDiscardHandIndex(ruledHandIndex);
    return true;
}

bool PlayerActions::sendRuledCleanupDiscardBatchIfComplete()
{
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false;
    }
    RuledClientState *h = player->getGame()->getGameEventHandler()->ruled();
    if (!h || !h->localPlayerMustCleanupDiscard()) {
        return false;
    }
    const int need = h->cleanupDiscardRequiredCount();
    if (need <= 0 || h->cleanupDiscardSelectedCount() != need) {
        return false;
    }
    const QList<int> idx = h->cleanupDiscardSelectedIndicesSorted();
    h->clearCleanupDiscardSelection(false);
    h->notifyHandUiChanged();

    ruled::v1::RuledCommand ruledCommand;
    auto *d = ruledCommand.mutable_discard_to_hand_size();
    for (int i : idx) {
        d->add_hand_card_indices(static_cast<quint32>(i));
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
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    const bool fromHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool fromPublicZone =
        card->getZone()->getName() == ZoneNames::GRAVE || card->getZone()->getName() == ZoneNames::EXILE;
    if (!fromHand && !fromPublicZone) {
        return false;
    }
    if (card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
        return false;
    }

    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (fromPublicZone) {
        const quint32 objectId = RuledActions::resolvePublicZoneObjectId(geh, card);
        if (objectId == 0 || !geh->isZoneActionLegal(objectId)) {
            return false;
        }
        const QVector<RuledFaceOption> options = geh->zoneActionFaceOptions(objectId);
        if (options.size() != 1) {
            return false; // A future multi-face public-zone cast gets a side-picker before use.
        }
        const auto &option = options.first();
        const QString cost = geh->zoneActionCost(objectId, option.faceIndex);
        if (cost.isEmpty()) {
            return false;
        }
        return beginRuledSpellCast(card, static_cast<int>(objectId), option.faceIndex, option.faceName, cost,
                                   geh->zoneActionSource(objectId));
    }

    const int ruledHandIndex = RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_CAST_SPELL, card);
    if (ruledHandIndex < 0) {
        return false;
    }
    const QVector<RuledFaceOption> faces =
        geh->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, ruledHandIndex);
    if (faces.size() > 1) {
        return tryRuledSpellCastFaceMenu(card);
    }
    if (faces.isEmpty()) {
        return false;
    }
    const auto &face = faces.first();
    return beginRuledSpellCast(card, ruledHandIndex, face.faceIndex, face.faceName, face.manaCost);
}

bool PlayerActions::beginRuledSpellCast(CardItem *,
                                        int ruledHandIndex,
                                        int faceIndex,
                                        const QString &castName,
                                        const QString &castCost,
                                        RuledCastSource source)
{
    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (source == RuledCastSource::Hand ? !geh->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, ruledHandIndex)
                                        : !geh->isZoneActionLegal(static_cast<quint32>(ruledHandIndex))) {
        return false;
    }
    if (pendingRuledSpellCast.valid && pendingRuledSpellCast.waitingForTarget &&
        pendingRuledSpellCast.handIndex == ruledHandIndex && pendingRuledSpellCast.faceIndex == faceIndex &&
        pendingRuledSpellCast.source == source) {
        // For multi-target spells, clicking the spell again while 1+ targets are chosen confirms
        // the selection (instead of canceling); clicking with 0 targets cancels as before.
        if (pendingRuledSpellCast.isDamageTargets && !pendingRuledSpellCast.selectedTargetOids.isEmpty()) {
            return finalizeTargetSelectionAndContinue();
        }
        cancelPendingRuledSpellCast();
        return true;
    }

    const auto actionIt = geh->handActions.constFind(ruled::v1::HAND_ACTION_CAST_SPELL);
    const int castKey = RuledClientState::spellTargetKey(ruledHandIndex, faceIndex);
    QVector<PendingRuledSpellCast::SelectedMode> selectedModes;
    const RuledHandActionSet *actionSet = source == RuledCastSource::Hand ? nullptr : &geh->zoneCastActions;
    if (source == RuledCastSource::Hand && actionIt != geh->handActions.constEnd()) {
        actionSet = &actionIt.value();
    }
    if (actionSet && actionSet->modalOptionsByCastKey.contains(castKey)) {
        const auto &modeOptions = actionSet->modalOptionsByCastKey.value(castKey);
        const auto selected = RuledPendingCast::chooseModes(player->getGame()->getTab(), castName, modeOptions,
                                                            actionSet->modalMinModesByCastKey.value(castKey),
                                                            actionSet->modalMaxModesByCastKey.value(castKey));
        if (!selected.has_value()) {
            return true;
        }
        for (const int modeIndex : *selected) {
            const auto option = std::find_if(modeOptions.cbegin(), modeOptions.cend(),
                                             [modeIndex](const auto &mode) { return mode.modeIndex == modeIndex; });
            if (option != modeOptions.cend()) {
                selectedModes.append({option->modeIndex, option->label, option->needsTarget, option->targets, {}, {}});
            }
        }
    }

    // Timing legality (sorcery vs. instant speed, flash, combat-declaration locks, priority) is
    // decided by the engine and surfaced via the CastSpell legality check above — the single
    // source of truth. We deliberately do NOT re-gate by card type here: doing so would block
    // flash creatures (CR 702.8b) and any future card that grants instant speed to a non-instant
    // spell. If the engine offered this hand index as castable, the click is allowed.

    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();
    clearPendingRuledSpellCast();
    pendingRuledSpellCast.valid = true;
    pendingRuledSpellCast.handIndex = ruledHandIndex;
    pendingRuledSpellCast.source = source;
    pendingRuledSpellCast.faceIndex = faceIndex;
    pendingRuledSpellCast.selectedTargetOids.clear();
    pendingRuledSpellCast.xValue = 0;
    pendingRuledSpellCast.cardName = castName;
    pendingRuledSpellCast.remainingCost = parseSimpleManaCost(castCost);
    pendingRuledSpellCast.selectedModes = selectedModes;

    // CR 107.3: record how many X pips the cost has; X is chosen before target selection
    // (see promptForRuledSpellXIfNeeded). parseSimpleManaCost folds each X pip
    // into the generic bucket as a single pip, so once X is chosen we top that bucket up to
    // xPips * X generic. The cost may be unbraced ("XR", Oracle single-face) or braced ("{X}{R}",
    // split faces), so count the X symbol directly — X is only ever the variable pip in a cost.
    const QString rawCost = castCost;
    pendingRuledSpellCast.xPips = rawCost.count(QLatin1Char('X'), Qt::CaseInsensitive);

    // CR 107.4d–f: keep flexible pips (hybrid {G/U}, mono-hybrid {2/W}, Phyrexian {B/P}) live
    // rather than prompting. They resolve as the player taps mana — a tapped color claims a pip
    // whose alternative it matches, off-color/colorless mana funds a mono-hybrid generic
    // alternative — and a Phyrexian pip can be paid with 2 life by clicking the player's portrait.
    pendingRuledSpellCast.flexPips = parseFlexPips(rawCost);

    pendingRuledSpellCast.activeModePosition = -1;
    if (!selectedModes.isEmpty()) {
        for (int i = 0; i < selectedModes.size(); ++i) {
            if (selectedModes.at(i).needsTarget) {
                pendingRuledSpellCast.activeModePosition = i;
                break;
            }
        }
    }
    pendingRuledSpellCast.waitingForTarget =
        pendingRuledSpellCast.activeModePosition >= 0 ||
        (selectedModes.isEmpty() &&
         (source == RuledCastSource::Hand
              ? geh->handActionNeedsTarget(ruled::v1::HAND_ACTION_CAST_SPELL, ruledHandIndex, faceIndex)
              : geh->zoneActionNeedsTarget(static_cast<quint32>(ruledHandIndex))));
    if (pendingRuledSpellCast.activeModePosition >= 0) {
        const auto &targetData = selectedModes.at(pendingRuledSpellCast.activeModePosition).targets;
        pendingRuledSpellCast.isDamageTargets = targetData.isDamageTargets;
        pendingRuledSpellCast.damageDividedEvenly = targetData.damageDividedEvenly;
        pendingRuledSpellCast.maxTargets = targetData.maxTargets;
        pendingRuledSpellCast.fixedDamage = targetData.fixedDamage;
        pendingRuledSpellCast.extraManaPerTarget = targetData.extraManaPerTarget;
    } else {
        pendingRuledSpellCast.isDamageTargets = geh->spellIsDamageTargets(ruledHandIndex, faceIndex, source);
        pendingRuledSpellCast.damageDividedEvenly =
            geh->spellTargetData(ruledHandIndex, faceIndex, source).damageDividedEvenly;
        pendingRuledSpellCast.maxTargets = geh->spellMaxTargets(ruledHandIndex, faceIndex, source);
        pendingRuledSpellCast.fixedDamage = geh->spellFixedDamage(ruledHandIndex, faceIndex, source);
        pendingRuledSpellCast.extraManaPerTarget = geh->spellExtraManaPerTarget(ruledHandIndex, faceIndex, source);
    }
    emit landTapUndoAvailableChanged(false);
    emit ruledSpellCastPendingChanged(true);

    // CR 601.2b: choose X before selecting targets and before paying mana.
    if (!promptForRuledSpellXIfNeeded()) {
        return true; // cancelled at the X prompt; cast aborted
    }

    if (pendingRuledSpellCast.waitingForTarget) {
        const QString effectText =
            pendingRuledSpellCast.activeModePosition >= 0
                ? pendingRuledSpellCast.selectedModes.at(pendingRuledSpellCast.activeModePosition).label
                : pendingRuledSpellCast.cardName;
        emit ruledSpellTargetingChanged(true, effectText);
        // Open the graveyard view(s) this spell can target, so a reanimation/regrowth target is
        // reachable without the player opening the pile by hand first.
        RuledActions::updateGraveyardTargetHint(player, pendingRuledSpellCast.handIndex,
                                                pendingRuledSpellCast.faceIndex);
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Choose a target for “%1”, or press Cancel.").arg(effectText));
        return true;
    }

    // CR 107.4d–f: front-load hybrid/Phyrexian choices before paying, so the player picks one side
    // of each flexible pip and the mana prompt then shows only the resolved (fixed) cost.
    if (!resolvePendingSpellFlexiblePips()) {
        return true; // cancelled at the flexible-pip dialog; cast aborted
    }

    if (totalRemainingForCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips) == 0) {
        return completePendingRuledSpellCast();
    }

    player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        tr("Cast %1 selected. Pay mana by clicking counters: %2.")
            .arg(pendingRuledSpellCast.cardName,
                 formatRemainingCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips)));
    return true;
}

bool PlayerActions::tryRuledSpellCastFaceMenu(CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false; // preserve the ordinary right-click inspection menu
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (!geh) {
        return false;
    }
    const int handIndex = RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_CAST_SPELL, card);
    if (handIndex < 0) {
        return false;
    }
    const QVector<RuledFaceOption> faces = geh->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, handIndex);
    if (faces.isEmpty()) {
        return false;
    }
    if (faces.size() == 1) {
        const auto actionIt = geh->handActions.constFind(ruled::v1::HAND_ACTION_CAST_SPELL);
        const int faceIndex = faces.first().faceIndex;
        const int castKey = RuledClientState::spellTargetKey(handIndex, faceIndex);
        if (actionIt == geh->handActions.constEnd() || !actionIt->modalOptionsByCastKey.contains(castKey)) {
            return false;
        }
        const auto &face = faces.first();
        return beginRuledSpellCast(card, handIndex, face.faceIndex, face.faceName, face.manaCost);
    }
    const auto chosen = RuledPendingCast::chooseFace(player->getGame()->getTab(), card->getName(), faces);
    if (!chosen.has_value()) {
        return true; // menu was shown, player cancelled
    }
    beginRuledSpellCast(card, handIndex, chosen->faceIndex, chosen->faceName, chosen->manaCost);
    return true;
}

bool PlayerActions::tryHandleRuledSpellTargetClick(CardItem *card)
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (!card || !card->getZone()) {
        return true;
    }
    if (!RuledActions::isRuledGame(player->getGame())) {
        clearPendingRuledSpellCast();
        return false;
    }

    const QString zoneName = card->getZone()->getName();
    const bool isOnBattlefield = (zoneName == ZoneNames::TABLE);
    const bool isOnStack = (zoneName == ZoneNames::STACK);
    const bool isOnGraveyard = (zoneName == ZoneNames::GRAVE);
    if (!isOnBattlefield && !isOnStack && !isOnGraveyard) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Select a target on the battlefield, stack, or a graveyard, or press Cancel."));
        return true;
    }

    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    const int slot = pendingRuledSpellCast.handIndex;
    const int face = pendingRuledSpellCast.faceIndex;

    const int ownerPlayerId = card && card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    quint32 targetOid = 0;
    if (isOnGraveyard) {
        // Graveyard cards are tracked via the GraveyardObjectMap (not the battlefield OID map),
        // and that map is keyed by owner: Server_Card ids repeat across players' zones, so a
        // spell that can read any graveyard (Reanimate) needs the owner to disambiguate.
        targetOid = handler ? handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId()) : 0;
    } else {
        targetOid = handler ? handler->engineOidForCardId(ownerPlayerId, card->getId()) : 0;
    }
    if (targetOid == 0) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("That target is not selectable yet. Select another target or cancel %1.")
                .arg(pendingRuledSpellCast.cardName));
        return true;
    }
    const bool hasModalTarget = pendingRuledSpellCast.activeModePosition >= 0;
    const auto *modalTarget =
        hasModalTarget ? &pendingRuledSpellCast.selectedModes.at(pendingRuledSpellCast.activeModePosition).targets
                       : nullptr;
    const bool valid =
        modalTarget
            ? (isOnBattlefield ? modalTarget->validPermanentIds.contains(targetOid)
               : isOnGraveyard ? modalTarget->validGraveyardIds.contains(targetOid)
                               : modalTarget->validStackIds.contains(targetOid))
            : (isOnBattlefield ? handler->isValidSpellTarget(slot, face, targetOid, pendingRuledSpellCast.source)
               : isOnGraveyard
                   ? handler->isValidSpellGraveyardTarget(slot, face, targetOid, pendingRuledSpellCast.source)
                   : handler->isValidSpellStackTarget(slot, face, targetOid, pendingRuledSpellCast.source));
    if (!valid) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("That is not a legal target for %1.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    if (pendingRuledSpellCast.selectedTargetOids.contains(targetOid)) {
        pendingRuledSpellCast.selectedTargetOids.removeOne(targetOid);
        const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target deselected. %1 target(s) chosen for %2.").arg(chosen).arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, effectiveDamageTargetsMax());
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    pendingRuledSpellCast.selectedTargetOids.append(targetOid);

    // For DamageTargets with room for more targets, stay in targeting mode.
    // CR 601.2d: each target must receive >= 1 damage, so the true cap is the total damage (or
    // the engine's max_targets, whichever is smaller). Reaching it auto-advances to damage
    // allocation — matching Fire's fixed 2-target cap, so Fireball no longer needs a re-click.
    const int effMax = effectiveDamageTargetsMax();
    const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
    // effMax == 0 means "no cap" — reachable only for evenly-divided damage, where no per-target
    // minimum bounds the count. There is nothing to auto-advance on, so the player confirms
    // explicitly (click the spell again, or the Confirm Targets button).
    if (pendingRuledSpellCast.isDamageTargets && (effMax <= 0 || chosen < effMax)) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target %1%2 chosen for %3. Click another target, or click %3 again to confirm.")
                .arg(chosen)
                .arg(effMax > 0 ? QStringLiteral("/%1").arg(effMax) : QString())
                .arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, effMax);
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    return finalizeTargetSelectionAndContinue();
}

namespace
{
} // namespace

bool PlayerActions::isTargetSelectedForPendingSpell(quint32 oid) const
{
    return pendingRuledSpellCast.valid && pendingRuledSpellCast.selectedTargetOids.contains(oid);
}

bool PlayerActions::isPlayerSelectedAsPendingSpellTarget(int playerId) const
{
    return pendingRuledSpellCast.valid &&
           pendingRuledSpellCast.selectedTargetOids.contains(static_cast<quint32>(playerId));
}

void PlayerActions::confirmMultiTargetSelection()
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget ||
        !pendingRuledSpellCast.isDamageTargets || pendingRuledSpellCast.selectedTargetOids.isEmpty()) {
        return;
    }
    finalizeTargetSelectionAndContinue();
}

bool PlayerActions::isAwaitingRuledPlayerTargetSelection() const
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler) {
        return false;
    }
    const int slot = pendingRuledSpellCast.handIndex;
    const int face = pendingRuledSpellCast.faceIndex;
    if (pendingRuledSpellCast.activeModePosition >= 0) {
        const auto &targets = pendingRuledSpellCast.selectedModes.at(pendingRuledSpellCast.activeModePosition).targets;
        return targets.canTargetSelf || targets.canTargetOpponent;
    }
    return handler->canSpellTargetSelf(slot, face, pendingRuledSpellCast.source) ||
           handler->canSpellTargetOpponent(slot, face, pendingRuledSpellCast.source);
}

bool PlayerActions::isAwaitingRuledAbilityOrTriggerPlayerTarget() const
{
    if (pendingActivatedAbility.valid && pendingActivatedAbility.waitingForTarget) {
        return true;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    return handler && (handler->hasPendingTriggerTarget() ||
                       handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
}

bool PlayerActions::tryHandleRuledSpellTargetPlayerClick(Player *targetPlayer)
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (!targetPlayer || !RuledActions::isRuledGame(player->getGame())) {
        clearPendingRuledSpellCast();
        return false;
    }

    if (!isAwaitingRuledPlayerTargetSelection()) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("%1 does not target players.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    const int targetPlayerId = targetPlayer->getPlayerInfo()->getId();
    if (targetPlayerId < 0) {
        return true;
    }

    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    const int slot = pendingRuledSpellCast.handIndex;
    const int face = pendingRuledSpellCast.faceIndex;
    const bool isSelf = (targetPlayerId == player->getPlayerInfo()->getId());
    const auto *modalTarget =
        pendingRuledSpellCast.activeModePosition >= 0
            ? &pendingRuledSpellCast.selectedModes.at(pendingRuledSpellCast.activeModePosition).targets
            : nullptr;
    const bool canTargetSelf = modalTarget ? modalTarget->canTargetSelf
                                           : handler->canSpellTargetSelf(slot, face, pendingRuledSpellCast.source);
    const bool canTargetOpponent = modalTarget
                                       ? modalTarget->canTargetOpponent
                                       : handler->canSpellTargetOpponent(slot, face, pendingRuledSpellCast.source);
    if (isSelf && !canTargetSelf) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("%1 must target an opponent.").arg(pendingRuledSpellCast.cardName));
        return true;
    }
    if (!isSelf && !canTargetOpponent) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("%1 cannot target opponents.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    const quint32 targetOid = static_cast<quint32>(targetPlayerId);
    if (pendingRuledSpellCast.selectedTargetOids.contains(targetOid)) {
        pendingRuledSpellCast.selectedTargetOids.removeOne(targetOid);
        const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target deselected. %1 target(s) chosen for %2.").arg(chosen).arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, effectiveDamageTargetsMax());
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    pendingRuledSpellCast.selectedTargetOids.append(targetOid);

    // CR 601.2d: each target must receive >= 1 damage, so the true cap is the total damage (or
    // the engine's max_targets, whichever is smaller). Reaching it auto-advances to damage
    // allocation — matching Fire's fixed 2-target cap, so Fireball no longer needs a re-click.
    const int effMax = effectiveDamageTargetsMax();
    const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
    // effMax == 0 means "no cap" — reachable only for evenly-divided damage, where no per-target
    // minimum bounds the count. There is nothing to auto-advance on, so the player confirms
    // explicitly (click the spell again, or the Confirm Targets button).
    if (pendingRuledSpellCast.isDamageTargets && (effMax <= 0 || chosen < effMax)) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target %1%2 chosen for %3. Click another target, or click %3 again to confirm.")
                .arg(chosen)
                .arg(effMax > 0 ? QStringLiteral("/%1").arg(effMax) : QString())
                .arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, effMax);
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    return finalizeTargetSelectionAndContinue();
}

int PlayerActions::pendingDamageTargetsTotal() const
{
    return pendingRuledSpellCast.fixedDamage > 0 ? pendingRuledSpellCast.fixedDamage : pendingRuledSpellCast.xValue;
}

int PlayerActions::effectiveDamageTargetsMax() const
{
    if (!pendingRuledSpellCast.isDamageTargets) {
        return pendingRuledSpellCast.maxTargets;
    }
    // "Divided evenly" has no per-target minimum — Fireball may legally target more creatures
    // than X (they simply each take 0). Only the engine's own cap applies, if any.
    if (pendingRuledSpellCast.damageDividedEvenly) {
        return pendingRuledSpellCast.maxTargets;
    }
    const int total = pendingDamageTargetsTotal();
    // CR 601.2d: at least 1 damage per target caps the count at the total damage. Fire caps at
    // min(2, total).
    if (pendingRuledSpellCast.maxTargets > 0) {
        return qMin(pendingRuledSpellCast.maxTargets, total);
    }
    return total;
}

bool PlayerActions::storeCurrentModalTargetsAndAdvance()
{
    const int current = pendingRuledSpellCast.activeModePosition;
    if (current < 0 || current >= pendingRuledSpellCast.selectedModes.size()) {
        return false;
    }
    auto &mode = pendingRuledSpellCast.selectedModes[current];
    mode.selectedTargetOids = pendingRuledSpellCast.selectedTargetOids;
    mode.selectedTargetDamages = pendingRuledSpellCast.selectedTargetDamages;

    for (int next = current + 1; next < pendingRuledSpellCast.selectedModes.size(); ++next) {
        const auto &nextMode = pendingRuledSpellCast.selectedModes.at(next);
        if (!nextMode.needsTarget) {
            continue;
        }
        pendingRuledSpellCast.activeModePosition = next;
        pendingRuledSpellCast.selectedTargetOids.clear();
        pendingRuledSpellCast.selectedTargetDamages.clear();
        pendingRuledSpellCast.isDamageTargets = nextMode.targets.isDamageTargets;
        pendingRuledSpellCast.maxTargets = nextMode.targets.maxTargets;
        pendingRuledSpellCast.fixedDamage = nextMode.targets.fixedDamage;
        pendingRuledSpellCast.extraManaPerTarget = nextMode.targets.extraManaPerTarget;
        pendingRuledSpellCast.waitingForTarget = true;
        emit ruledSpellTargetingChanged(true, nextMode.label);
        RuledActions::updateGraveyardTargetHint(player, pendingRuledSpellCast.handIndex,
                                                pendingRuledSpellCast.faceIndex);
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Choose a target for “%1”, or press Cancel.").arg(nextMode.label));
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        player->getGameScene()->update();
        return true;
    }
    pendingRuledSpellCast.activeModePosition = -1;
    return false;
}

bool PlayerActions::finalizeTargetSelectionAndContinue()
{
    pendingRuledSpellCast.waitingForTarget = false;
    emit ruledSpellTargetingChanged(false, {});
    emit ruledMultiTargetSelectionUpdated(0, -1);
    player->getGameScene()->update();

    // CR 601.2f: DamageTargets surcharge — extra generic mana per target beyond the first
    // (Fireball costs {1} more per extra target). Fold it into the generic bucket now that the
    // target count is fixed, so the mana prompt matches the engine's real cost. The engine
    // recomputes this independently in cast_spell; this only keeps the local display honest.
    if (pendingRuledSpellCast.isDamageTargets && pendingRuledSpellCast.extraManaPerTarget > 0) {
        const int extra =
            pendingRuledSpellCast.extraManaPerTarget * qMax(0, pendingRuledSpellCast.selectedTargetOids.size() - 1);
        if (extra > 0) {
            pendingRuledSpellCast.remainingCost[QChar('X')] += extra;
        }
    }

    // DamageTargets: allocate damage among chosen targets interactively.
    if (pendingRuledSpellCast.isDamageTargets) {
        const int total =
            pendingRuledSpellCast.fixedDamage > 0 ? pendingRuledSpellCast.fixedDamage : pendingRuledSpellCast.xValue;
        const int numTargets = pendingRuledSpellCast.selectedTargetOids.size();
        // "Divided evenly, rounded down" involves no choice, so there is nothing to allocate: the
        // engine divides on resolution among the targets still legal then and ignores whatever
        // damage_amount we send. Targeting more creatures than the total is legal here — they each
        // simply take 0 — so neither the min-1-per-target rejection nor the interactive allocation
        // below applies. Send explicit zeros so the wire value matches what the engine will use.
        if (pendingRuledSpellCast.damageDividedEvenly) {
            pendingRuledSpellCast.selectedTargetDamages.clear();
            for (int i = 0; i < numTargets; ++i)
                pendingRuledSpellCast.selectedTargetDamages.append(0);
        } else if (numTargets > total) {
            player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
                tr("Cannot assign at least 1 damage to each target (%1 targets, %2 total). Cast cancelled.")
                    .arg(numTargets)
                    .arg(total));
            clearPendingRuledSpellCast();
            return true;
        } else if (numTargets == 1) {
            pendingRuledSpellCast.selectedTargetDamages.clear();
            pendingRuledSpellCast.selectedTargetDamages.append(static_cast<quint32>(total));
            // Single target: skip interactive allocation, fall through to mana payment.
        } else {
            // Multiple targets: initialize each to 1 and enter interactive allocation mode.
            pendingRuledSpellCast.targetDamageAllocations.clear();
            for (int i = 0; i < numTargets; ++i)
                pendingRuledSpellCast.targetDamageAllocations.append(1);
            pendingRuledSpellCast.damageAllocationTotal = total;
            pendingRuledSpellCast.inDamageAllocationMode = true;
            player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
            player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
                tr("Assign %1 damage among %2 targets (min 1 each). "
                   "Click to add, right-click to reduce. Confirm when done.")
                    .arg(total)
                    .arg(numTargets));
            return true; // wait for the player to confirm via the prompt button
        }
    }

    if (storeCurrentModalTargetsAndAdvance()) {
        return true;
    }

    // CR 107.4d–f: front-load hybrid/Phyrexian choices.
    if (!resolvePendingSpellFlexiblePips()) {
        return true;
    }

    if (totalRemainingForCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips) == 0) {
        return completePendingRuledSpellCast();
    }

    player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        tr("Target(s) selected for %1. Pay mana by clicking counters: %2.")
            .arg(pendingRuledSpellCast.cardName,
                 formatRemainingCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips)));
    return true;
}

bool PlayerActions::isInSpellDamageAllocationMode() const
{
    return pendingRuledSpellCast.valid && pendingRuledSpellCast.inDamageAllocationMode;
}

bool PlayerActions::isSpellDamageAllocationDisplayActive() const
{
    return pendingRuledSpellCast.valid && pendingRuledSpellCast.isDamageTargets &&
           !pendingRuledSpellCast.selectedTargetOids.isEmpty();
}

int PlayerActions::spellDamageAllocationForOid(quint32 oid) const
{
    if (!isSpellDamageAllocationDisplayActive())
        return 0;
    const int idx = pendingRuledSpellCast.selectedTargetOids.indexOf(oid);
    if (idx < 0)
        return 0;
    // While interactively allocating, show the in-progress split; once confirmed (and through
    // mana payment) show the amount that will actually be sent with the cast.
    if (pendingRuledSpellCast.inDamageAllocationMode) {
        return idx < pendingRuledSpellCast.targetDamageAllocations.size()
                   ? pendingRuledSpellCast.targetDamageAllocations.at(idx)
                   : 0;
    }
    return idx < pendingRuledSpellCast.selectedTargetDamages.size()
               ? static_cast<int>(pendingRuledSpellCast.selectedTargetDamages.at(idx))
               : 0;
}

int PlayerActions::spellDamageAllocationForPlayerId(int playerId) const
{
    return spellDamageAllocationForOid(static_cast<quint32>(playerId));
}

int PlayerActions::spellDamageAllocationAssignedTotal() const
{
    int sum = 0;
    for (int v : pendingRuledSpellCast.targetDamageAllocations)
        sum += v;
    return sum;
}

int PlayerActions::spellDamageAllocationMaxTotal() const
{
    return pendingRuledSpellCast.damageAllocationTotal;
}

bool PlayerActions::spellDamageAllocationIsLegal() const
{
    return isInSpellDamageAllocationMode() &&
           spellDamageAllocationAssignedTotal() == pendingRuledSpellCast.damageAllocationTotal;
}

bool PlayerActions::tryBumpSpellDamageAllocationForOid(quint32 oid, int delta)
{
    if (!isInSpellDamageAllocationMode())
        return false;
    const int idx = pendingRuledSpellCast.selectedTargetOids.indexOf(oid);
    if (idx < 0 || idx >= pendingRuledSpellCast.targetDamageAllocations.size())
        return false;
    const int cur = pendingRuledSpellCast.targetDamageAllocations.at(idx);
    const int total = pendingRuledSpellCast.damageAllocationTotal;
    const int othersSum = spellDamageAllocationAssignedTotal() - cur;
    const int next = qBound(1, cur + delta, total - othersSum);
    if (next == cur)
        return true; // legal target but no change possible
    pendingRuledSpellCast.targetDamageAllocations[idx] = next;
    player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
    return true;
}

bool PlayerActions::tryBumpSpellDamageAllocationForCard(CardItem *card, int delta)
{
    if (!isInSpellDamageAllocationMode() || !card)
        return false;
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler)
        return false;
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0)
        return false;
    return tryBumpSpellDamageAllocationForOid(oid, delta);
}

bool PlayerActions::tryBumpSpellDamageAllocationForPlayer(Player *targetPlayer, int delta)
{
    if (!isInSpellDamageAllocationMode() || !targetPlayer)
        return false;
    return tryBumpSpellDamageAllocationForOid(static_cast<quint32>(targetPlayer->getPlayerInfo()->getId()), delta);
}

void PlayerActions::confirmSpellDamageAllocation()
{
    if (!spellDamageAllocationIsLegal())
        return;
    pendingRuledSpellCast.selectedTargetDamages.clear();
    for (int v : pendingRuledSpellCast.targetDamageAllocations)
        pendingRuledSpellCast.selectedTargetDamages.append(static_cast<quint32>(v));
    pendingRuledSpellCast.inDamageAllocationMode = false;
    player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();

    if (storeCurrentModalTargetsAndAdvance())
        return;
    if (!resolvePendingSpellFlexiblePips())
        return;
    if (totalRemainingForCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips) == 0) {
        completePendingRuledSpellCast();
        return;
    }
    player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        tr("Target(s) selected for %1. Pay mana by clicking counters: %2.")
            .arg(pendingRuledSpellCast.cardName,
                 formatRemainingCost(pendingRuledSpellCast.remainingCost, pendingRuledSpellCast.flexPips)));
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
                    if (aboutToTap && RuledActions::isRuledGame(player->getGame()) &&
                        card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
                        RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
                        const auto combatPhase = handler->getCombatPhase();
                        const bool locked = (combatPhase == RuledClientState::RuledCombatPhase::DeclareAttackers &&
                                             handler->localPlayerIsActive()) ||
                                            (combatPhase == RuledClientState::RuledCombatPhase::DeclareBlockers &&
                                             handler->localPlayerIsDefender());
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

bool PlayerActions::tryRuledActivateAbilityMenu(CardItem *card, bool leftClick)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::TABLE) {
        return false;
    }
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return leftClick; // left-click is consumed; right-click still opens inspection
    }
    // Only show the ability menu when the local player actually has priority.
    {
        const int localId = player->getPlayerInfo()->getId();
        const int priorityId = player->getGame()->getGameState()->getPriorityPlayer();
        if (priorityId < 0 || localId != priorityId) {
            return false;
        }
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler) {
        return false;
    }

    // Suppress the menu while the player is actively declaring attackers/blockers or choosing a target.
    // After submission the step enters a priority window where abilities are legal, so only block
    // during the live declaration window (before the player hits Done).
    {
        using Phase = RuledClientState::RuledCombatPhase;
        const auto phase = handler->getCombatPhase();
        if (phase == Phase::DeclareAttackers && handler->localPlayerIsActive() &&
            !handler->hasAttackersSubmittedThisStep()) {
            return false;
        }
        if (phase == Phase::DeclareBlockers && handler->localPlayerIsDefender() &&
            !handler->hasBlockersSubmittedThisStep()) {
            return false;
        }
        // Block starting a new activation while choosing a target. Paying mana (a pending spell or
        // ability waiting on mana) is intentionally NOT blocked here: tapping a mana land floats mana
        // that autoApplyFloatedManaToPendingCost routes into the pending cost. The full ability menu is
        // still suppressed during ability payment further below, to avoid clobbering it.
        if (handler->hasPendingTriggerTarget() ||
            handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource) ||
            pendingActivatedAbility.waitingForTarget ||
            pendingRuledSpellCast.waitingForTarget) {
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

    // CR 605: a permanent whose *only* activated ability is a mana ability gets the fast path —
    // no pending-ability state, just send activate_ability. Two sub-cases:
    //   • Single option (basic land): left-click auto-activates; right-click falls through to the
    //     full menu (keeps the existing "right-click = see text" behavior).
    //   • Multiple options (dual land): both left and right click show a compact color-picker menu
    //     so the player can choose which color to produce.
    const QStringList manaProduced = handler->activatedAbilityManaProducedForOid(oid);
    const QStringList costLabels = handler->activatedAbilityCostLabelsForOid(oid);
    // A tapped (or summoning-sick) mana source has nothing to offer: skip the fast path rather
    // than firing an activation the engine will reject.
    if (abilityTexts.size() == 1 && !manaProduced.value(0).isEmpty() && handler->abilityActivatable(oid, 0) &&
        handler->abilityCostChoices(oid, 0).isEmpty()) {
        const QStringList colorOptions = manaProduced.value(0).split(QChar('/'));
        if (colorOptions.size() > 1) {
            // Dual land: show a compact color-picker on both left and right click.
            const QString costPrefix = costLabels.value(0);
            QMenu colorMenu;
            colorMenu.setTitle(card->getName());
            for (const QString &opt : colorOptions) {
                const QString label =
                    costPrefix.isEmpty() ? tr("Add {%1}").arg(opt) : tr("%1: Add {%2}").arg(costPrefix, opt);
                colorMenu.addAction(label);
            }
            QAction *chosen = colorMenu.exec(QCursor::pos());
            if (!chosen) {
                return true; // player dismissed the picker
            }
            const int sel = colorMenu.actions().indexOf(chosen);
            const QChar desiredColor = (sel >= 0 && sel < colorOptions.size() && !colorOptions.at(sel).isEmpty())
                                           ? colorOptions.at(sel).at(0).toUpper()
                                           : QChar();
            Command_RuledPayload *activate = newRuledPayloadActivateManaAbilityForLand(card, desiredColor);
            if (!activate) {
                return false;
            }
            sendGameCommand(*activate);
            delete activate;
            return true;
        }
        // Single-option mana ability: left-click auto-activates, right-click falls through.
        if (leftClick) {
            Command_RuledPayload *activate = newRuledPayloadActivateManaAbilityForLand(card, QChar());
            if (!activate) {
                return false; // not a mana source the engine recognizes; let normal handling continue
            }
            sendGameCommand(*activate);
            delete activate;
            return true;
        }
    }

    // Past the direct mana-float fast path: opening the full activation menu now would overwrite an
    // activated ability that is still mid-payment (the shared pendingActivatedAbility). Suppress it so
    // mana taps keep flowing into that ability instead of starting a second one.
    if (pendingActivatedAbility.valid) {
        return false;
    }

    // Build and show the context menu with full "cost: text" labels (Oracle format).
    QMenu menu;
    menu.setTitle(card->getName());
    QVector<QString> menuLabels;
    menuLabels.reserve(abilityTexts.size());
    for (int i = 0; i < abilityTexts.size(); ++i) {
        const QString label = handler->activatedAbilityMenuLabel(oid, i);
        menuLabels.append(label);
        QAction *action = menu.addAction(label);
        // Disable rather than omit: the indices below are ability indices, and the player still
        // wants to see that the ability exists and why it is unavailable right now.
        action->setEnabled(handler->abilityActivatable(oid, i));
    }
    QAction *chosen = menu.exec(QCursor::pos());
    if (!chosen) {
        return true; // menu was shown, player cancelled
    }

    const int abilityIndex = menuLabels.indexOf(chosen->text());
    if (abilityIndex < 0) {
        return true;
    }

    // Engine-authoritative: ability slot key present in valid_targets_by_ability means it needs a target.
    const bool needsTarget = handler->abilityNeedsTarget(oid, abilityIndex);

    // Look up the mana cost from the engine-supplied cost string (e.g. "4", "R", "").
    // This comes directly from AbilityCost in the tricerules registry — no text parsing.
    const QStringList manaCostStrings = handler->activatedAbilityManaCostsForOid(oid);
    const QString manaCostStr = (abilityIndex < manaCostStrings.size()) ? manaCostStrings.at(abilityIndex) : QString{};
    const QMap<QChar, int> manaCost = parseSimpleManaCost(manaCostStr);
    // CR 107.4d–f: flexible pips ({G/U}, {2/W}, {B/P}) in the ability cost are front-loaded via
    // the choice dialog before mana payment, just like a spell cast (see resolvePendingAbility...).
    const QVector<RuledFlexPip> flexPips = parseFlexPips(manaCostStr);

    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();

    pendingActivatedAbility = {};
    pendingActivatedAbility.valid = true;
    pendingActivatedAbility.permanentOid = oid;
    pendingActivatedAbility.abilityIndex = abilityIndex;
    pendingActivatedAbility.abilityText = chosen->text();
    pendingActivatedAbility.cardName = card->getName();
    pendingActivatedAbility.needsTarget = needsTarget;
    pendingActivatedAbility.waitingForTarget = needsTarget;
    pendingActivatedAbility.selectedTargetOid = 0;
    pendingActivatedAbility.costChoices = handler->abilityCostChoices(oid, abilityIndex);
    pendingActivatedAbility.nextCostChoice = 0;
    pendingActivatedAbility.waitingForCost = false;
    pendingActivatedAbility.waitingForMana = false;
    pendingActivatedAbility.remainingCost = manaCost;
    pendingActivatedAbility.flexPips = flexPips;

    if (needsTarget) {
        // Target first, then mana payment after target is chosen.
        emit ruledActivatedAbilityTargetPendingChanged(true, chosen->text());
        handler->emitLocalLog(tr("Choose a target for: %1").arg(chosen->text()));
    } else {
        continuePendingActivatedAbilityAfterChoice();
    }
    return true;
}

bool PlayerActions::tryHandleRuledAbilityTargetClick(CardItem *card)
{
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (handler && handler->isEngineCommandPending()) {
        return true;
    }

    // CR 614.12 / 707.5: Clone's entering-as-copy choice is untargeted but uses the existing
    // engine-authoritative board click path.
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(tr("Choose a creature on the battlefield for Clone to copy."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 sourceOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (sourceOid == 0 ||
            !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, sourceOid)) {
            handler->emitLocalLog(tr("That is not a creature Clone can copy."));
            return true;
        }
        handler->submitPendingChoiceObject(sourceOid);
        return true;
    }

    // Check pending copy target choice first (CR 707.10c: redirect targets for a spell copy).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget)) {
        if (!card || !card->getZone()) {
            return false;
        }
        const QString zoneName = card->getZone()->getName();
        if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK) {
            handler->emitLocalLog(tr("Select a target on the battlefield or stack for the copy."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (targetOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, targetOid)) {
            handler->emitLocalLog(tr("That is not a valid target for the copy."));
            return true;
        }
        handler->submitPendingChoiceObject(targetOid);
        return true;
    }

    // Check pending legend-rule keep choice (CR 704.5j: click the legend to keep on the battlefield).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::LegendKeep)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(tr("Click the legendary permanent to keep on the battlefield."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 keepOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (keepOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::LegendKeep, keepOid)) {
            handler->emitLocalLog(tr("That is not one of the legends you must choose between."));
            return true;
        }
        handler->submitPendingChoiceObject(keepOid);
        return true;
    }

    // Check pending trigger first (higher priority).
    if (handler && handler->hasPendingTriggerTarget()) {
        if (!card || !card->getZone()) {
            return false;
        }
        const QString zoneName = card->getZone()->getName();
        const bool triggerIsGraveyard = (zoneName == ZoneNames::GRAVE);
        if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK && !triggerIsGraveyard) {
            return false;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        quint32 targetOid = 0;
        if (triggerIsGraveyard) {
            targetOid = handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
        } else {
            targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        }
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

    // Explicit nonmana activated-cost choices are engine-authored. Hand candidates are concealed
    // hand slots and battlefield candidates are ObjectIds; never infer legality from card text.
    if (pendingActivatedAbility.valid && pendingActivatedAbility.waitingForCost) {
        if (!card || !card->getZone() ||
            pendingActivatedAbility.nextCostChoice >= pendingActivatedAbility.costChoices.size()) {
            return true;
        }
        const auto &choice = pendingActivatedAbility.costChoices.at(pendingActivatedAbility.nextCostChoice);
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        quint32 selectedId = 0;
        if (choice.zone == RuledAbilityCostChoiceZone::Hand) {
            if (card->getZone()->getName() != ZoneNames::HAND) {
                handler->emitLocalLog(tr("Choose a card from your hand to discard."));
                return true;
            }
            const int handSlot = handler->engineHandSlotForServerCard(ownerPlayerId, card->getId());
            if (handSlot < 0) {
                handler->emitLocalLog(tr("That hand card is not selectable yet."));
                return true;
            }
            selectedId = static_cast<quint32>(handSlot);
        } else {
            if (card->getZone()->getName() != ZoneNames::TABLE) {
                handler->emitLocalLog(tr("Choose a permanent on the battlefield to sacrifice."));
                return true;
            }
            selectedId = handler->engineOidForCardId(ownerPlayerId, card->getId());
        }
        if (selectedId == 0 && choice.zone == RuledAbilityCostChoiceZone::Battlefield) {
            handler->emitLocalLog(tr("That permanent is not selectable yet."));
            return true;
        }
        if (!choice.candidateIds.contains(selectedId)) {
            handler->emitLocalLog(tr("That object cannot pay this ability cost."));
            return true;
        }
        for (const auto &already : pendingActivatedAbility.costSelections) {
            if (already.zone == choice.zone && already.selectedId == selectedId) {
                handler->emitLocalLog(tr("One object cannot pay two cost components."));
                return true;
            }
        }
        pendingActivatedAbility.costSelections.append({choice.costIndex, choice.zone, selectedId});
        ++pendingActivatedAbility.nextCostChoice;
        pendingActivatedAbility.waitingForCost = false;
        continuePendingActivatedAbilityAfterChoice();
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
        handler->emitLocalLog(tr("Select a target on the battlefield (or stack), or press Cancel."));
        return true;
    }
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (targetOid == 0) {
        handler->emitLocalLog(tr("That target is not selectable yet."));
        return true;
    }
    if (!handler->isValidAbilityTarget(pendingActivatedAbility.permanentOid, pendingActivatedAbility.abilityIndex,
                                       targetOid)) {
        handler->emitLocalLog(tr("That is not a legal target for: %1").arg(pendingActivatedAbility.abilityText));
        return true;
    }

    pendingActivatedAbility.selectedTargetOid = targetOid;
    pendingActivatedAbility.waitingForTarget = false;
    emit ruledActivatedAbilityTargetPendingChanged(false, {});
    continuePendingActivatedAbilityAfterChoice();
    return true;
}

bool PlayerActions::tryHandleRuledAbilityTargetPlayerClick(Player *targetPlayer)
{
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (handler && handler->isEngineCommandPending()) {
        return true;
    }

    // Check pending copy target choice first (CR 707.10c: redirect targets for a spell copy).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget)) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, targetOid)) {
            handler->emitLocalLog(tr("That player is not a valid target for the copy."));
            return true;
        }
        handler->submitPendingChoiceObject(targetOid);
        return true;
    }

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
        handler->emitLocalLog(tr("You cannot target yourself with: %1").arg(pendingActivatedAbility.abilityText));
        return true;
    }
    if (!isSelf && !handler->canAbilityTargetOpponent(permOid, abilityIdx)) {
        handler->emitLocalLog(tr("You cannot target that player with: %1").arg(pendingActivatedAbility.abilityText));
        return true;
    }
    pendingActivatedAbility.selectedTargetOid = targetOid;
    pendingActivatedAbility.waitingForTarget = false;
    emit ruledActivatedAbilityTargetPendingChanged(false, {});
    continuePendingActivatedAbilityAfterChoice();
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
