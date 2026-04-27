#include "game_prompt_widget.h"

#include <QFrame>
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

    futureActionsLabel = new QLabel(this);
    futureActionsLabel->setObjectName("futureActionsLabel");
    layout->addWidget(futureActionsLabel);

    futureActionsFrame = new QFrame(this);
    futureActionsFrame->setObjectName("futureActionsFrame");
    futureActionsFrame->setFrameShape(QFrame::StyledPanel);
    futureActionsFrame->setMinimumHeight(48);
    layout->addWidget(futureActionsFrame);

    fallbackPromptText = tr("Waiting for ruled action prompt...");
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
