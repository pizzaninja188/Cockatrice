/**
 * @file player_actions.h
 *  @ingroup GameLogicActions
 * @ingroup GameLogicPlayers
 * @brief TODO: Document this.
 */

#ifndef COCKATRICE_PLAYER_ACTIONS_H
#define COCKATRICE_PLAYER_ACTIONS_H
#include "../dialogs/dlg_create_token.h"
#include "../dialogs/dlg_move_top_cards_until.h"
#include "event_processing_options.h"
#include "player.h"

#include <QMenu>
#include <QObject>
#include <QMap>
#include <QPair>
#include <QVector>
#include <libcockatrice/card/relation/card_relation_type.h>
#include <libcockatrice/filters/filter_string.h>
#include <libcockatrice/protocol/pb/card_attributes.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>

namespace google
{
namespace protobuf
{
class Message;
}
} // namespace google

class CardItem;
class Command_MoveCard;
class GameEventContext;
class PendingCommand;
class Player;
class PlayerActions : public QObject
{
    Q_OBJECT

signals:
    void logSetTapped(Player *player, CardItem *card, bool tapped);
    void logSetAnnotation(Player *player, CardItem *card, QString newAnnotation);
    void logSetDoesntUntap(Player *player, CardItem *card, bool doesntUntap);
    void logSetPT(Player *player, CardItem *card, QString newPT);
    void ruledSpellTargetingChanged(bool active, const QString &cardName);
    void landTapUndoAvailableChanged(bool available);
    void ruledSpellCastPendingChanged(bool pending);
    /// Emitted when `remainingCost` changes during ruled spell payment (land or counter).
    void ruledSpellManaPromptChanged();
    /// Emitted when an activated ability enters or leaves the mana-payment waiting state.
    void ruledAbilityActivationPendingChanged(bool pending);
    /// Emitted when `remainingCost` changes during ability mana payment (land or counter).
    void ruledAbilityManaPromptChanged();
    /// Emitted when an activated ability enters or leaves the target-selection waiting state.
    void ruledActivatedAbilityTargetPendingChanged(bool pending, QString abilityText);

public:
    enum CardsToReveal
    {
        RANDOM_CARD_FROM_ZONE = -2
    };

    explicit PlayerActions(Player *player);

    void sendGameCommand(PendingCommand *pend);
    void sendGameCommand(const google::protobuf::Message &command);

    PendingCommand *prepareGameCommand(const ::google::protobuf::Message &cmd);
    PendingCommand *prepareGameCommand(const QList<const ::google::protobuf::Message *> &cmdList);

    void setCardAttrHelper(const GameEventContext &context,
                           CardItem *card,
                           CardAttribute attribute,
                           const QString &avalue,
                           bool allCards,
                           EventProcessingOptions options);

    void moveOneCardUntil(CardItem *card);
    void stopMoveTopCardsUntil();
    bool tryPayRuledSpellWithCounter(const QString &counterName);
    /// CR 605: activate this land's mana ability so the engine produces mana and taps it. When the
    /// ability has multiple options (a dual land), `desiredColor` (e.g. 'u') selects the matching
    /// one. Caller owns the pointer; nullptr if the card is not a mana source in this ruled game.
    [[nodiscard]] Command_RuledPayload *newRuledPayloadActivateManaAbilityForLand(CardItem *card, QChar desiredColor);
    bool tryHandleRuledSpellTargetClick(CardItem *card);
    bool tryHandleRuledSpellTargetPlayerClick(Player *targetPlayer);
    /// True when the local player must pick a player (not permanent) for the pending ruled cast.
    [[nodiscard]] bool isAwaitingRuledPlayerTargetSelection() const;
    /// True when an activated ability or triggered ability is waiting for a target (player click allowed).
    [[nodiscard]] bool isAwaitingRuledAbilityOrTriggerPlayerTarget() const;
    void cancelPendingRuledSpellCast();
    /// Returns the mana-payment prompt text if a spell is pending and still needs mana, otherwise empty.
    [[nodiscard]] QString pendingRuledSpellPromptText() const;
    /// Activated abilities on a battlefield permanent. With `leftClick` true, a permanent whose sole
    /// ability is a mana ability (CR 605) is activated directly (the mana floats, no menu); every
    /// other case (multiple abilities, or a non-mana ability) opens the activation menu. Right-click
    /// passes `leftClick` false to always open the menu. Returns true if the click was consumed.
    bool tryRuledActivateAbilityMenu(CardItem *card, bool leftClick);
    /// Show a side-picker menu ("Cast Fire" / "Cast Ice") for a multi-face card in hand and begin the
    /// chosen face's cast. Returns true if the menu was shown (click consumed); false for single-face
    /// cards or when no face is currently castable.
    bool tryRuledSpellCastFaceMenu(CardItem *card);
    /// Handle a target click for a pending activated ability activation or trigger target selection.
    bool tryHandleRuledAbilityTargetClick(CardItem *card);
    bool tryHandleRuledAbilityTargetPlayerClick(Player *targetPlayer);
    /// Click a pool mana counter to pay toward a pending activated ability. Returns true if consumed.
    bool tryPayRuledAbilityWithCounter(const QString &counterName);
    void cancelPendingActivatedAbility();
    /// Returns the mana-payment prompt text if an ability is pending and still needs mana, otherwise empty.
    [[nodiscard]] QString pendingRuledAbilityPromptText() const;
    bool tryToggleRuledCleanupDiscard(CardItem *card);
    bool tryRuledOpeningBottomCard(CardItem *card);
    bool sendRuledCleanupDiscardBatchIfComplete();

