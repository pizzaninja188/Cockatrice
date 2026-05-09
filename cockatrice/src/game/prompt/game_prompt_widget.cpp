#include "game_prompt_widget.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QObject>
#include <QPushButton>
#include <QVBoxLayout>

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

    promptTitleLabel = nullptr;

    promptLabel = new QLabel(this);
    promptLabel->setObjectName("promptLabel");
    promptLabel->setWordWrap(true);
    promptLabel->setMinimumHeight(34);
    layout->addWidget(promptLabel);

    openingRowLayout = new QHBoxLayout;
    openingRowLayout->setSpacing(4);
    openingPickSeatButton1 = new QPushButton(this);
    openingPickSeatButton2 = new QPushButton(this);
    openingKeepButton = new QPushButton(this);
    openingMulliganButton = new QPushButton(this);
    openingPickSeatButton1->hide();
    openingPickSeatButton2->hide();
    openingKeepButton->hide();
    openingMulliganButton->hide();
    openingRowLayout->addWidget(openingPickSeatButton1);
    openingRowLayout->addWidget(openingPickSeatButton2);
    openingRowLayout->addWidget(openingKeepButton);
    openingRowLayout->addWidget(openingMulliganButton);
    openingBottomCancelButton = new QPushButton(this);
    openingBottomDoneButton = new QPushButton(this);
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

    undoLandTapButton = new QPushButton(this);
    undoLandTapButton->setObjectName("undoLandTapButton");
    connect(undoLandTapButton, &QPushButton::clicked, this, &GamePromptWidget::undoLandTapRequested);

    auto *actionRow = new QHBoxLayout;
    actionRow->setContentsMargins(0, 0, 0, 0);
    actionRow->setSpacing(4);
    actionRow->addWidget(cancelTargetingButton);
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
    undoLandTapButton->setText(tr("Undo"));
    openingKeepButton->setText(tr("Keep"));
    openingMulliganButton->setText(tr("Mulligan"));
    openingBottomCancelButton->setText(tr("Cancel"));
    openingBottomDoneButton->setText(tr("Done"));
    if (ruledOpeningUiKind == 1 && ruledOpeningPickSeatIds.size() >= 2) {
        openingPickSeatButton1->setText(tr("You"));
        openingPickSeatButton2->setText(tr("Opponent"));
    }
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

void GamePromptWidget::setCombatMode(CombatMode mode, bool localPlayerHasButtons)
{
    if (mode == currentCombatMode && localPlayerHasButtons == localPlayerHasCombatButtons) {
        return;
    }
    currentCombatMode = mode;
    localPlayerHasCombatButtons = localPlayerHasButtons;
    updateCombatButtonsVisibility();
    refreshPromptLabel();
}

