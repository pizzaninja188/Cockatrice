#include "game_prompt_widget.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QObject>
#include <QPushButton>
#include <QVBoxLayout>
#include <QtAlgorithms>

namespace {
QString extractPrimaryPrompt(const QString &ruledLog)
{
    if (ruledLog.trimmed().isEmpty()) {
        return {};
    }

    const QStringList lines = ruledLog.split('\n', Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        const QString trimmed = line.trimmed();
        if (trimmed.contains(QStringLiteral("Assign combat damage"), Qt::CaseInsensitive) ||
            trimmed.contains(QStringLiteral("Assign damage order"), Qt::CaseInsensitive)) {
            QString t = trimmed;
            if (t.startsWith(QChar(0x2014))) {
                t = t.mid(1).trimmed();
            }
            return t;
        }
    }
    for (const QString &line : lines) {
        const QString trimmed = line.trimmed();
        if (trimmed.startsWith(QStringLiteral("Priority:")) || trimmed.startsWith(QStringLiteral("Phase:"))) {
            return trimmed;
        }
    }
    for (const QString &line : lines) {
        const QString trimmed = line.trimmed();
        if (!trimmed.startsWith(QChar(0x2014))) {
            return trimmed;
        }
    }

    return lines.first().trimmed();
}

QString currentPhaseDisplayName(int phase)
{
    switch (phase) {
        case 0:
            return GamePromptWidget::tr("Untap Step");
        case 1:
            return GamePromptWidget::tr("Upkeep Step");
        case 2:
            return GamePromptWidget::tr("Draw Step");
        case 3:
            return GamePromptWidget::tr("First Main Phase");
        case 4:
            return GamePromptWidget::tr("Beginning of Combat");
        case 5:
            return GamePromptWidget::tr("Declare Attackers Step");
        case 6:
            return GamePromptWidget::tr("Declare Blockers Step");
        case 7:
            return GamePromptWidget::tr("Combat Damage Step");
        case 8:
            return GamePromptWidget::tr("End of Combat Step");
        case 9:
            return GamePromptWidget::tr("Second Main Phase");
        case 10:
            return GamePromptWidget::tr("End Step");
        default:
            return {};
    }
}

QString nextStepButtonTextForPhase(int phase)
{
    // Returns the name of the phase we are passing *to* (current + 1).
    // Indices match `PhasesToolbar` / `GameState::activePhaseChanged` (0 = untap … 10 = end step).
    switch (phase) {
        case 0:
            return GamePromptWidget::tr("Upkeep Step");
        case 1:
            return GamePromptWidget::tr("Draw Step");
        case 2:
            return GamePromptWidget::tr("First Main Phase");
        case 3:
            return GamePromptWidget::tr("Beginning of Combat");
        case 4:
            return GamePromptWidget::tr("Declare Attackers");
        case 5:
            return GamePromptWidget::tr("Declare Blockers");
        case 6:
            return GamePromptWidget::tr("Combat Damage");
        case 7:
            return GamePromptWidget::tr("End of Combat");
        case 8:
            return GamePromptWidget::tr("Second Main Phase");
        case 9:
            return GamePromptWidget::tr("End Step");
        case 10:
            return GamePromptWidget::tr("Next Turn");
        default:
            return GamePromptWidget::tr("Pass Priority");
    }
}
} // namespace

