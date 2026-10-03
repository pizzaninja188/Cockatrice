#ifndef COCKATRICE_RULED_MANA_POOL_TRACKER_H
#define COCKATRICE_RULED_MANA_POOL_TRACKER_H

#include <QHash>

/// Tracks engine-owned absolute pool values independently from the optimistically reduced display.
class RuledManaPoolTracker
{
public:
    struct Refresh
    {
        int newlyProduced = 0;
        int displayedBeforeNewStaging = 0;
    };

    [[nodiscard]] Refresh observe(int counterId,
                                  int displayedOldValue,
                                  int authoritativeNewValue,
                                  int optimisticallyStaged)
    {
        const int authoritativeOldValue =
            authoritativeValues.value(counterId, displayedOldValue + optimisticallyStaged);
        authoritativeValues.insert(counterId, authoritativeNewValue);
        return {qMax(0, authoritativeNewValue - authoritativeOldValue),
                qMax(0, authoritativeNewValue - optimisticallyStaged)};
    }

    void remove(int counterId)
    {
        authoritativeValues.remove(counterId);
    }

    [[nodiscard]] int restoreOptimisticDebit(int counterId, int displayedValue, int stillStaged) const
    {
        // A later preview can retire a debit after Undo already removed its engine mana.
        // Cap this credit rather than refunding other pips awaiting submission.
        const int credited = displayedValue + 1;
        const auto authoritative = authoritativeValues.constFind(counterId);
        return authoritative == authoritativeValues.cend()
                   ? credited
                   : qMax(0, qMin(credited, authoritative.value() - stillStaged));
    }

private:
    QHash<int, int> authoritativeValues;
};

#endif // COCKATRICE_RULED_MANA_POOL_TRACKER_H