void GamePromptWidget::setTargetingMode(bool enabled, const QString &cardName)
{
    targetingModeEnabled = enabled;
    if (enabled) {
        setPromptText(tr("Cast %1 selected. Select a target card, or press Cancel.").arg(cardName));
    }
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

void GamePromptWidget::setRuledOpeningUi(int kind, QVector<int> pickSeatIds, int mulliganCount)
{
    ruledOpeningUiKind = kind;
    ruledOpeningMulliganCount = mulliganCount;
    ruledOpeningPickSeatIds = std::move(pickSeatIds);
    openingPickSeatButton1->disconnect();
    openingPickSeatButton2->disconnect();
    if (kind == 1 && ruledOpeningPickSeatIds.size() >= 2) {
        setPromptText(tr("Choose who goes first."));
        openingPickSeatButton1->setText(tr("You"));
        openingPickSeatButton2->setText(tr("Opponent"));
        const int selfSeatId = ruledOpeningPickSeatIds[0];
        const int opponentSeatId = ruledOpeningPickSeatIds[1];
        QObject::connect(openingPickSeatButton1, &QPushButton::clicked, this, [this, selfSeatId] {
            emit ruledOpeningPickSeatRequested(selfSeatId);
        });
        QObject::connect(openingPickSeatButton2, &QPushButton::clicked, this, [this, opponentSeatId] {
            emit ruledOpeningPickSeatRequested(opponentSeatId);
        });
    }
    if (kind == 2) {
        const int keepCount = 7 - mulliganCount;
        const int mulliganTo = keepCount - 1;
        setPromptText(tr("Mulligan to %1 or keep these %2?").arg(mulliganTo).arg(keepCount));
    }
    if (kind == 3) {
        ruledOpeningBottomSelected = 0;
        setPromptText(tr("Put %1 card(s) to the bottom of your library.").arg(mulliganCount));
    }
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setRuledOpeningBottomProgress(int /*required*/, int selected)
{
    ruledOpeningBottomSelected = selected;
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setCleanupDiscardMode(bool active, int cardsRequired, int cardsSelected)
{
    cleanupDiscardMode = active;
    cleanupCardsRequired = cardsRequired;
    cleanupCardsSelected = cardsSelected;
    if (active && cardsRequired > 0) {
        setPromptText(tr("Cleanup — discard %2 card(s) to reach hand size 7. Selected: %1 of %2. Click hand cards to "
                         "toggle; click again to deselect.")
                          .arg(cardsSelected)
                          .arg(cardsRequired));
    } else if (!active) {
        setPromptText({});
    }
    updateCombatButtonsVisibility();
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
    if (spellCastPending == pending) {
        return;
    }
    spellCastPending = pending;
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setCombatDamageStatus(const QString &attackerName, int assigned, int power, bool legal)
{
    if (attackerName.isEmpty()) {
        confirmCombatDamageButton->setEnabled(false);
        return;
    }
    setPromptText(tr("Assign combat damage for %1\n%2")
                      .arg(attackerName)
                      .arg(tr("Assigned %1 of %2.").arg(assigned).arg(power)));
    confirmCombatDamageButton->setEnabled(legal && assigned == power && power > 0);
}

void GamePromptWidget::updateCombatButtonsVisibility()
{
    if (ruledOpeningUiKind != 0) {
        passPriorityButton->setVisible(false);
        confirmAttackersButton->setVisible(false);
        confirmBlockersButton->setVisible(false);
        resetBlockersButton->setVisible(false);
        confirmCombatDamageButton->setVisible(false);
        cancelTargetingButton->setVisible(false);
        undoLandTapButton->setVisible(false);
        const bool showPick = ruledOpeningUiKind == 1 && !ruledOpeningPickSeatIds.isEmpty();
        openingPickSeatButton1->setVisible(showPick && ruledOpeningPickSeatIds.size() >= 1);
        openingPickSeatButton2->setVisible(showPick && ruledOpeningPickSeatIds.size() >= 2);
        openingKeepButton->setVisible(ruledOpeningUiKind == 2);
        openingMulliganButton->setVisible(ruledOpeningUiKind == 2 && (7 - ruledOpeningMulliganCount) - 1 >= 0);
        const bool isBottom = (ruledOpeningUiKind == 3);
        openingBottomCancelButton->setVisible(isBottom && ruledOpeningBottomSelected >= 1);
        openingBottomDoneButton->setVisible(isBottom && ruledOpeningMulliganCount > 0 &&
                                             ruledOpeningBottomSelected == ruledOpeningMulliganCount);
        return;
    }
    openingPickSeatButton1->hide();
    openingPickSeatButton2->hide();
    openingKeepButton->hide();
    openingMulliganButton->hide();
    openingBottomCancelButton->hide();
    openingBottomDoneButton->hide();
    if (cleanupDiscardMode) {
        passPriorityButton->setVisible(false);
        confirmAttackersButton->setVisible(false);
        confirmBlockersButton->setVisible(false);
        resetBlockersButton->setVisible(false);
        confirmCombatDamageButton->setVisible(false);
        cancelTargetingButton->setVisible(false);
        undoLandTapButton->setVisible(false);
        return;
    }
    if (targetingModeEnabled || spellCastPending) {
        passPriorityButton->setVisible(false);
        confirmAttackersButton->setVisible(false);
        confirmBlockersButton->setVisible(false);
        resetBlockersButton->setVisible(false);
        confirmCombatDamageButton->setVisible(false);
        cancelTargetingButton->setVisible(true);
        undoLandTapButton->setVisible(false);
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
    resetBlockersButton->setVisible(showBlockers);
    confirmCombatDamageButton->setVisible(showCombatDamage);
    cancelTargetingButton->setVisible(false);
    undoLandTapButton->setVisible(localPlayerHasPriority && landTapUndoAvailable && !showAttackers && !showBlockers &&
                                   !showCombatDamage && !waitingOnOpponentCombatDamage);
}

void GamePromptWidget::updatePassPriorityButtonText()
{
    if (ruledStackHasItems) {
        passPriorityButton->setText(tr("No Response"));
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
    if (targetingModeEnabled || spellCastPending || cleanupDiscardMode || ruledOpeningUiKind != 0) {
        return;
    }
    if (currentCombatMode == CombatMode::AssignCombatDamage) {
        return;
    }

    const QString &waitName = priorityPlayerName.isEmpty() ? activePlayerName : priorityPlayerName;

    if (currentCombatMode == CombatMode::DeclareAttackers) {
        if (localPlayerHasCombatButtons) {
            promptLabel->setText(tr("%1's Declare Attackers. Choose attackers.").arg(activePlayerName));
        } else {
            promptLabel->setText(tr("Waiting for %1...").arg(waitName));
        }
        return;
    }
    if (currentCombatMode == CombatMode::DeclareBlockers) {
        if (localPlayerHasCombatButtons) {
            promptLabel->setText(tr("%1's Declare Blockers. Choose blockers.").arg(activePlayerName));
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

    const QString phaseName = currentPhaseDisplayName(currentActivePhase);
    if (phaseName.isEmpty()) {
        return;
    }
    const bool isMyMainPhase = localPlayerIsActive && (currentActivePhase == 3 || currentActivePhase == 9) && !ruledStackHasItems;
    const QString actions = isMyMainPhase
        ? tr("Cast spells, activate abilities, and play land.")
        : tr("Cast instants and activate abilities.");
    promptLabel->setText(tr("%1's %2. %3").arg(activePlayerName, phaseName, actions));
}
