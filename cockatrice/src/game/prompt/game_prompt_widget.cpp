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

    skipAttackersButton = new QPushButton(this);
    skipAttackersButton->setObjectName("skipAttackersButton");
    connect(skipAttackersButton, &QPushButton::clicked, this, &GamePromptWidget::skipAttackersRequested);
    combatRow->addWidget(skipAttackersButton);

    confirmBlockersButton = new QPushButton(this);
    confirmBlockersButton->setObjectName("confirmBlockersButton");
    connect(confirmBlockersButton, &QPushButton::clicked, this, &GamePromptWidget::confirmBlockersRequested);
    combatRow->addWidget(confirmBlockersButton);

    skipBlockersButton = new QPushButton(this);
    skipBlockersButton->setObjectName("skipBlockersButton");
    connect(skipBlockersButton, &QPushButton::clicked, this, &GamePromptWidget::skipBlockersRequested);
    combatRow->addWidget(skipBlockersButton);

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
    passPriorityButton->setText(tr("Pass Priority"));
    confirmAttackersButton->setText(tr("Confirm Attackers"));
    skipAttackersButton->setText(tr("No Attackers"));
    confirmBlockersButton->setText(tr("Confirm Blockers"));
    skipBlockersButton->setText(tr("No Blockers"));
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
        currentCombatMode == CombatMode::DeclareAttackers && localPlayerHasCombatButtons;
    const bool showBlockers =
        currentCombatMode == CombatMode::DeclareBlockers && localPlayerHasCombatButtons;
    confirmAttackersButton->setVisible(showAttackers);
    skipAttackersButton->setVisible(showAttackers);
    confirmBlockersButton->setVisible(showBlockers);
    skipBlockersButton->setVisible(showBlockers);
}
