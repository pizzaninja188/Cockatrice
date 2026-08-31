#include "ruled_presentation_resolver.h"

#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <QFileInfo>

bool RuledPresentationResolver::loadForCardDatabase(const QString &cardDatabasePath, QString *error)
{
    const QString cachePath = RuledOracleCache::cachePathForCardDatabase(cardDatabasePath);
    const bool loaded = cache.load(cachePath, error);
    if (loaded) {
        const QFileInfo info(cachePath);
        loadedCachePath = cachePath;
        loadedLastModified = info.lastModified();
        loadedSize = info.size();
    }
    return loaded;
}

void RuledPresentationResolver::refreshForCardDatabase(const QString &cardDatabasePath)
{
    if (cardDatabasePath.isEmpty()) {
        return;
    }
    const QString cachePath = RuledOracleCache::cachePathForCardDatabase(cardDatabasePath);
    const QFileInfo info(cachePath);
    if (cachePath == loadedCachePath && info.exists() && info.lastModified() == loadedLastModified &&
        info.size() == loadedSize) {
        return;
    }
    loadForCardDatabase(cardDatabasePath);
}

QString RuledPresentationResolver::resolve(const ruled::v1::PresentationRef &presentation) const
{
    const QString fallback = QString::fromStdString(presentation.fallback_text());
    if (presentation.oracle_line_indices().empty()) {
        return fallback;
    }
    const QString text = cache.compatibleFaceText(QString::fromStdString(presentation.external_card_name()),
                                                  QString::fromStdString(presentation.external_face_name()),
                                                  QString::fromStdString(presentation.oracle_text_sha256()));
    if (text.isEmpty()) {
        return fallback;
    }
    const QStringList lines = text.split(QLatin1Char('\n'));
    QStringList selected;
    selected.reserve(static_cast<qsizetype>(presentation.oracle_line_indices().size()));
    for (const quint32 oneBasedIndex : presentation.oracle_line_indices()) {
        if (oneBasedIndex == 0 || oneBasedIndex > static_cast<quint32>(lines.size())) {
            return fallback;
        }
        selected.append(lines.at(static_cast<qsizetype>(oneBasedIndex - 1)));
    }
    return selected.join(QLatin1Char('\n'));
}
