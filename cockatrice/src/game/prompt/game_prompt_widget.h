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

    /// The panel's mutually-exclusive prompt modes, replacing the per-mechanic bool family that
    /// used to be re-derived by hand in both the button-visibility and the label code.
    /// `effectiveMode()` resolves the one priority chain; both of those switch on its result.
    /// A new prompt mechanic is a value here plus a case in each switch.
    enum class PromptMode
    {
        /// Priority / combat layout: the pass button, the combat declare buttons, undo.
        Normal,
        /// A ruled gameplay command is in flight. Keep the existing prompt text during the short
        /// no-indicator window, but suppress every gameplay control immediately.
        CommandPending,
        /// The same input lock after its 150 ms threshold; owns the waiting status line.
        UpdatingGame,
        /// A cast or activated ability is collecting X, mana or targets. Never pushed by the
        /// caller — derived from the targeting sources, which arrive on independent signals.
        Targeting,
        /// The engine parked a click-a-permanent choice on us (trigger target, copy retarget,
        /// legend keep). Rendered from `RuledPromptState::text`.
        ClickChoice,
        /// CR 514.1: discard down to hand size.
        CleanupDiscard,
        /// Tier-3 resolution pick over cards in a zone (Brainstorm, Gifts Ungiven, …).
        ResolutionPick,
        /// CR 603.3b: ordering this player's simultaneous triggers. The picking happens in the
        /// dedicated ordering window, so this mode only supplies the prompt line and suppresses
        /// the priority/combat buttons the engine is refusing anyway.
        TriggerOrder,
        OpeningChooseFirst,
        OpeningMulligan,
        OpeningBottom,
    };

    /// The exclusive prompt mode plus its payload, pushed as one unit.
    struct RuledPromptState
    {
        PromptMode mode = PromptMode::Normal;
        /// Cards required / selected — CleanupDiscard, OpeningBottom, ResolutionPick. For the
        /// opening modes `required` is the mulligan depth.
        int required = 0;
        int selected = 0;
        /// Caller-supplied prompt line. Required for ClickChoice / ResolutionPick (the engine
        /// wrote it); on Normal it overrides the composed phase/priority line; the other modes
        /// compose their own translated text and ignore it.
        QString text;
        /// OpeningChooseFirst only: [local seat id, opponent seat id].
        QVector<int> openingPickSeatIds;
        /// ClickChoice only: the engine permits an empty answer, so the prompt offers Decline.
        /// New fields go at the *end* — callers and tests use positional aggregate init, so
        /// inserting in the middle silently rebinds their arguments (or fails to compile).
        bool canDecline = false;
    };

    /// Independent async inputs that all mean "mid-cast / mid-activation" and OR into
    /// PromptMode::Targeting. They stay separate from RuledPromptState because three different
    /// PlayerActions signals raise and drop them without knowing about each other.
    enum TargetingSource
    {
        SpellTargetSelection = 0x1, ///< ruledSpellTargetingChanged — picking a spell's targets
        SpellCastPending = 0x2,     ///< ruledSpellCastPendingChanged — choosing X / paying mana
        AbilityTargetPending = 0x4, ///< an activated ability is waiting for its target
    };
    Q_DECLARE_FLAGS(TargetingSources, TargetingSource)

    explicit GamePromptWidget(QWidget *parent = nullptr);
    void retranslateUi();

    /// The mode actually in force once the targeting sources are folded in.
    [[nodiscard]] PromptMode effectiveMode() const;