GamePromptWidget::GamePromptWidget(QWidget *parent) : QWidget(parent)
{
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(6, 6, 6, 6);
    layout->setSpacing(4);

    promptLabel = new QLabel(this);
    promptLabel->setObjectName("promptLabel");
    promptLabel->setWordWrap(true);
    promptLabel->setMinimumHeight(34);
    layout->addWidget(promptLabel);

    auto *openingRowLayout = new QHBoxLayout;
    openingRowLayout->setSpacing(4);
    openingPickSeatButton1 = new QPushButton(this);
    openingPickSeatButton1->setObjectName("openingPickSeatButton1");
    openingPickSeatButton2 = new QPushButton(this);
    openingPickSeatButton2->setObjectName("openingPickSeatButton2");
    openingKeepButton = new QPushButton(this);
    openingKeepButton->setObjectName("openingKeepButton");
    openingMulliganButton = new QPushButton(this);
    openingMulliganButton->setObjectName("openingMulliganButton");
    openingPickSeatButton1->hide();
    openingPickSeatButton2->hide();
    openingKeepButton->hide();
    openingMulliganButton->hide();
    openingRowLayout->addWidget(openingPickSeatButton1);
    openingRowLayout->addWidget(openingPickSeatButton2);
    openingRowLayout->addWidget(openingKeepButton);
    openingRowLayout->addWidget(openingMulliganButton);
    openingBottomCancelButton = new QPushButton(this);
    openingBottomCancelButton->setObjectName("openingBottomCancelButton");
    openingBottomDoneButton = new QPushButton(this);
    openingBottomDoneButton->setObjectName("openingBottomDoneButton");
    openingBottomCancelButton->hide();
    openingBottomDoneButton->hide();
    openingRowLayout->addWidget(openingBottomCancelButton);
    openingRowLayout->addWidget(openingBottomDoneButton);
    layout->addLayout(openingRowLayout);
    connect(openingKeepButton, &QPushButton::clicked, this, &GamePromptWidget::ruledOpeningMulliganKeepRequested);
    connect(openingMulliganButton, &QPushButton::clicked, this, &GamePromptWidget::ruledOpeningMulliganRedrawRequested);
    connect(openingBottomCancelButton, &QPushButton::clicked,
            this, &GamePromptWidget::ruledOpeningBottomCancelRequested);
    connect(openingBottomDoneButton, &QPushButton::clicked,
            this, &GamePromptWidget::ruledOpeningBottomDoneRequested);

    resolutionHandPickConfirmButton = new QPushButton(this);
    resolutionHandPickConfirmButton->setObjectName("resolutionHandPickConfirmButton");
    resolutionHandPickConfirmButton->hide();
    connect(resolutionHandPickConfirmButton, &QPushButton::clicked, this,
            &GamePromptWidget::ruledResolutionHandPickConfirmRequested);
    layout->addWidget(resolutionHandPickConfirmButton);

    auto *resolutionPaymentRow = new QHBoxLayout;
    resolutionPaymentRow->setContentsMargins(0, 0, 0, 0);
    resolutionPaymentRow->setSpacing(4);
    resolutionPaymentDeclineButton = new QPushButton(this);
    resolutionPaymentDeclineButton->setObjectName("resolutionPaymentDeclineButton");
    resolutionPaymentDeclineButton->hide();
    connect(resolutionPaymentDeclineButton, &QPushButton::clicked, this,
            &GamePromptWidget::ruledResolutionPaymentDeclineRequested);
    resolutionPaymentRow->addWidget(resolutionPaymentDeclineButton);
    resolutionPaymentPayButton = new QPushButton(this);
    resolutionPaymentPayButton->setObjectName("resolutionPaymentPayButton");
    resolutionPaymentPayButton->hide();
    connect(resolutionPaymentPayButton, &QPushButton::clicked, this,
            &GamePromptWidget::ruledResolutionPaymentPayRequested);
    resolutionPaymentRow->addWidget(resolutionPaymentPayButton);
    layout->addLayout(resolutionPaymentRow);

    choiceOptionsRow = new QHBoxLayout;
    choiceOptionsRow->setContentsMargins(0, 0, 0, 0);
    choiceOptionsRow->setSpacing(4);
    layout->addLayout(choiceOptionsRow);

    passPriorityButton = new QPushButton(this);
    passPriorityButton->setObjectName("passPriorityButton");
    connect(passPriorityButton, &QPushButton::clicked, this, &GamePromptWidget::passPriorityRequested);
    layout->addWidget(passPriorityButton);

    auto *combatRow = new QHBoxLayout;
    combatRow->setContentsMargins(0, 0, 0, 0);
    combatRow->setSpacing(4);

    confirmAttackersButton = new QPushButton(this);
    confirmAttackersButton->setObjectName("confirmAttackersButton");
    connect(confirmAttackersButton, &QPushButton::clicked, this, &GamePromptWidget::confirmAttackersRequested);
    combatRow->addWidget(confirmAttackersButton);

    confirmBlockersButton = new QPushButton(this);
    confirmBlockersButton->setObjectName("confirmBlockersButton");
    connect(confirmBlockersButton, &QPushButton::clicked, this, &GamePromptWidget::confirmBlockersRequested);
    combatRow->addWidget(confirmBlockersButton);

    resetBlockersButton = new QPushButton(this);
    resetBlockersButton->setObjectName("resetBlockersButton");
    connect(resetBlockersButton, &QPushButton::clicked, this, &GamePromptWidget::resetBlockersRequested);
    combatRow->addWidget(resetBlockersButton);

    confirmCombatDamageButton = new QPushButton(this);
    confirmCombatDamageButton->setObjectName("confirmCombatDamageButton");
    connect(confirmCombatDamageButton, &QPushButton::clicked, this, &GamePromptWidget::confirmCombatDamageRequested);
    combatRow->addWidget(confirmCombatDamageButton);

    cancelTargetingButton = new QPushButton(this);
    cancelTargetingButton->setObjectName("cancelTargetingButton");
    connect(cancelTargetingButton, &QPushButton::clicked, this, &GamePromptWidget::cancelTargetingRequested);

    declineClickChoiceButton = new QPushButton(this);
    declineClickChoiceButton->setObjectName("declineClickChoiceButton");
    declineClickChoiceButton->hide();
    connect(declineClickChoiceButton, &QPushButton::clicked, this, &GamePromptWidget::declineClickChoiceRequested);

    confirmSpellDamageButton = new QPushButton(this);
    confirmSpellDamageButton->setObjectName("confirmSpellDamageButton");
    confirmSpellDamageButton->hide();
    connect(confirmSpellDamageButton, &QPushButton::clicked, this, &GamePromptWidget::confirmSpellDamageRequested);

    confirmTargetsButton = new QPushButton(this);
    confirmTargetsButton->setObjectName("confirmTargetsButton");
    connect(confirmTargetsButton, &QPushButton::clicked, this, &GamePromptWidget::confirmTargetsRequested);

    undoLandTapButton = new QPushButton(this);
    undoLandTapButton->setObjectName("undoLandTapButton");
    connect(undoLandTapButton, &QPushButton::clicked, this, &GamePromptWidget::undoLandTapRequested);

    auto *actionRow = new QHBoxLayout;
    actionRow->setContentsMargins(0, 0, 0, 0);
    actionRow->setSpacing(4);
    actionRow->addWidget(cancelTargetingButton);
    actionRow->addWidget(declineClickChoiceButton);
    actionRow->addWidget(confirmTargetsButton);
    actionRow->addWidget(confirmSpellDamageButton);
    actionRow->addWidget(undoLandTapButton);
    layout->addLayout(actionRow);

    layout->addLayout(combatRow);

    fallbackPromptText = tr("Waiting for ruled action prompt...");
    updateCombatButtonsVisibility();
    retranslateUi();
}

