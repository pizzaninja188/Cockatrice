#ifndef COCKATRICE_RULED_CARD_POPUP_H
#define COCKATRICE_RULED_CARD_POPUP_H

#include <functional>
#include <libcockatrice/utility/card_ref.h>

class AbstractCardItem;
class AbstractGame;
class QObject;
class QPoint;

/// The per-card identity used by the popup open and dismissal signals.
class RuledCardPopupIdentity
{
public:
    CardRef open(const CardRef &common, bool ruledFaceDownExile, const QString &privateName)
    {
        shownExileIdentity = ruledFaceDownExile;
        shownExileName = privateName;
        if (ruledFaceDownExile) {
            return {privateName, {}};
        }
        return common;
    }

    QString close(const QString &signalName)
    {
        const bool useExileName = shownExileIdentity && signalName != QStringLiteral("_");
        const QString shownName = shownExileName;
        shownExileIdentity = false;
        shownExileName.clear();
        return useExileName ? shownName : signalName;
    }

private:
    bool shownExileIdentity = false;
    QString shownExileName;
};

void connectRuledCardPopup(AbstractGame *game,
                           AbstractCardItem *card,
                           QObject *context,
                           std::function<void(const QPoint &, const CardRef &)> show,
                           std::function<void(const QString &)> dismiss);

#endif // COCKATRICE_RULED_CARD_POPUP_H
