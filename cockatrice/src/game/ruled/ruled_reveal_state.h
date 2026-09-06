#ifndef COCKATRICE_RULED_REVEAL_STATE_H
#define COCKATRICE_RULED_REVEAL_STATE_H

#include <QMap>
#include <QObject>
#include <QSet>
#include <QStringList>
#include <QVector>

namespace ruled::v1
{
class CardsRevealed;
class ActivePublicRevealSnapshot;
} // namespace ruled::v1

// One presentation history for all public reveals. Completed entries are immutable snapshots,
// never handles to live hidden cards. Choice IDs are usable only on the current choice surface.
class RuledRevealState : public QObject
{
    Q_OBJECT
public:
    enum class Phase
    {
        Completed,
        Choice,
        Active
    };
    struct Card
    {
        quint32 objectId = 0;
        quint64 generation = 0;
        QString cardId;
        QString name;
        bool operator==(const Card &) const = default;
    };
    struct Entry
    {
        QString id;
        int owner = -1;
        int sourceZone = 0;
        quint32 sourceObjectId = 0;
        QString sourceDescription;
        QVector<Card> cards;
        Phase phase = Phase::Completed;
        QVector<int> choiceCardIds;
        QStringList cardNames() const;
        bool operator==(const Entry &) const = default;
    };

    explicit RuledRevealState(QObject *parent = nullptr) : QObject(parent)
    {
    }
    bool publish(const ruled::v1::CardsRevealed &reveal);
    bool beginChoice(const ruled::v1::CardsRevealed &reveal, const QVector<int> &choiceCardIds);
    void completeChoice();
    void applyActiveSnapshot(const ruled::v1::ActivePublicRevealSnapshot &snapshot);
    void dismiss(const QString &id);
    void clear();
    void setWindowsSuppressed(bool suppressed);
    void discardCompleted();
    bool windowsSuppressed() const
    {
        return suppressed_;
    }
    const QMap<QString, Entry> &entries() const
    {
        return entries_;
    }
    const QStringList &order() const
    {
        return order_;
    }
    QString choiceId() const
    {
        return choiceId_;
    }
    bool hasChoice() const
    {
        return !choiceId_.isEmpty();
    }
    const Entry *choice() const;
signals:
    void changed();

private:
    QMap<QString, Entry> entries_;
    QStringList order_;
    QSet<QString> seen_;
    QSet<QString> active_;
    QString choiceId_;
    bool suppressed_ = false;
};

#endif
