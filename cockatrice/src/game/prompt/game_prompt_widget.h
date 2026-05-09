#ifndef COCKATRICE_GAME_PROMPT_WIDGET_H
#define COCKATRICE_GAME_PROMPT_WIDGET_H

#include <QVector>
#include <QWidget>

class QLabel;
class QPushButton;
class QHBoxLayout;

class GamePromptWidget : public QWidget
{
    Q_OBJECT
public:
    enum class CombatMode
    {
        None,
        DeclareAttackers,
        DeclareBlockers,
        AssignCombatDamage
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
    void setRuledOpeningUi(int kind, QVector<int> pickSeatIds, int mulliganCount = 0);
    void setRuledOpeningBottomProgress(int required, int selected);
    void setLandTapUndoAvailable(bool available);
    void setSpellCastPending(bool pending);
    /// Active player only: drives assign-combat-damage title, assigned/power line, and OK enable.
    void setCombatDamageStatus(const QString &attackerName, int assigned, int power, bool legal);
    void setActivePlayerName(const QString &name);
    void setPriorityPlayerName(const QString &name);
    void setLocalPlayerIsActive(bool isActive);
    void refreshPromptLabel();
    [[nodiscard]] QString getActivePlayerName() const { return activePlayerName; }
    /// True only when the local player must press a combat declare button (not just pass priority).
    bool localPlayerMustDeclareCombat() const
    {
        return (currentCombatMode == CombatMode::DeclareAttackers ||
                currentCombatMode == CombatMode::DeclareBlockers) &&
               localPlayerHasCombatButtons;
    }

signals:
    void passPriorityRequested();
    void confirmAttackersRequested();
    void confirmBlockersRequested();
    void resetBlockersRequested();
    void confirmCombatDamageRequested();
    void cancelTargetingRequested();
    void ruledOpeningPickSeatRequested(int seatId);
    void ruledOpeningMulliganKeepRequested();
    void ruledOpeningMulliganRedrawRequested();
    void ruledOpeningBottomCancelRequested();
    void ruledOpeningBottomDoneRequested();
    void undoLandTapRequested();

private:
    void updatePassPriorityButtonText();
    void updateCombatButtonsVisibility();

    QLabel *promptTitleLabel;
    QLabel *promptLabel;
    QPushButton *passPriorityButton;
    QPushButton *confirmAttackersButton;
    QPushButton *confirmBlockersButton;
    QPushButton *resetBlockersButton;
    QPushButton *confirmCombatDamageButton;
    QPushButton *cancelTargetingButton;
    QPushButton *undoLandTapButton;
    QString fallbackPromptText;
    bool landTapUndoAvailable = false;
    bool spellCastPending = false;
    int currentActivePhase = -1;
    bool localPlayerHasPriority = false;
    CombatMode currentCombatMode = CombatMode::None;
    bool localPlayerHasCombatButtons = false;
    bool targetingModeEnabled = false;
    bool ruledStackHasItems = false;
    bool cleanupDiscardMode = false;
    int cleanupCardsRequired = 0;
    int cleanupCardsSelected = 0;
    QString activePlayerName;
    QString priorityPlayerName;
    bool localPlayerIsActive = false;
    int ruledOpeningUiKind = 0;
    int ruledOpeningMulliganCount = 0;
    QVector<int> ruledOpeningPickSeatIds;
    QHBoxLayout *openingRowLayout = nullptr;
    QPushButton *openingPickSeatButton1 = nullptr;
    QPushButton *openingPickSeatButton2 = nullptr;
    QPushButton *openingKeepButton = nullptr;
    QPushButton *openingMulliganButton = nullptr;
    QPushButton *openingBottomCancelButton = nullptr;
    QPushButton *openingBottomDoneButton = nullptr;
    int ruledOpeningBottomSelected = 0;
};

#endif // COCKATRICE_GAME_PROMPT_WIDGET_H