    void recordLandTapUndo(int cardId, const QString &counterName, int counterId);
    void undoLastLandTap();
    void clearLandTapUndoStack();
    [[nodiscard]] bool hasLandTapUndoEntries() const { return !landTapUndoStack.isEmpty(); }

    [[nodiscard]] bool isMovingCardsUntil() const
    {
        return movingCardsUntil;
    }

public slots:
    void setLastToken(CardInfoPtr cardInfo);
    void playCard(CardItem *c, bool faceDown);
    void playCardToTable(const CardItem *c, bool faceDown);

    void actUntapAll();
    void actRollDie();
    void actCreateToken();
    void actCreateAnotherToken();
    void actShuffle();
    void actShuffleTop();
    void actShuffleBottom();
    void actDrawCard();
    void actDrawCards();
    void actUndoDraw();
    void actMulligan();
    void actMulliganSameSize();
    void actMulliganMinusOne();
    void doMulligan(int number);

    void actPlay();
    void actPlayFacedown();
    void actHide();

    void actMoveTopCardToPlay();
    void actMoveTopCardToPlayFaceDown();
    void actMoveTopCardToGrave();
    void actMoveTopCardToExile();
    void actMoveTopCardsToGrave();
    void actMoveTopCardsToGraveFaceDown();
    void actMoveTopCardsToExile();
    void actMoveTopCardsToExileFaceDown();
    void actMoveTopCardsUntil();
    void actMoveTopCardToBottom();
    void actDrawBottomCard();
    void actDrawBottomCards();
    void actMoveBottomCardToPlay();
    void actMoveBottomCardToPlayFaceDown();
    void actMoveBottomCardToGrave();
    void actMoveBottomCardToExile();
    void actMoveBottomCardsToGrave();
    void actMoveBottomCardsToGraveFaceDown();
    void actMoveBottomCardsToExile();
    void actMoveBottomCardsToExileFaceDown();
    void actMoveBottomCardToTop();

    void actSelectAll();
    void actSelectRow();
    void actSelectColumn();

    void actViewLibrary();
    void actViewHand();
    void actViewTopCards();
    void actViewBottomCards();
    void actAlwaysRevealTopCard();
    void actAlwaysLookAtTopCard();
    void actViewGraveyard();
    void actLendLibrary(int lendToPlayerId);
    void actRevealTopCards(int revealToPlayerId, int amount);
    void actRevealRandomGraveyardCard(int revealToPlayerId);
    void actViewRfg();
    void actViewSideboard();

    void actSayMessage();

    void actOpenDeckInDeckEditor();
    void actCreatePredefinedToken();
    void actCreateRelatedCard();
    void actCreateAllRelatedCards();

    void actMoveCardXCardsFromTop();
    void actCardCounterTrigger();
    void actAttach();
    void actUnattach();
    void actDrawArrow();
    void actIncPT(int deltaP, int deltaT);
    void actResetPT();
    void actSetPT();
    void actIncP();
    void actDecP();
    void actIncT();
    void actDecT();
    void actIncPT();
    void actDecPT();
    void actFlowP();
    void actFlowT();
    void actSetAnnotation();
    void actReveal(QAction *action);
    void actRevealHand(int revealToPlayerId);
    void actRevealRandomHandCard(int revealToPlayerId);
    void actRevealLibrary(int revealToPlayerId);

    void actSortHand();

    void cardMenuAction();

