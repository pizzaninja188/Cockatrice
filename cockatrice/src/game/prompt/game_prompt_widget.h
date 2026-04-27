#ifndef COCKATRICE_GAME_PROMPT_WIDGET_H
#define COCKATRICE_GAME_PROMPT_WIDGET_H

#include <QWidget>

class QLabel;
class QPushButton;
class QFrame;

class GamePromptWidget : public QWidget
{
    Q_OBJECT
public:
    explicit GamePromptWidget(QWidget *parent = nullptr);
    void retranslateUi();

public slots:
    void setPromptText(const QString &promptText);
    void setPromptFromRuledLog(const QString &ruledLog);
    void setPassPriorityEnabled(bool enabled);

signals:
    void passPriorityRequested();

private:
    QLabel *promptTitleLabel;
    QLabel *promptLabel;
    QPushButton *passPriorityButton;
    QLabel *futureActionsLabel;
    QFrame *futureActionsFrame;
    QString fallbackPromptText;
};

#endif // COCKATRICE_GAME_PROMPT_WIDGET_H