public slots:
    /// The single entry point for the exclusive prompt modes.
    void setRuledPromptState(RuledPromptState newState);
    void setPromptText(const QString &promptText);
    void setPromptFromRuledLog(const QString &ruledLog);
    void setPassPriorityEnabled(bool enabled);
    void setActivePhase(int phase);
    void setLocalPlayerHasPriority(bool hasPriority);
    /// `declarationSatisfied` (CR 508.1d / 509.1c): when false, the local player still has a
    /// must-attack / must-block creature that isn't staged, so the confirm (OK) button is disabled
    /// to prevent submitting an illegal declaration that the engine would reject (softlock).
    void setCombatMode(CombatMode mode, bool localPlayerHasButtons, bool declarationSatisfied = true);
    void setTargetingMode(bool enabled, const QString &effectText = {});
    void setRuledStackHasItems(bool hasItems);
    /// CR 510.4: true while the engine reports a pending first-strike damage substep.
    /// Drives the "First Strike Damage" vs "Combat Damage" pass-priority button label
    /// while the local player is in the declare-blockers (or first-strike-damage) phase.
    void setFirstStrikeStepPending(bool pending);
    /// CR 510.4: true once the engine has entered the first-strike damage substep — the
    /// pass-priority button leads into the regular combat damage step from here, so it
    /// reads "Combat Damage" rather than "End of Combat".
    void setFirstStrikeDamageStepActive(bool active);
    void setMultiTargetSelectionCount(int selected, int maxTargets);
    /// Spell damage allocation mode (multi-target DamageTargets). `active` false clears the mode.
    void setSpellDamageAllocationStatus(bool active, int assigned, int total);
    void setLandTapUndoAvailable(bool available);
    // Targeting sources — wired straight to the PlayerActions signals that raise/drop them.
    void setSpellCastPending(bool pending);
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
    void confirmSpellDamageRequested();
    void cancelTargetingRequested();
    void declineClickChoiceRequested();
    void confirmTargetsRequested();
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
    /// Write the label the current mode owns (composed here, or the caller's `text`).
    void applyPromptStateText();
    /// Raise or drop one targeting source and re-render.
    void setTargetingSource(TargetingSource source, bool active);
    /// Hide every priority / combat / targeting control — what the take-over modes all do.
    void hideActionAndCombatButtons();

    QLabel *promptLabel;
    QPushButton *passPriorityButton;
    QPushButton *confirmAttackersButton;
    QPushButton *confirmBlockersButton;
    QPushButton *resetBlockersButton;
    QPushButton *confirmCombatDamageButton;
    QPushButton *confirmSpellDamageButton = nullptr;
    QPushButton *cancelTargetingButton;
    QPushButton *declineClickChoiceButton = nullptr;
    QPushButton *confirmTargetsButton = nullptr;
    QPushButton *undoLandTapButton;
    /// The exclusive mode + payload, and the OR-set that derives PromptMode::Targeting.
    /// Between them these two replace the thirteen per-mechanic members this widget used to keep.
    RuledPromptState promptState;
    TargetingSources targetingSources;
    int multiTargetSelectedCount = 0;
    int multiTargetMaxCount = -1;
    QString fallbackPromptText;
    bool landTapUndoAvailable = false;
    int currentActivePhase = -1;
    bool localPlayerHasPriority = false;
    CombatMode currentCombatMode = CombatMode::None;
    bool localPlayerHasCombatButtons = false;
    /// CR 508.1d / 509.1c: false while a required attacker/blocker is still unstaged; disables OK.
    bool combatDeclarationSatisfied = true;
    /// A sub-state of PromptMode::Targeting: allocating a multi-target spell's damage.
    bool spellDamageAllocationMode = false;
    bool ruledStackHasItems = false;
    bool firstStrikeStepPending = false;
    bool firstStrikeDamageStepActive = false;
    QString activePlayerName;
    QString priorityPlayerName;
    bool localPlayerIsActive = false;
    /// Non-empty while the engine has rejected a block declaration (e.g. menace with one blocker).
    /// Shown instead of the normal "Choose blockers." text; cleared when the player successfully
    /// submits legal blocks or leaves the declare-blockers state.
    QString stickyBlockerError;
    QPushButton *openingPickSeatButton1 = nullptr;
    QPushButton *openingPickSeatButton2 = nullptr;
    QPushButton *openingKeepButton = nullptr;
    QPushButton *openingMulliganButton = nullptr;
    QPushButton *openingBottomCancelButton = nullptr;
    QPushButton *openingBottomDoneButton = nullptr;
    QPushButton *resolutionHandPickConfirmButton = nullptr;
};

Q_DECLARE_OPERATORS_FOR_FLAGS(GamePromptWidget::TargetingSources)

#endif // COCKATRICE_GAME_PROMPT_WIDGET_H
