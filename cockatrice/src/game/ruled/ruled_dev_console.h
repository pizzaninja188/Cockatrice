/**
 * @file ruled_dev_console.h
 * @ingroup Ruled
 * @brief Dev-loop console: a one-line command entry for the debug cheat commands.
 *
 * Fork-owned. Sits in the existing Messages dock, directly under the ruled prompt widget — the
 * same placement pattern `GamePromptWidget` uses, so no new dock is involved.
 *
 * Like `GamePromptWidget`, the widget never sends anything: it emits `commandSubmitted` and
 * `TabGame` does the parse and the send. That keeps the transport in one place and the widget
 * testable offscreen.
 *
 * Off unless `--dev-console` was passed, and only ever built for a ruled, non-replay game. That
 * gating is cosmetic convenience, not security: the real gate is engine-side (see `engine::dev`),
 * because a client is never trusted.
 */

#ifndef RULED_DEV_CONSOLE_H
#define RULED_DEV_CONSOLE_H

#include <QStringList>
#include <QWidget>

class QLabel;
class QLineEdit;

class RuledDevConsoleWidget : public QWidget
{
    Q_OBJECT
public:
    explicit RuledDevConsoleWidget(QWidget *parent = nullptr);

    /// Latched once from `--dev-console` in main.cpp. A normal launch never builds the widget.
    static void setEnabledFromCommandLine(bool enabled);
    [[nodiscard]] static bool isEnabled();

    /// Show feedback under the input: a parse error, or the help text.
    void setStatus(const QString &text, bool isError);
    void retranslateUi();

signals:
    /// A line the user submitted, verbatim. Interpretation belongs to the caller.
    void commandSubmitted(const QString &line);

protected:
    /// Intercepts Up/Down on the input for history. An event filter rather than a QLineEdit
    /// subclass keeps this one class, which is all the behaviour warrants.
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    void submit();
    void recallHistory(int delta);

    QLabel *promptLabel = nullptr;
    QLineEdit *input = nullptr;
    QLabel *statusLabel = nullptr;

    QStringList history;
    /// Index into `history` while browsing; == history.size() means "on the fresh line".
    int historyPos = 0;
};

#endif // RULED_DEV_CONSOLE_H