void GamePromptWidget::retranslateUi()
{
    if (promptLabel->text().isEmpty() || promptLabel->text() == fallbackPromptText) {
        fallbackPromptText = tr("Waiting for ruled action prompt...");
        promptLabel->setText(fallbackPromptText);
    }
    updatePassPriorityButtonText();
    confirmAttackersButton->setText(tr("OK"));
    confirmBlockersButton->setText(tr("OK"));
    resetBlockersButton->setText(tr("Reset Blockers"));
    confirmCombatDamageButton->setText(tr("OK"));
    cancelTargetingButton->setText(tr("Cancel"));
    declineClickChoiceButton->setText(tr("Decline"));
    confirmTargetsButton->setText(tr("Confirm Targets"));
    confirmSpellDamageButton->setText(tr("Confirm Damage"));
    undoLandTapButton->setText(tr("Undo"));
    openingKeepButton->setText(tr("Keep"));
    openingMulliganButton->setText(tr("Mulligan"));
    openingBottomCancelButton->setText(tr("Cancel"));
    openingBottomDoneButton->setText(tr("Done"));
    if (promptState.mode == PromptMode::OpeningChooseFirst && promptState.openingPickSeatIds.size() >= 2) {
        openingPickSeatButton1->setText(tr("You"));
        openingPickSeatButton2->setText(tr("Opponent"));
    }
    resolutionHandPickConfirmButton->setText(tr("Confirm"));
    resolutionPaymentDeclineButton->setText(tr("Decline"));
    resolutionPaymentPayButton->setText(tr("Pay"));
}

// ---------------------------------------------------------------------------------------
// Prompt mode
// ---------------------------------------------------------------------------------------

