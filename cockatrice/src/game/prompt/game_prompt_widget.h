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
    enum class CombatMode
    {
        None,
        DeclareAttackers,
        DeclareBlockers
    };

    explicit GamePromptWidget(QWidget *parent = nullptr);
    void retranslateUi();

public slots:
    void setPromptText(const QString &promptText);
    void setPromptFromRuledLog(const QString &ruledLog);
    void setPassPriorityEnabled(bool enabled);
    void setCombatMode(CombatMode mode, bool localPlayerHasButtons);

signals:
    void passPriorityRequested();
    void confirmAttackersRequested();
    void skipAttackersRequested();
    void confirmBlockersRequested();
    void skipBlockersRequested();

private:
    void updateCombatButtonsVisibility();

    QLabel *promptTitleLabel;
    QLabel *promptLabel;
    QPushButton *passPriorityButton;
    QPushButton *confirmAttackersButton;
    QPushButton *skipAttackersButton;
    QPushButton *confirmBlockersButton;
    QPushButton *skipBlockersButton;
    QLabel *futureActionsLabel;
    QFrame *futureActionsFrame;
    QString fallbackPromptText;
    CombatMode currentCombatMode = CombatMode::None;
    bool localPlayerHasCombatButtons = false;
};

#endif // COCKATRICE_GAME_PROMPT_WIDGET_H
