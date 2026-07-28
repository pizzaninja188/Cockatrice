#include "ruled_dev_console.h"

#include <QEvent>
#include <QHBoxLayout>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QVBoxLayout>

namespace {
/// Latched from --dev-console. File-static rather than a setting: it is a launch-time dev switch,
/// not a preference, and must not survive into a normal session.
bool devConsoleEnabled = false;
} // namespace

void RuledDevConsoleWidget::setEnabledFromCommandLine(bool enabled)
{
    devConsoleEnabled = enabled;
}

bool RuledDevConsoleWidget::isEnabled()
{
    return devConsoleEnabled;
}

RuledDevConsoleWidget::RuledDevConsoleWidget(QWidget *parent) : QWidget(parent)
{
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(2);

    auto *row = new QHBoxLayout;
    row->setContentsMargins(0, 0, 0, 0);
    promptLabel = new QLabel(this);
    promptLabel->setObjectName(QStringLiteral("devConsolePrompt"));
    input = new QLineEdit(this);
    input->setObjectName(QStringLiteral("devConsoleInput"));
    input->installEventFilter(this);
    row->addWidget(promptLabel);
    row->addWidget(input, 1);
    layout->addLayout(row);

    statusLabel = new QLabel(this);
    statusLabel->setObjectName(QStringLiteral("devConsoleStatus"));
    statusLabel->setWordWrap(true);
    statusLabel->hide();
    layout->addWidget(statusLabel);

    connect(input, &QLineEdit::returnPressed, this, &RuledDevConsoleWidget::submit);
    retranslateUi();
}

void RuledDevConsoleWidget::retranslateUi()
{
    promptLabel->setText(tr("dev>"));
    input->setPlaceholderText(tr("put hand Serra Angel · mana 3RR · help"));
}

void RuledDevConsoleWidget::setStatus(const QString &text, bool isError)
{
    if (text.isEmpty()) {
        statusLabel->clear();
        statusLabel->hide();
        return;
    }
    // Colour by role rather than by theme palette: this is a dev affordance, and the distinction
    // that matters is "your command was rejected" vs "here is some text".
    statusLabel->setStyleSheet(isError ? QStringLiteral("color: palette(bright-text);") : QString());
    statusLabel->setText(text);
    statusLabel->show();
}

void RuledDevConsoleWidget::submit()
{
    const QString line = input->text().trimmed();
    if (line.isEmpty()) {
        return;
    }
    // Consecutive duplicates would just pad the history someone is arrowing back through.
    if (history.isEmpty() || history.last() != line) {
        history.append(line);
    }
    historyPos = history.size();
    input->clear();
    setStatus(QString(), false);
    emit commandSubmitted(line);
}

void RuledDevConsoleWidget::recallHistory(int delta)
{
    if (history.isEmpty()) {
        return;
    }
    const int next = qBound(0, historyPos + delta, history.size());
    historyPos = next;
    // Stepping past the newest entry returns to an empty line, the way a shell does.
    input->setText(next == history.size() ? QString() : history.at(next));
}

bool RuledDevConsoleWidget::eventFilter(QObject *watched, QEvent *event)
{
    if (watched == input && event->type() == QEvent::KeyPress) {
        auto *key = static_cast<QKeyEvent *>(event);
        if (key->key() == Qt::Key_Up) {
            recallHistory(-1);
            return true;
        }
        if (key->key() == Qt::Key_Down) {
            recallHistory(1);
            return true;
        }
    }
    return QWidget::eventFilter(watched, event);
}
