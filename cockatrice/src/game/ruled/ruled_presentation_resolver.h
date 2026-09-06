#ifndef COCKATRICE_RULED_PRESENTATION_RESOLVER_H
#define COCKATRICE_RULED_PRESENTATION_RESOLVER_H

#include <QSet>
#include <QString>
#include <functional>
#include <libcockatrice/card/card_info.h>

namespace ruled::v1
{
class PresentationRef;
}

class RuledPresentationResolver
{
public:
    using CardLookup = std::function<CardInfoPtr(const QString &)>;
    explicit RuledPresentationResolver(CardLookup lookup = {}) : lookup(std::move(lookup))
    {
    }
    QString resolve(const ruled::v1::PresentationRef &presentation) const;
    struct Result
    {
        QString text;
        QString diagnostic;
    };
    Result inspect(const ruled::v1::PresentationRef &presentation) const;

private:
    CardLookup lookup;
    mutable QSet<QString> reportedDiagnostics;
};

#endif