    /// Ruled mode: the engine's count of this player's currently-undoable mana abilities
    /// (LegalActions.undoable_mana_abilities). Drives the Undo affordance; re-emits
    /// landTapUndoAvailableChanged so the prompt reflects the authoritative state.
    void setRuledUndoableManaCount(int count);

private:
    // A flexible mana pip (CR 107.4d–f) parsed from a Scryfall brace cost, with its ordinal
    // position so the engine can match the player's payment choice to the right pip.
    struct RuledFlexPip
    {
        quint32 pipIndex = 0; // position among all pips in the cost ({G/U} in "{1}{G/U}" is index 1)
        QChar colorA;         // first/only color letter (W/U/B/R/G)
        QChar colorB;         // hybrid second color; null otherwise
        int generic = 0;      // mono-hybrid generic alternative N ({2/W} -> 2); 0 if not mono-hybrid
        bool phyrexian = false; // Phyrexian {C/P}: payable with the color or 2 life
        // Mono-hybrid only: how many of `generic` have been paid so far with off-color/colorless
        // mana. The pip is satisfied once it reaches `generic` (or earlier via the color).
        int genericPaid = 0;
    };

    struct PendingActivatedAbility
    {
        bool valid = false;
        quint32 permanentOid = 0;
        int abilityIndex = -1;
        QString abilityText;
        QString cardName;
        bool needsTarget = false;
        bool waitingForTarget = false;
        quint32 selectedTargetOid = 0;
        bool waitingForMana = false;
        QMap<QChar, int> remainingCost;
        // CR 107.4d–f: unresolved flexible pips (hybrid/mono-hybrid/Phyrexian) in the ability
        // cost. Resolved as mana is tapped (hybrid/mono) or via a self-portrait click (Phyrexian
        // life), mirroring the spell-cast flow — no upfront prompt.
        QVector<RuledFlexPip> flexPips;
        // CR 107.4f: pip indices the player chose to pay with 2 life. Sent as FlexPipPayment.
        QVector<quint32> lifePipIndices;
    };

    struct PendingRuledSpellCast
    {
        int handIndex = -1;
        // CR 709/712/715: which face of a multi-face card is being cast (split half / MDFC face).
        // 0 (front/primary) for single-face cards; sent as CastSpell.face_index.
        int faceIndex = 0;
        QString cardName;
        QMap<QChar, int> remainingCost;
        QVector<quint32> selectedTargetOids;
        bool waitingForTarget = false;
        bool valid = false;
        // CR 107.3: value chosen for X when the cost has an X pip; 0 otherwise. The number of
        // X pips in the cost (folded into remainingCost's generic bucket at parse time); 0 when
        // the cost has no X. Used to defer the X prompt until after targeting.
        int xPips = 0;
        // Value chosen for X, sent on the CastSpell command. Chosen after targets but before
        // paying mana (CR 601.2b orders X before costs; we prompt for targets first for UX).
        int xValue = 0;
        // CR 107.4d–f: unresolved flexible pips (hybrid/mono-hybrid/Phyrexian). Resolved as mana
        // is tapped (hybrid/mono) or via a self-portrait click (Phyrexian life) — no upfront prompt.
        QVector<RuledFlexPip> flexPips;
        // CR 107.4f: pip indices (into the full mana cost) the player chose to pay with life
        // for Phyrexian pips. Sent as FlexPipPayment{pay_life} on the CastSpell command.
        QVector<quint32> lifePipIndices;
    };

    struct LandTapUndoEntry
    {
        int cardId;
        QString counterName;
        int counterId;
    };