GamePromptWidget::PromptMode GamePromptWidget::effectiveMode() const
{
    // One priority chain, resolved here and nowhere else: a take-over mode outranks the
    // mid-cast targeting state, which outranks a parked click-a-permanent choice.
    switch (promptState.mode) {
        case PromptMode::CommandPending:
        case PromptMode::UpdatingGame:
        case PromptMode::ResolutionPick:
        case PromptMode::ResolutionPayment:
        case PromptMode::ChoiceOptions:
        case PromptMode::WaitingForChoice:
        // The engine is hard-blocked on the ordering answer, so a leftover mid-cast targeting
        // state cannot legitimately coexist with it — this takes over.
        case PromptMode::TriggerOrder:
        case PromptMode::OpeningChooseFirst:
        case PromptMode::OpeningMulligan:
        case PromptMode::OpeningBottom:
        case PromptMode::CleanupDiscard:
            return promptState.mode;
        default:
            break;
    }
    if (targetingSources) {
        return PromptMode::Targeting;
    }
    if (promptState.mode == PromptMode::ClickChoice) {
        return PromptMode::ClickChoice;
    }
    return PromptMode::Normal;
}

void GamePromptWidget::setRuledPromptState(RuledPromptState newState)
{
    promptState = std::move(newState);
    qDeleteAll(choiceOptionButtons);
    choiceOptionButtons.clear();
    for (const auto &option : promptState.choiceOptions) {
        auto *button = new QPushButton(option.label, this);
        button->setObjectName(QStringLiteral("ruledChoiceOptionButton_%1").arg(option.index));
        button->setEnabled(option.enabled);
        connect(button, &QPushButton::clicked, this,
                [this, index = option.index] { emit ruledChoiceOptionRequested(index); });
        choiceOptionsRow->addWidget(button);
        choiceOptionButtons.append(button);
    }
    // The seat buttons carry the seat ids in their click handlers, so rewire them on entry.
    openingPickSeatButton1->disconnect();
    openingPickSeatButton2->disconnect();
    if (promptState.mode == PromptMode::OpeningChooseFirst && promptState.openingPickSeatIds.size() >= 2) {
        openingPickSeatButton1->setText(tr("You"));
        openingPickSeatButton2->setText(tr("Opponent"));
        const int selfSeatId = promptState.openingPickSeatIds[0];
        const int opponentSeatId = promptState.openingPickSeatIds[1];
        QObject::connect(openingPickSeatButton1, &QPushButton::clicked, this,
                         [this, selfSeatId] { emit ruledOpeningPickSeatRequested(selfSeatId); });
        QObject::connect(openingPickSeatButton2, &QPushButton::clicked, this,
                         [this, opponentSeatId] { emit ruledOpeningPickSeatRequested(opponentSeatId); });
    }
    applyPromptStateText();
    updateCombatButtonsVisibility();
    refreshPromptLabel();
}

void GamePromptWidget::applyPromptStateText()
{
    switch (promptState.mode) {
        case PromptMode::CommandPending:
            // Preserve the last settled prompt during the sub-150 ms input lock.
            return;
        case PromptMode::UpdatingGame:
            setPromptText(tr("Updating game%1").arg(QChar(0x2026)));
            return;
        case PromptMode::CleanupDiscard:
            if (promptState.required > 0) {
                setPromptText(tr("Cleanup — discard %2 card(s) to reach hand size 7. Selected: %1 of %2. Click hand "
                                 "cards to toggle; click again to deselect.")
                                  .arg(promptState.selected)
                                  .arg(promptState.required));
            }
            return;
        case PromptMode::OpeningChooseFirst:
            setPromptText(tr("Choose who goes first."));
            return;
        case PromptMode::OpeningMulligan: {
            const int keepCount = 7 - promptState.required;
            setPromptText(tr("Mulligan to %1 or keep these %2?").arg(keepCount - 1).arg(keepCount));
            return;
        }
        case PromptMode::OpeningBottom:
            setPromptText(tr("Put %1 card(s) to the bottom of your library.").arg(promptState.required));
            return;
        case PromptMode::ClickChoice:
        case PromptMode::ResolutionPick:
        case PromptMode::ResolutionPayment:
        case PromptMode::ChoiceOptions:
        case PromptMode::WaitingForChoice:
        case PromptMode::TriggerOrder:
            // Engine-authored: the caller passed the prompt the engine wrote.
            setPromptText(promptState.text);
            return;
        case PromptMode::Normal:
            if (!promptState.text.isEmpty()) {
                setPromptText(promptState.text);
            } else if (effectiveMode() == PromptMode::Normal) {
                // Nothing owns the label any more; refreshPromptLabel() recomposes it below.
                setPromptText({});
            }
            return;
        case PromptMode::Targeting:
            // Never pushed — see PromptMode::Targeting.
            return;
    }
}

