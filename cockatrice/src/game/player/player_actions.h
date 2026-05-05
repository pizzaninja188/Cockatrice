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
    /// Apply one land mana pip toward pending spell cost (local only). Returns { consumed, costFullyPaid }.
    [[nodiscard]] QPair<bool, bool> tryConsumeLandManaPipTowardPendingSpell(const QString &manaCounterName);
    /// Call after tap `SetCardAttr` commands are sent. Completes cast and/or updates prompt.
    void afterRuledLandTapsAppliedForSpellMana(bool completeCast, bool partialCostRemainPrompt);
    /// Ruled engine pool +1 for this land pip (no UI counter). Caller owns the pointer; nullptr if N/A.
    [[nodiscard]] Command_RuledPayload *newRuledPayloadAddManaToPoolForLandName(const QString &manaCounterName);
    bool tryHandleRuledSpellTargetClick(CardItem *card);
    bool tryHandleRuledSpellTargetPlayerClick(Player *targetPlayer);
    /// True when the local player must pick a player (not permanent) for the pending ruled cast.
    [[nodiscard]] bool isAwaitingRuledPlayerTargetSelection() const;
    void cancelPendingRuledSpellCast();
    /// Returns the mana-payment prompt text if a spell is pending and still needs mana, otherwise empty.
    [[nodiscard]] QString pendingRuledSpellPromptText() const;
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

private:
    struct PendingRuledSpellCast
    {
        int handIndex = -1;
        QString cardName;
        QMap<QChar, int> remainingCost;
        QVector<quint32> selectedTargetOids;
        bool waitingForTarget = false;
        bool valid = false;
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
    static bool ruledSpellNeedsTarget(const CardItem *card);
    static QMap<QChar, int> parseSimpleManaCost(const QString &manaCost);
    static QString formatSimpleManaCost(const QMap<QChar, int> &cost);
    void clearPendingRuledSpellCast();
    bool completePendingRuledSpellCast();
    bool tryReducePendingSpellRemainingCostOnePip(bool colorlessMana, QChar coloredMana);
    void finishPendingSpellManaPaymentStep();

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
    QVector<LandTapUndoEntry> landTapUndoStack;
    QVector<LandTapUndoEntry> midCastLandTapStack;
    QVector<int> manaPaymentCounterIds;

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
