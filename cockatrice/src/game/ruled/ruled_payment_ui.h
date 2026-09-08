#ifndef RULED_PAYMENT_UI_H
#define RULED_PAYMENT_UI_H

#include "ruled_payment.h"
#include "ruled_pending_cast.h"

#include <QPair>
#include <QVector>
#include <optional>

class PlayerActions;
class CardItem;
class QPainter;
class Command_RuledPayload;

/// Fork-owned payment and cast-progression bridge. RuledPendingCast owns the local transaction;
/// RuledPayment stages its payment, and Rust authors legality. PlayerActions supplies UI/transport.
class RuledPaymentUi
{
public:
    [[nodiscard]] QJsonObject diagnosticSnapshot() const;
    explicit RuledPaymentUi(PlayerActions *actions);
    static std::optional<ruled::v1::RuledCommand> buildCommand(PlayerActions *actions);
    static std::optional<ruled::v1::RuledCommand> buildActivationCommand(PlayerActions *actions);
    bool startOrRefresh();
    bool payMana(const QString &name, quint32 groupId = 0);
    bool autoPayMana(const QString &name, int amount, quint32 groupId = 0);
    bool click(CardItem *card, bool leftClick);
    [[nodiscard]] QVector<QPair<int, QString>> contributionOptions(CardItem *card) const;
    bool contribute(CardItem *card, int kind);
    [[nodiscard]] int optimisticManaCounterSpendCount(int counterId) const;
    [[nodiscard]] int restrictedManaSpendCount(quint32 groupId, QChar symbol) const;
    bool applicable() const;
    [[nodiscard]] bool isStagingSpecialCast(const RuledClientState &state) const;
    QString prompt() const;
    void clear();
    void suspendForManaAbility(quint32 oid, int abilityIndex);
    void resumeAfterManaAbility();
    static void paint(CardItem *card, QPainter *painter);

    // Local casting/cost progression; state remains in RuledPendingCast.
    bool tryHandlePriorityCostClick(CardItem *card);
    bool tryHandleAdditionalCostClick(CardItem *card);
    void installProgressionConnections();
    bool tryRequireSpellTargetCost(ruled::v1::TargetRefKind kind, quint32 targetOid, int activeGroupIndex);
    bool tryUndoManaAbility();
    void reconcilePendingRuledTargetSelections();
    void clearPendingRuledSpellCast();
    void cancelPendingRuledSpellCast();
    void selectPendingRuledCastCostOption(int optionIndex);
    void confirmPendingRuledCastCostGroup();
    void backPendingRuledCastCostObject();
    void continuePendingSpellAfterChoice();
    void continuePendingActivatedAbilityAfterChoice();
    bool tryPayRuledAbilityWithCounter(const QString &counterName);
    bool tryPayRuledRestrictedMana(quint32 groupId, QChar symbol);
    void cancelPendingActivatedAbility();
    Command_RuledPayload *newRuledPayloadActivateManaAbilityForLand(CardItem *card, QChar desiredColor);
    bool tryPayRuledSpellWithCounter(const QString &counterName);
    bool tryPayRuledResolutionWithCounter(const QString &counterName);
    void syncRuledResolutionPayment(bool active, int genericCost);
    int ruledManaCounterOptimisticSpendCount(int counterId) const;
    int ruledRestrictedManaOptimisticSpendCount(quint32 groupId, QChar symbol) const;
    bool ruledRestrictedManaPaymentPending() const;
    bool ruledRestrictedManaGroupEligible(quint32 groupId) const;
    void clearRestrictedManaPaymentSelections();
    void declineRuledResolutionPayment();
    void finishRuledResolutionPaymentSubmission(bool accepted);
    void autoApplyFloatedManaToPendingCost(const QString &counterName, int amount);
    void confirmRuledGraveyardCostSelection();
    void cancelRuledGraveyardCostSelection();
    void resumePendingRuledPaymentAfterEngineCommand();
    static bool startPublicZoneCast(PlayerActions *actions, CardItem *card, bool contextMenu);
    bool tryStartRuledSpellCast(CardItem *card);
    bool tryRuledSpellCastFaceMenu(CardItem *card);
    bool beginRuledSpellCast(CardItem *card,
                             int ruledHandIndex,
                             int faceIndex,
                             const QString &castName,
                             const QString &castCost,
                             int genericCostReduction,
                             RuledCastSource source,
                             ruled::v1::CastMethod castMethod,
                             quint64 castingPermissionId = 0);
    void autoApplyRestrictedManaToPendingCost(quint32 groupId, QChar symbol, int amount);
    void finalizePendingSpellManaCost();
    bool tryRuledActivateAbilityMenu(CardItem *card, bool leftClick);

private:
    static bool promptFlexiblePipChoices(const QString &fullCost,
                                         const QString &cardName,
                                         const QVector<RuledFlexPip> &flex,
                                         QVector<bool> &choiceIsAlternative);
    bool promptForRuledSpellXIfNeeded();
    bool resolvePendingSpellFlexiblePips();
    bool resolvePendingAbilityFlexiblePips();
    bool completePendingRuledSpellCast();
    bool completeActivateAbility();
    bool tryReducePendingAbilityRemainingCostOnePip(bool colorlessMana, QChar coloredMana);
    void finishPendingAbilityManaPaymentStep();
    bool tryReducePendingSpellRemainingCostOnePip(bool colorlessMana, QChar coloredMana);
    void finishPendingSpellManaPaymentStep();
    QSet<quint32> eligibleRestrictedManaForPendingAbility() const;
    bool promptForNextRuledCastCostGroup();
    void continuePendingSpellAfterCastCostGroups();

    enum class Context
    {
        None,
        Spell,
        Ability,
        Resolution
    };

    struct SuspendedPayment
    {
        std::optional<PendingRuledSpellCast> spell;
        std::optional<PendingActivatedAbility> ability;
        RuledPayment payment;
        Context context = Context::None;
    };
    Context context() const;
    Context activeContext = Context::None;
    std::optional<ruled::v1::RuledCommand> buildPaymentCommand() const;
    void schedule();
    void query();
    void received();
    void changed();
    void restoreOptimisticManaCounters(const QVector<int> &counterIds);
    bool stageMana(RuledPayment &model, const QString &name, quint32 groupId);
    PlayerActions *actions;
    bool queued = false;
    bool choosingLifePayment = false;
    QVector<SuspendedPayment> suspendedPayments;
};

#endif
