#ifndef RULED_ORACLE_CACHE_H
#define RULED_ORACLE_CACHE_H

#include <QHash>
#include <QList>
#include <QString>

struct RuledOracleFace
{
    QString cardName;
    QString faceName;
    QString oracleText;
};

class RuledOracleCache
{
public:
    static QString cachePathForCardDatabase(const QString &cardDatabasePath);
    static QString normalizedText(const QString &text);
    static QString textSha256(const QString &text);
    static bool writeAtomic(const QString &cachePath,
                            const QString &sourceUrl,
                            const QString &sourceVersion,
                            const QList<RuledOracleFace> &faces,
                            QString *error = nullptr);

    bool load(const QString &cachePath, QString *error = nullptr);
    bool isValid() const;
    QString sourceUrl() const;
    QString sourceVersion() const;
    QString compatibleFaceText(const QString &cardName,
                               const QString &faceName,
                               const QString &expectedSha256) const;

private:
    struct CachedFace
    {
        QString text;
        QString sha256;
    };

    static QString key(const QString &cardName, const QString &faceName);

    bool valid = false;
    QString loadedSourceUrl;
    QString loadedSourceVersion;
    QHash<QString, CachedFace> cachedFaces;
};

#endif