    Player *player;
    bool tryPlayRuledLand(CardItem *card);
    bool tryStartRuledSpellCast(CardItem *card);
    // Set up and begin a pending ruled cast for an already-resolved hand slot + face. faceIndex selects
    // the split/MDFC half (0 for single-face); castName/castCost are that face's name and mana cost.
    // Handles the toggle-cancel, {X}, hybrid/Phyrexian pip, target and mana-payment flow.
    bool beginRuledSpellCast(CardItem *card, int ruledHandIndex, int faceIndex, const QString &castName,
                             const QString &castCost);
    static QMap<QChar, int> parseSimpleManaCost(const QString &manaCost);
    static QVector<RuledFlexPip> parseFlexPips(const QString &manaCost);
    static QString formatSimpleManaCost(const QMap<QChar, int> &cost);
    // Render the still-unpaid cost, fixed pips plus any flexible pips ({G/U}, {2/W}, {B/P}).
    static QString formatRemainingCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex);
    // Total pips still owed (fixed + flexible). Zero means the cost is fully paid.
    static int totalRemainingForCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex);
    // True if `color` can satisfy `pip`'s colored alternative (either side of a hybrid, the
    // single color of a mono-hybrid/Phyrexian pip).
    static bool flexPipMatchesColor(const RuledFlexPip &pip, QChar color);
    // CR 107.4d–f: front-load the flexible-pip choice. Shows a modal dialog displaying the full
    // cost plus one dropdown per flexible pip (hybrid {G/U} → either color; mono-hybrid {2/W} →
    // the color or N generic; Phyrexian {C/P} → the color or 2 life). On confirm, fills
    // `choiceIsAlternative` index-aligned to `flex` (false = primary color `colorA`, true = the
    // alternative). Returns false if the player cancelled.
    static bool promptFlexiblePipChoices(const QString &fullCost,
                                         const QString &cardName,
                                         const QVector<RuledFlexPip> &flex,
                                         QVector<bool> &choiceIsAlternative);
    // Fold the player's per-pip choices into the fixed cost: a color choice adds a fixed colored
    // pip, a mono-hybrid generic choice adds N to the generic bucket, a Phyrexian life choice
    // records the pip index in `lifePipIndices`. Clears `flex` once everything is folded so the
    // remaining cost (and the widget prompt) reflects only the resolved mana.
    static void applyFlexChoicesToCost(QMap<QChar, int> &fixed,
                                       QVector<quint32> &lifePipIndices,
                                       QVector<RuledFlexPip> &flex,
                                       const QVector<bool> &choiceIsAlternative);
    // CR 107.4d–f: route one tapped mana into the cheapest still-open demand — a fixed colored
    // pip, an untouched flexible pip's color, fixed generic, or a mono-hybrid generic alternative.
    // Returns false if the mana can't be used (caller leaves it unspent). Mutates fixed + flex.
    static bool applyManaPipToFlexibleCost(QMap<QChar, int> &fixed,
                                           QVector<RuledFlexPip> &flex,
                                           bool colorlessMana,
                                           QChar coloredMana);
    void clearPendingRuledSpellCast();
    // Prompts for the value of X when the pending spell's cost has X pips, tops up the generic
    // mana bucket, and records xValue. Returns false if the player cancelled (cast is aborted).
    bool promptForRuledSpellXIfNeeded();
    // CR 107.4d–f: front-load the pending spell's flexible-pip choices via promptFlexiblePipChoices,
    // folding them into the fixed cost. No-op (returns true) when the cost has no flexible pips.
    // Returns false if the player cancelled the dialog (cast is aborted).
    bool resolvePendingSpellFlexiblePips();
    // Same as resolvePendingSpellFlexiblePips for the pending activated ability.
    bool resolvePendingAbilityFlexiblePips();
    bool completePendingRuledSpellCast();
    bool tryReducePendingSpellRemainingCostOnePip(bool colorlessMana, QChar coloredMana);
    void finishPendingSpellManaPaymentStep();
    bool completeActivateAbility();
    bool tryReducePendingAbilityRemainingCostOnePip(bool colorlessMana, QChar coloredMana);
    void finishPendingAbilityManaPaymentStep();

    int defaultNumberTopCards = 1;
    int defaultNumberTopCardsToPlaceBelow = 1;
    int defaultNumberBottomCards = 1;
    int defaultNumberDieRoll = 20;

    TokenInfo lastTokenInfo;
    int lastTokenTableRow;

    bool movingCardsUntil;
    QTimer *moveTopCardTimer;
    FilterString movingCardsUntilFilter;
    int movingCardsUntilCounter = 0;
    MoveTopCardsUntilOptions movingCardsUntilOptions;
    PendingRuledSpellCast pendingRuledSpellCast;
    PendingActivatedAbility pendingActivatedAbility;
    QVector<LandTapUndoEntry> landTapUndoStack;
    QVector<LandTapUndoEntry> midCastLandTapStack;
    QVector<int> manaPaymentCounterIds;
    // Ruled mode: engine-reported count of this player's undoable mana abilities (CR 605 float
    // courtesy). Authoritative source for the Undo button in ruled games; the local
    // `landTapUndoStack` drives it only in freeform. See setRuledUndoableManaCount.
    int ruledUndoableManaCount = 0;
    // True when the Undo affordance should currently be offered: engine count > 0 in ruled mode,
    // a non-empty local stack in freeform.
    [[nodiscard]] bool landTapUndoCurrentlyAvailable() const;

    void moveTopCardsTo(const QString &targetZone, const QString &zoneDisplayName, bool faceDown);
    void moveBottomCardsTo(const QString &targetZone, const QString &zoneDisplayName, bool faceDown);

    void createCard(const CardItem *sourceCard,
                    const QString &dbCardName,
                    CardRelationType attach = CardRelationType::DoesNotAttach,
                    bool persistent = false);
    bool createRelatedFromRelation(const CardItem *sourceCard, const CardRelation *cardRelation);

    void playSelectedCards(bool faceDown = false);

    void cmdSetTopCard(Command_MoveCard &cmd);
    void cmdSetBottomCard(Command_MoveCard &cmd);

    QVariantList parsePT(const QString &pt);
};

#endif // COCKATRICE_PLAYER_ACTIONS_H
