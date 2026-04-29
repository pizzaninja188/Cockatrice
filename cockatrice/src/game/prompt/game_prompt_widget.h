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
    void setActivePhase(int phase);
    void setLocalPlayerHasPriority(bool hasPriority);
    void setCombatMode(CombatMode mode, bool localPlayerHasButtons);
    void setTargetingMode(bool enabled, const QString &cardName = {});

signals:
    void passPriorityRequested();
    void confirmAttackersRequested();
    void confirmBlockersRequested();
    void resetBlockersRequested();
    void cancelTargetingRequested();

private:
    void updatePassPriorityButtonText();
    void updateCombatButtonsVisibility();

    QLabel *promptTitleLabel;
    QLabel *promptLabel;
    QPushButton *passPriorityButton;
    QPushButton *confirmAttackersButton;
    QPushButton *confirmBlockersButton;
    QPushButton *resetBlockersButton;
    QPushButton *cancelTargetingButton;
    QLabel *futureActionsLabel;
    QFrame *futureActionsFrame;
    QString fallbackPromptText;
    int currentActivePhase = -1;
    bool localPlayerHasPriority = false;
    CombatMode currentCombatMode = CombatMode::None;
    bool localPlayerHasCombatButtons = false;
    bool targetingModeEnabled = false;
};

#endif // COCKATRICE_GAME_PROMPT_WIDGET_H
