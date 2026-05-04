#include "game_prompt_widget.h"

#include <QFrame>
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

QString nextStepButtonTextForPhase(int phase)
{
    switch (phase) {
        case 1:
            return GamePromptWidget::tr("Draw Step");
        case 2:
            return GamePromptWidget::tr("First Main Phase");
        case 3:
            return GamePromptWidget::tr("Combat");
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

    promptTitleLabel = new QLabel(this);
    promptTitleLabel->setObjectName("promptTitleLabel");
    promptTitleLabel->setTextFormat(Qt::RichText);
    layout->addWidget(promptTitleLabel);

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
    layout->addLayout(openingRowLayout);
    connect(openingKeepButton, &QPushButton::clicked, this, &GamePromptWidget::ruledOpeningMulliganKeepRequested);
    connect(openingMulliganButton, &QPushButton::clicked, this, &GamePromptWidget::ruledOpeningMulliganRedrawRequested);

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

    cancelTargetingButton = new QPushButton(this);
    cancelTargetingButton->setObjectName("cancelTargetingButton");
    connect(cancelTargetingButton, &QPushButton::clicked, this, &GamePromptWidget::cancelTargetingRequested);
    layout->addWidget(cancelTargetingButton);

    layout->addLayout(combatRow);

    futureActionsLabel = new QLabel(this);
    futureActionsLabel->setObjectName("futureActionsLabel");
    layout->addWidget(futureActionsLabel);

    futureActionsFrame = new QFrame(this);
    futureActionsFrame->setObjectName("futureActionsFrame");
    futureActionsFrame->setFrameShape(QFrame::StyledPanel);
    futureActionsFrame->setMinimumHeight(48);
    layout->addWidget(futureActionsFrame);

    fallbackPromptText = tr("Waiting for ruled action prompt...");
    updateCombatButtonsVisibility();
    retranslateUi();
}

void GamePromptWidget::retranslateUi()
{
    promptTitleLabel->setText(QStringLiteral("<b>%1</b>").arg(tr("Current action")));
    if (promptLabel->text().isEmpty() || promptLabel->text() == fallbackPromptText) {
        fallbackPromptText = tr("Waiting for ruled action prompt...");
        promptLabel->setText(fallbackPromptText);
    }
    updatePassPriorityButtonText();
    confirmAttackersButton->setText(tr("OK"));
    confirmBlockersButton->setText(tr("OK"));
    resetBlockersButton->setText(tr("Reset Blockers"));
    cancelTargetingButton->setText(tr("Cancel"));
    openingKeepButton->setText(tr("Keep hand"));
    openingMulliganButton->setText(tr("Mulligan (redraw to 7)"));
    if (ruledOpeningUiKind == 1 && ruledOpeningPickSeatIds.size() >= 2) {
        openingPickSeatButton1->setText(tr("You"));
        openingPickSeatButton2->setText(tr("Opponent"));
    }
    futureActionsLabel->setText(tr("Future actions"));
    futureActionsFrame->setToolTip(tr("Reserved space for upcoming action buttons (undo land tap, undo mana, etc.)."));
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
}

void GamePromptWidget::setLocalPlayerHasPriority(bool hasPriority)
{
    if (localPlayerHasPriority == hasPriority) {
        return;
    }
    localPlayerHasPriority = hasPriority;
    updateCombatButtonsVisibility();
}

void GamePromptWidget::setCombatMode(CombatMode mode, bool localPlayerHasButtons)
{
    if (mode == currentCombatMode && localPlayerHasButtons == localPlayerHasCombatButtons) {
        return;
    }
    currentCombatMode = mode;
    localPlayerHasCombatButtons = localPlayerHasButtons;
    updateCombatButtonsVisibility();
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
}

void GamePromptWidget::setRuledOpeningUi(int kind, QVector<int> pickSeatIds)
{
    ruledOpeningUiKind = kind;
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
    if (kind == 3) {
        setPromptText(tr("Opening: click a highlighted hand card to put it on the bottom of your deck."));
    }
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

void GamePromptWidget::updateCombatButtonsVisibility()
{
    if (ruledOpeningUiKind != 0) {
        passPriorityButton->setVisible(false);
        confirmAttackersButton->setVisible(false);
        confirmBlockersButton->setVisible(false);
        resetBlockersButton->setVisible(false);
        cancelTargetingButton->setVisible(false);
        const bool showPick = ruledOpeningUiKind == 1 && !ruledOpeningPickSeatIds.isEmpty();
        openingPickSeatButton1->setVisible(showPick && ruledOpeningPickSeatIds.size() >= 1);
        openingPickSeatButton2->setVisible(showPick && ruledOpeningPickSeatIds.size() >= 2);
        openingKeepButton->setVisible(ruledOpeningUiKind == 2);
        openingMulliganButton->setVisible(ruledOpeningUiKind == 2);
        return;
    }
    openingPickSeatButton1->hide();
    openingPickSeatButton2->hide();
    openingKeepButton->hide();
    openingMulliganButton->hide();
    if (cleanupDiscardMode) {
        passPriorityButton->setVisible(false);
        confirmAttackersButton->setVisible(false);
        confirmBlockersButton->setVisible(false);
        resetBlockersButton->setVisible(false);
        cancelTargetingButton->setVisible(false);
        return;
    }
    if (targetingModeEnabled) {
        passPriorityButton->setVisible(false);
        confirmAttackersButton->setVisible(false);
        confirmBlockersButton->setVisible(false);
        resetBlockersButton->setVisible(false);
        cancelTargetingButton->setVisible(true);
        return;
    }

    const bool showAttackers =
        localPlayerHasPriority && currentCombatMode == CombatMode::DeclareAttackers && localPlayerHasCombatButtons;
    const bool showBlockers =
        localPlayerHasPriority && currentCombatMode == CombatMode::DeclareBlockers && localPlayerHasCombatButtons;
    passPriorityButton->setVisible(localPlayerHasPriority && !showAttackers && !showBlockers);
    confirmAttackersButton->setVisible(showAttackers);
    confirmBlockersButton->setVisible(showBlockers);
    resetBlockersButton->setVisible(showBlockers);
    cancelTargetingButton->setVisible(false);
}

void GamePromptWidget::updatePassPriorityButtonText()
{
    if (ruledStackHasItems) {
        passPriorityButton->setText(tr("No Response"));
        return;
    }
    passPriorityButton->setText(nextStepButtonTextForPhase(currentActivePhase));
}
