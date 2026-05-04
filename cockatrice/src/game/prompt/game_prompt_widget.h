#ifndef COCKATRICE_GAME_PROMPT_WIDGET_H
#define COCKATRICE_GAME_PROMPT_WIDGET_H

#include <QVector>
#include <QWidget>

class QLabel;
class QPushButton;
class QFrame;
class QHBoxLayout;

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
    void setRuledStackHasItems(bool hasItems);
    void setCleanupDiscardMode(bool active, int cardsRequired, int cardsSelected);
    /// `kind`: 0 none, 1 choose first seat, 2 mulligan choice, 3 bottom cards (hand clicks).
    void setRuledOpeningUi(int kind, QVector<int> pickSeatIds);

signals:
    void passPriorityRequested();
    void confirmAttackersRequested();
    void confirmBlockersRequested();
    void resetBlockersRequested();
    void cancelTargetingRequested();
    void ruledOpeningPickSeatRequested(int seatId);
    void ruledOpeningMulliganKeepRequested();
    void ruledOpeningMulliganRedrawRequested();

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
    bool ruledStackHasItems = false;
    bool cleanupDiscardMode = false;
    int cleanupCardsRequired = 0;
    int cleanupCardsSelected = 0;
    int ruledOpeningUiKind = 0;
    QVector<int> ruledOpeningPickSeatIds;
    QHBoxLayout *openingRowLayout = nullptr;
    QPushButton *openingPickSeatButton1 = nullptr;
    QPushButton *openingPickSeatButton2 = nullptr;
    QPushButton *openingKeepButton = nullptr;
    QPushButton *openingMulliganButton = nullptr;
};

#endif // COCKATRICE_GAME_PROMPT_WIDGET_H
