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

private:
    QHash<int, int> authoritativeValues;
};

#endif // COCKATRICE_RULED_MANA_POOL_TRACKER_H