void GamePromptWidget::setMultiTargetSelectionCount(int selected, int minTargets, int maxTargets)
{
    multiTargetSelectedCount = selected;
    multiTargetMinCount = minTargets;
    multiTargetMaxCount = maxTargets;
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setSpellDamageAllocationStatus(bool active, int assigned, int total)
{
    spellDamageAllocationMode = active;
    if (active) {
        const bool legal = (total > 0 && assigned == total);
        setPromptText(tr("Assign %1 damage — %2/%3 assigned. "
                         "Click targets to add, right-click to reduce.")
                          .arg(total).arg(assigned).arg(total));
        confirmSpellDamageButton->setEnabled(legal);
    }
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setPromptText(const QString &promptText)
{
    if (promptText.trimmed().isEmpty()) {
        promptLabel->setText(fallbackPromptText);
        return;
    }
    promptLabel->setText(promptText.trimmed());
}

void GamePromptWidget::setPromptFromRuledLog(const QString &ruledLog)
{
    const QString prompt = extractPrimaryPrompt(ruledLog);
    if (prompt.isEmpty()) {
        setPromptText({});
        return;
    }
    setPromptText(prompt);
}

void GamePromptWidget::setPassPriorityEnabled(bool enabled)
{
    passPriorityButton->setEnabled(enabled);
}

void GamePromptWidget::setActivePhase(int phase)
{
    if (phase == currentActivePhase) {
        return;
    }
    currentActivePhase = phase;
    updatePassPriorityButtonText();
    refreshPromptLabel();
}

void GamePromptWidget::setLocalPlayerHasPriority(bool hasPriority)
{
    if (localPlayerHasPriority == hasPriority) {
        return;
    }
    localPlayerHasPriority = hasPriority;
    updateCombatButtonsVisibility();
    refreshPromptLabel();
}

void GamePromptWidget::setCombatMode(CombatMode mode, bool localPlayerHasButtons, bool declarationSatisfied)
{
    if (mode == currentCombatMode && localPlayerHasButtons == localPlayerHasCombatButtons &&
        declarationSatisfied == combatDeclarationSatisfied) {
        return;
    }
    combatDeclarationSatisfied = declarationSatisfied;
    // Clear the sticky rejection label whenever we leave the "defender has buttons" state:
    // either the phase advanced past declare-blockers, or legal blocks were accepted and the
    // local player no longer has blocker buttons (blockersSubmittedThisStep flipped true).
    if (!stickyBlockerError.isEmpty() &&
        (mode != CombatMode::DeclareBlockers || !localPlayerHasButtons)) {
        stickyBlockerError.clear();
    }
    currentCombatMode = mode;
    localPlayerHasCombatButtons = localPlayerHasButtons;
    updateCombatButtonsVisibility();
    updatePassPriorityButtonText();
    refreshPromptLabel();
}

void GamePromptWidget::setStickyBlockerError(const QString &msg)
{
    stickyBlockerError = msg;
    refreshPromptLabel();
}

void GamePromptWidget::setTargetingSource(TargetingSource source, bool active)
{
    const TargetingSources next = active ? (targetingSources | source) : (targetingSources & ~TargetingSources(source));
    if (next == targetingSources) {
        return;
    }
    targetingSources = next;
    updateCombatButtonsVisibility();
    refreshPromptLabel();
}

void GamePromptWidget::setTargetingMode(bool enabled, const QString &effectText)
{
    // Unlike the other two sources this one always re-announces itself: re-entering targeting for
    // a different mode of the same spell must replace the effect text on the label.
    if (enabled) {
        setPromptText(effectText);
    }
    setTargetingSource(TargetingSource::SpellTargetSelection, enabled);
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setRuledStackHasItems(bool hasItems)
{
    if (ruledStackHasItems == hasItems) {
        return;
    }
    ruledStackHasItems = hasItems;
    updatePassPriorityButtonText();
    refreshPromptLabel();
}

void GamePromptWidget::setFirstStrikeStepPending(bool pending)
{
    if (firstStrikeStepPending == pending) {
        return;
    }
    firstStrikeStepPending = pending;
    updatePassPriorityButtonText();
}

void GamePromptWidget::setFirstStrikeDamageStepActive(bool active)
{
    if (firstStrikeDamageStepActive == active) {
        return;
    }
    firstStrikeDamageStepActive = active;
    updatePassPriorityButtonText();
    refreshPromptLabel();
}

void GamePromptWidget::setLandTapUndoAvailable(bool available)
{
    if (landTapUndoAvailable == available) {
        return;
    }
    landTapUndoAvailable = available;
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setSpellCastPending(bool pending)
{
    setTargetingSource(TargetingSource::SpellCastPending, pending);
}

void GamePromptWidget::setActivatedAbilityTargetPending(bool pending, const QString &abilityText)
{
    if (targetingSources.testFlag(TargetingSource::AbilityTargetPending) == pending) {
        return;
    }
    if (pending) {
        setPromptText(tr("Choose a target for “%1”, or press Cancel.").arg(abilityText));
    }
    setTargetingSource(TargetingSource::AbilityTargetPending, pending);
}

void GamePromptWidget::setCombatDamageStatus(const QString &attackerName, int assigned, int power,
                                              int playerDamage, bool legal)
{
    if (attackerName.isEmpty()) {
        confirmCombatDamageButton->setEnabled(false);
        return;
    }
    QString detail;
    if (playerDamage > 0) {
        // Trample: show blocker assignment and implied player damage separately.
        detail = tr("Assigned %1 to blockers, %2 tramples to player (of %3).")
                     .arg(assigned)
                     .arg(playerDamage)
                     .arg(power);
    } else {
        detail = tr("Assigned %1 of %2.").arg(assigned).arg(power);
    }
    setPromptText(tr("Assign combat damage for %1\n%2").arg(attackerName).arg(detail));
    // OK button: legal already validates totals; power > 0 guards against 0-power edge case.
    confirmCombatDamageButton->setEnabled(legal && power > 0);
}

void GamePromptWidget::hideActionAndCombatButtons()
{
    passPriorityButton->setVisible(false);
    confirmAttackersButton->setVisible(false);
    confirmBlockersButton->setVisible(false);
    resetBlockersButton->setVisible(false);
    confirmCombatDamageButton->setVisible(false);
    cancelTargetingButton->setVisible(false);
    declineClickChoiceButton->setVisible(false);
    confirmTargetsButton->setVisible(false);
    undoLandTapButton->setVisible(false);
    resolutionPaymentDeclineButton->setVisible(false);
    resolutionPaymentPayButton->setVisible(false);
}

void GamePromptWidget::updateCombatButtonsVisibility()
{
    const PromptMode mode = effectiveMode();
    confirmSpellDamageButton->setVisible(false);

    // Mode-owned buttons: shown by exactly one mode each, hidden everywhere else.
    const bool showSeatPicks = mode == PromptMode::OpeningChooseFirst && !promptState.openingPickSeatIds.isEmpty();
    openingPickSeatButton1->setVisible(showSeatPicks);
    openingPickSeatButton2->setVisible(showSeatPicks && promptState.openingPickSeatIds.size() >= 2);
    openingKeepButton->setVisible(mode == PromptMode::OpeningMulligan);
    // No mulligan below a zero-card hand.
    openingMulliganButton->setVisible(mode == PromptMode::OpeningMulligan && (7 - promptState.required) - 1 >= 0);
    openingBottomCancelButton->setVisible(mode == PromptMode::OpeningBottom && promptState.selected >= 1);
    openingBottomDoneButton->setVisible(mode == PromptMode::OpeningBottom && promptState.required > 0 &&
                                        promptState.selected == promptState.required);
    resolutionHandPickConfirmButton->setVisible(mode == PromptMode::ResolutionPick);
    resolutionPaymentDeclineButton->setVisible(mode == PromptMode::ResolutionPayment);
    resolutionPaymentPayButton->setVisible(mode == PromptMode::ResolutionPayment);
    resolutionPaymentPayButton->setEnabled(promptState.paymentCurrentlyLegal);
    for (auto *button : choiceOptionButtons) {
        button->setVisible(mode == PromptMode::ChoiceOptions);
    }
    declineClickChoiceButton->setVisible(
        (mode == PromptMode::ClickChoice || mode == PromptMode::ChoiceOptions) && promptState.canDecline);
    if (mode == PromptMode::ResolutionPick) {
        resolutionHandPickConfirmButton->setEnabled(promptState.selected >= promptState.required);
    }

    // Every take-over mode suppresses the priority / combat / targeting controls.
    if (mode != PromptMode::Normal && mode != PromptMode::Targeting) {
        hideActionAndCombatButtons();
        declineClickChoiceButton->setVisible(
            (mode == PromptMode::ClickChoice || mode == PromptMode::ChoiceOptions) && promptState.canDecline);
        resolutionPaymentDeclineButton->setVisible(mode == PromptMode::ResolutionPayment);
        resolutionPaymentPayButton->setVisible(mode == PromptMode::ResolutionPayment);
        resolutionPaymentPayButton->setEnabled(promptState.paymentCurrentlyLegal);
        undoLandTapButton->setVisible(mode == PromptMode::ResolutionPayment && landTapUndoAvailable);
        return;
    }

    if (mode == PromptMode::Targeting) {
        hideActionAndCombatButtons();
        cancelTargetingButton->setVisible(true);
        if (spellDamageAllocationMode) {
            // enabled state is managed by setSpellDamageAllocationStatus
            confirmSpellDamageButton->setVisible(true);
            return;
        }
        // Multi-target spells confirm an in-progress selection; single-target ones just wait.
        confirmTargetsButton->setVisible(targetingSources.testFlag(TargetingSource::SpellTargetSelection) &&
                                         multiTargetMaxCount >= 0 && multiTargetSelectedCount >= 0);
        confirmTargetsButton->setEnabled(multiTargetSelectedCount >= multiTargetMinCount &&
                                         multiTargetSelectedCount <= multiTargetMaxCount);
        return;
    }

    const bool showAttackers =
        localPlayerHasPriority && currentCombatMode == CombatMode::DeclareAttackers && localPlayerHasCombatButtons;
    const bool showBlockers =
        localPlayerHasPriority && currentCombatMode == CombatMode::DeclareBlockers && localPlayerHasCombatButtons;
    // Assign combat damage UI is driven by combat role, not priority (AP assigns; engine validates).
    const bool showCombatDamage =
        currentCombatMode == CombatMode::AssignCombatDamage && localPlayerHasCombatButtons;
    const bool waitingOnOpponentCombatDamage =
        currentCombatMode == CombatMode::AssignCombatDamage && !localPlayerHasCombatButtons;
    passPriorityButton->setVisible(localPlayerHasPriority && !showAttackers && !showBlockers && !showCombatDamage &&
                                   !waitingOnOpponentCombatDamage);
    confirmAttackersButton->setVisible(showAttackers);
    confirmBlockersButton->setVisible(showBlockers);
    // CR 508.1d / 509.1c: gray out OK while a required attacker/blocker is still unstaged, so the
    // player cannot submit a declaration the engine would reject (which softlocks the combat step).
    confirmAttackersButton->setEnabled(combatDeclarationSatisfied);
    confirmBlockersButton->setEnabled(combatDeclarationSatisfied);
    if (showAttackers) {
        confirmAttackersButton->setToolTip(
            combatDeclarationSatisfied
                ? QString()
                : tr("You must attack with all creatures that are required to attack."));
    }
    if (showBlockers) {
        confirmBlockersButton->setToolTip(
            combatDeclarationSatisfied
                ? QString()
                : tr("You must block with all creatures that are required to block."));
    }
    resetBlockersButton->setVisible(showBlockers);
    confirmCombatDamageButton->setVisible(showCombatDamage);
    cancelTargetingButton->setVisible(false);
    confirmTargetsButton->setVisible(false);
    undoLandTapButton->setVisible(localPlayerHasPriority && landTapUndoAvailable && !showAttackers && !showBlockers &&
                                   !showCombatDamage && !waitingOnOpponentCombatDamage);
}

void GamePromptWidget::updatePassPriorityButtonText()
{
    if (ruledStackHasItems) {
        passPriorityButton->setText(tr("No Response"));
        return;
    }
    // CR 510.4: the button is always a forward-label (the step we are passing *into*).
    // Inside the first-strike damage substep, pressing passes into the regular combat-damage
    // step, so the label is "Combat Damage".
    // While a first-strike substep is still pending (declare-blockers phase with a FS/DS
    // combatant already in combat), pressing will lead into the first-strike step, so
    // "First Strike Damage".
    if (firstStrikeDamageStepActive) {
        passPriorityButton->setText(tr("Combat Damage"));
        return;
    }
    if (firstStrikeStepPending && currentCombatMode == CombatMode::DeclareBlockers) {
        passPriorityButton->setText(tr("First Strike Damage"));
        return;
    }
    passPriorityButton->setText(nextStepButtonTextForPhase(currentActivePhase));
}

void GamePromptWidget::setActivePlayerName(const QString &name)
{
    activePlayerName = name;
    refreshPromptLabel();
}

void GamePromptWidget::setPriorityPlayerName(const QString &name)
{
    priorityPlayerName = name;
    refreshPromptLabel();
}

void GamePromptWidget::setLocalPlayerIsActive(bool isActive)
{
    localPlayerIsActive = isActive;
    refreshPromptLabel();
}

void GamePromptWidget::refreshPromptLabel()
{
    switch (effectiveMode()) {
        case PromptMode::Normal:
            // A caller-supplied line (e.g. the opening phase's "Waiting for …") outranks the
            // composed phase/priority line below.
            if (!promptState.text.isEmpty()) {
                return;
            }
            break;
        default:
            // Every other mode owns the label; applyPromptStateText / the targeting setters wrote it.
            return;
    }
    if (currentCombatMode == CombatMode::AssignCombatDamage) {
        return;
    }

    const QString &waitName = priorityPlayerName.isEmpty() ? activePlayerName : priorityPlayerName;

    if (currentCombatMode == CombatMode::DeclareAttackers) {
        if (localPlayerHasCombatButtons) {
            if (!combatDeclarationSatisfied) {
                promptLabel->setText(
                    tr("%1's Declare Attackers step. Some creatures must attack — declare them to continue.")
                        .arg(activePlayerName));
            } else {
                promptLabel->setText(tr("%1's Declare Attackers step. Choose attackers.").arg(activePlayerName));
            }
        } else if (localPlayerHasPriority) {
            promptLabel->setText(
                tr("%1's Declare Attackers step. Cast instants and activate abilities.").arg(activePlayerName));
        } else {
            promptLabel->setText(tr("Waiting for %1...").arg(waitName));
        }
        return;
    }
    if (currentCombatMode == CombatMode::DeclareBlockers) {
        if (localPlayerHasCombatButtons) {
            if (!stickyBlockerError.isEmpty()) {
                promptLabel->setText(stickyBlockerError);
            } else if (!combatDeclarationSatisfied) {
                promptLabel->setText(
                    tr("%1's Declare Blockers step. Some creatures must block — declare them to continue.")
                        .arg(activePlayerName));
            } else {
                promptLabel->setText(tr("%1's Declare Blockers step. Choose blockers.").arg(activePlayerName));
            }
        } else if (localPlayerHasPriority) {
            promptLabel->setText(
                tr("%1's Declare Blockers step. Cast instants and activate abilities.").arg(activePlayerName));
        } else {
            promptLabel->setText(tr("Waiting for %1...").arg(waitName));
        }
        return;
    }

    if (activePlayerName.isEmpty()) {
        return;
    }

    if (!localPlayerHasPriority) {
        promptLabel->setText(tr("Waiting for %1...").arg(waitName));
        return;
    }

    // CR 510.4: phase 7 hosts both the first-strike and regular combat damage substeps; use a
    // distinct label while inside the first-strike substep so the prompt doesn't read like a
    // generic "Combat Damage Step".
    const QString phaseName = firstStrikeDamageStepActive
        ? tr("First Strike Damage Step")
        : currentPhaseDisplayName(currentActivePhase);
    if (phaseName.isEmpty()) {
        return;
    }
    const bool isMyMainPhase = localPlayerIsActive && (currentActivePhase == 3 || currentActivePhase == 9) && !ruledStackHasItems;
    const QString actions = isMyMainPhase
        ? tr("Cast spells, activate abilities, and play land.")
        : tr("Cast instants and activate abilities.");
    promptLabel->setText(tr("%1's %2. %3").arg(activePlayerName, phaseName, actions));
}
