#include "ruled_dev_console.h"

#include <QEvent>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QVBoxLayout>

namespace
{
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

    auto *presetRow = new QHBoxLayout;
    presetLabel = new QLabel(this);
    presetLabel->setWordWrap(true);
    loadButton = new QPushButton(this);
    loadButton->setObjectName(QStringLiteral("devPresetLoad"));
    runButton = new QPushButton(this);
    runButton->setObjectName(QStringLiteral("devPresetRun"));
    cancelButton = new QPushButton(this);
    cancelButton->setObjectName(QStringLiteral("devPresetCancel"));
    presetRow->addWidget(presetLabel, 1);
    presetRow->addWidget(loadButton);
    presetRow->addWidget(runButton);
    presetRow->addWidget(cancelButton);
    layout->addLayout(presetRow);
    preview = new QPlainTextEdit(this);
    preview->setObjectName(QStringLiteral("devPresetPreview"));
    preview->setReadOnly(true);
    preview->setMaximumHeight(90);
    preview->hide();
    layout->addWidget(preview);
    connect(loadButton, &QPushButton::clicked, this, [this] {
        const QString path = QFileDialog::getOpenFileName(this, tr("Load prepared-state preset"), QString(),
                                                          tr("Command files (*.txt);;All files (*)"));
        if (!path.isEmpty())
            loadPreset(path);
    });
    connect(runButton, &QPushButton::clicked, this, [this] {
        if (busy || presetName.isEmpty())
            return;
        setBusy(true);
        emit presetSubmitted(presetName, preview->toPlainText());
    });
    connect(cancelButton, &QPushButton::clicked, this, &RuledDevConsoleWidget::cancelRequested);

    statusLabel = new QLabel(this);
    statusLabel->setObjectName(QStringLiteral("devConsoleStatus"));
    statusLabel->setWordWrap(true);
    statusLabel->hide();
    layout->addWidget(statusLabel);

    connect(input, &QLineEdit::returnPressed, this, &RuledDevConsoleWidget::submit);
    retranslateUi();
    setBusy(false);
}

void RuledDevConsoleWidget::retranslateUi()
{
    promptLabel->setText(tr("dev>"));
    input->setPlaceholderText(tr("put hand Serra Angel · move gy Serra Angel · mana 3RR · help"));
    presetLabel->setText(tr("Prepared state: reruns add copies; no reset or rollback."));
    loadButton->setText(tr("Load preset…"));
    runButton->setText(tr("Run (adds state)"));
    cancelButton->setText(tr("Cancel remaining"));
}

void RuledDevConsoleWidget::setBusy(bool value)
{
    busy = value;
    input->setEnabled(!busy);
    loadButton->setEnabled(!busy);
    runButton->setEnabled(!busy && !presetName.isEmpty());
    cancelButton->setEnabled(busy);
}

bool RuledDevConsoleWidget::loadPreset(const QString &path)
{
    if (busy)
        return false;
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly) || file.size() > 65536) {
        setStatus(tr("Cannot load preset: use a readable UTF-8 text file of at most 64 KiB."), true);
        return false;
    }
    const QByteArray bytes = file.read(65537);
    QByteArray content = bytes;
    if (content.startsWith(QByteArray::fromHex("efbbbf")))
        content.remove(0, 3);
    const QString text = QString::fromUtf8(content);
    if (file.error() != QFileDevice::NoError || bytes.size() > 65536 || text.toUtf8() != content ||
        text.contains(QChar::Null)) {
        setStatus(tr("Cannot load preset: invalid UTF-8 text or file read error."), true);
        return false;
    }
    presetName = QFileInfo(path).completeBaseName();
    preview->setPlainText(text);
    preview->show();
    preview->setToolTip(path);
    setBusy(false);
    setStatus(tr("Loaded %1. Review the commands, then Run. Nothing sent yet.").arg(presetName), false);
    return true;
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
    if (busy || line.isEmpty()) {
        return;
    }
    // Consecutive duplicates would just pad the history someone is arrowing back through.
    if (history.isEmpty() || history.last() != line) {
        history.append(line);
    }
    historyPos = history.size();
    input->clear();
    setStatus(QString(), false);
    setBusy(true);
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
