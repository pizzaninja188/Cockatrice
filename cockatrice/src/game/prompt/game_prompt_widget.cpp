#include "game_prompt_widget.h"

#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
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

void GamePromptWidget::updateCombatButtonsVisibility()
{
    const bool showAttackers =
        localPlayerHasPriority && currentCombatMode == CombatMode::DeclareAttackers && localPlayerHasCombatButtons;
    const bool showBlockers =
        localPlayerHasPriority && currentCombatMode == CombatMode::DeclareBlockers && localPlayerHasCombatButtons;
    passPriorityButton->setVisible(localPlayerHasPriority && !showAttackers && !showBlockers);
    confirmAttackersButton->setVisible(showAttackers);
    confirmBlockersButton->setVisible(showBlockers);
    resetBlockersButton->setVisible(showBlockers);
}

void GamePromptWidget::updatePassPriorityButtonText()
{
    passPriorityButton->setText(nextStepButtonTextForPhase(currentActivePhase));
}
