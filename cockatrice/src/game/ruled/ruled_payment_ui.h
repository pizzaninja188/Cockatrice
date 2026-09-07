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

/// Fork-owned UI bridge. The headless RuledPayment holds staging; Rust authors legality.
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
    QString prompt() const;
    void clear();
    void suspendForManaAbility(quint32 oid, int abilityIndex);
    void resumeAfterManaAbility();
    static void paint(CardItem *card, QPainter *painter);

private:
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
