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
    /// CR 510.4: true while the engine reports a pending first-strike damage substep.
    /// Drives the "First Strike Damage" vs "Combat Damage" pass-priority button label
    /// while the local player is in the declare-blockers (or first-strike-damage) phase.
    void setFirstStrikeStepPending(bool pending);
    /// CR 510.4: true once the engine has entered the first-strike damage substep — the
    /// pass-priority button leads into the regular combat damage step from here, so it
    /// reads "Combat Damage" rather than "End of Combat".
    void setFirstStrikeDamageStepActive(bool active);
    void setCleanupDiscardMode(bool active, int cardsRequired, int cardsSelected);
    /// `kind`: 0 none, 1 choose first seat, 2 mulligan choice, 3 bottom cards (hand clicks).
    void setRuledOpeningUi(int kind, QVector<int> pickSeatIds, int mulliganCount = 0);
    /// Tier-3 hand-pick mode (Brainstorm etc.): required == 0 clears the mode.
    void setResolutionHandPickMode(int required, int selected);
    void setRuledOpeningBottomProgress(int required, int selected);
    void setLandTapUndoAvailable(bool available);
    void setSpellCastPending(bool pending);
    void setTriggerTargetPending(bool pending);
    void setActivatedAbilityTargetPending(bool pending, const QString &abilityText);
    /// Active player only: drives assign-combat-damage title, assigned/power line, and OK enable.
    /// `playerDamage` is the implied trample damage to the defending player (0 for non-trample).
    void setCombatDamageStatus(const QString &attackerName, int assigned, int power, int playerDamage, bool legal);
    void setActivePlayerName(const QString &name);
    void setPriorityPlayerName(const QString &name);
    void setLocalPlayerIsActive(bool isActive);
    void refreshPromptLabel();
    /// Show `msg` persistently in place of the normal "Choose blockers." label until
    /// the player successfully submits legal blocks (or leaves the declare-blockers state).
    void setStickyBlockerError(const QString &msg);
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
    void ruledResolutionHandPickConfirmRequested();

private:
    void updatePassPriorityButtonText();
    void updateCombatButtonsVisibility();

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
    bool triggerTargetPending = false;
    int currentActivePhase = -1;
    bool localPlayerHasPriority = false;
    CombatMode currentCombatMode = CombatMode::None;
    bool localPlayerHasCombatButtons = false;
    bool targetingModeEnabled = false;
    bool activatedAbilityTargetPending = false;
    bool ruledStackHasItems = false;
    bool firstStrikeStepPending = false;
    bool firstStrikeDamageStepActive = false;
    bool cleanupDiscardMode = false;
    QString activePlayerName;
    QString priorityPlayerName;
    bool localPlayerIsActive = false;
    /// Non-empty while the engine has rejected a block declaration (e.g. menace with one blocker).
    /// Shown instead of the normal "Choose blockers." text; cleared when the player successfully
    /// submits legal blocks or leaves the declare-blockers state.
    QString stickyBlockerError;
    int ruledOpeningUiKind = 0;
    int ruledOpeningMulliganCount = 0;
    QVector<int> ruledOpeningPickSeatIds;
    QPushButton *openingPickSeatButton1 = nullptr;
    QPushButton *openingPickSeatButton2 = nullptr;
    QPushButton *openingKeepButton = nullptr;
    QPushButton *openingMulliganButton = nullptr;
    QPushButton *openingBottomCancelButton = nullptr;
    QPushButton *openingBottomDoneButton = nullptr;
    int ruledOpeningBottomSelected = 0;

    // Resolution hand-pick (Brainstorm etc.)
    QPushButton *resolutionHandPickConfirmButton = nullptr;
    int resolutionHandPickRequired = 0;
    int resolutionHandPickSelected = 0;
};

#endif // COCKATRICE_GAME_PROMPT_WIDGET_H
