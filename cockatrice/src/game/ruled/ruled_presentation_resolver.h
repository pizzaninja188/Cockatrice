#ifndef COCKATRICE_RULED_PRESENTATION_RESOLVER_H
#define COCKATRICE_RULED_PRESENTATION_RESOLVER_H

#include <QString>
#include <QDateTime>
#include <libcockatrice/card/ruled_oracle_cache.h>

namespace ruled::v1
{
class PresentationRef;
}

class RuledPresentationResolver
{
public:
    bool loadForCardDatabase(const QString &cardDatabasePath, QString *error = nullptr);
    void refreshForCardDatabase(const QString &cardDatabasePath);
    QString resolve(const ruled::v1::PresentationRef &presentation) const;

private:
    RuledOracleCache cache;
    QString loadedCachePath;
    QDateTime loadedLastModified;
    qint64 loadedSize = -1;
};

#endif
