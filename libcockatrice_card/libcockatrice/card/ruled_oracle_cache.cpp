#include "ruled_oracle_cache.h"

#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QSaveFile>
#include <QStringList>
#include <algorithm>

namespace
{
constexpr int CACHE_FORMAT_VERSION = 1;

void setError(QString *error, const QString &message)
{
    if (error) {
        *error = message;
    }
}

QString legacyNormalizedText(const QString &text)
{
    QString normalized = text;
    normalized.replace(QStringLiteral("\r\n"), QStringLiteral("\n"));
    normalized.replace(QLatin1Char('\r'), QLatin1Char('\n'));
    QStringList lines;
    for (const QString &line : normalized.split(QLatin1Char('\n'))) {
        const QString trimmed = line.trimmed();
        if (!trimmed.isEmpty()) {
            lines.append(trimmed);
        }
    }
    return lines.join(QLatin1Char('\n'));
}

QString legacyTextSha256(const QString &text)
{
    return QString::fromLatin1(
        QCryptographicHash::hash(legacyNormalizedText(text).toUtf8(), QCryptographicHash::Sha256).toHex());
}
}

QString RuledOracleCache::cachePathForCardDatabase(const QString &cardDatabasePath)
{
    const QFileInfo info(cardDatabasePath);
    return info.dir().filePath(info.completeBaseName() + QStringLiteral(".ruled-oracle.json"));
}

QString RuledOracleCache::normalizedText(const QString &text)
{
    static const QRegularExpression bracketedLoyaltyCost(
        QStringLiteral(R"(^\[((?:\+|−|-)?(?:\d+|X))\]:)"));
    QStringList lines = legacyNormalizedText(text).split(QLatin1Char('\n'));
    for (QString &line : lines) {
        line.replace(bracketedLoyaltyCost, QStringLiteral("\\1:"));
    }
    return lines.join(QLatin1Char('\n'));
}

QString RuledOracleCache::textSha256(const QString &text)
{
    return QString::fromLatin1(
        QCryptographicHash::hash(normalizedText(text).toUtf8(), QCryptographicHash::Sha256).toHex());
}

QString RuledOracleCache::key(const QString &cardName, const QString &faceName)
{
    return cardName + QChar(0x1f) + faceName;
}

bool RuledOracleCache::writeAtomic(const QString &cachePath,
                                   const QString &sourceUrl,
                                   const QString &sourceVersion,
                                   const QList<RuledOracleFace> &faces,
                                   QString *error)
{
    QJsonArray serializedFaces;
    QHash<QString, RuledOracleFace> uniqueFaces;
    for (const RuledOracleFace &face : faces) {
        if (!face.cardName.isEmpty() && !face.faceName.isEmpty()) {
            uniqueFaces.insert(key(face.cardName, face.faceName), face);
        }
    }
    QList<QString> keys = uniqueFaces.keys();
    std::sort(keys.begin(), keys.end());
    for (const QString &faceKey : keys) {
        const RuledOracleFace &face = uniqueFaces[faceKey];
        const QString normalized = normalizedText(face.oracleText);
        serializedFaces.append(QJsonObject{
            {QStringLiteral("cardName"), face.cardName},
            {QStringLiteral("faceName"), face.faceName},
            {QStringLiteral("oracleText"), normalized},
            {QStringLiteral("sha256"), textSha256(normalized)},
        });
    }

    const QJsonObject root{
        {QStringLiteral("formatVersion"), CACHE_FORMAT_VERSION},
        {QStringLiteral("source"),
         QJsonObject{{QStringLiteral("url"), sourceUrl}, {QStringLiteral("version"), sourceVersion}}},
        {QStringLiteral("faces"), serializedFaces},
    };
    QSaveFile file(cachePath);
    if (!file.open(QIODevice::WriteOnly)) {
        setError(error, file.errorString());
        return false;
    }
    const QByteArray bytes = QJsonDocument(root).toJson(QJsonDocument::Compact);
    if (file.write(bytes) != bytes.size()) {
        setError(error, file.errorString());
        file.cancelWriting();
        return false;
    }
    if (!file.commit()) {
        setError(error, file.errorString());
        return false;
    }
    return true;
}

bool RuledOracleCache::load(const QString &cachePath, QString *error)
{
    valid = false;
    loadedSourceUrl.clear();
    loadedSourceVersion.clear();
    cachedFaces.clear();
    QFile file(cachePath);
    if (!file.open(QIODevice::ReadOnly)) {
        setError(error, file.errorString());
        return false;
    }
    QJsonParseError parseError;
    const QJsonDocument document = QJsonDocument::fromJson(file.readAll(), &parseError);
    const QJsonObject root = document.object();
    if (parseError.error != QJsonParseError::NoError || root.value(QStringLiteral("formatVersion")).toInt() !=
                                                            CACHE_FORMAT_VERSION) {
        setError(error, QStringLiteral("invalid ruled Oracle cache format"));
        return false;
    }
    const QJsonObject source = root.value(QStringLiteral("source")).toObject();
    loadedSourceUrl = source.value(QStringLiteral("url")).toString();
    loadedSourceVersion = source.value(QStringLiteral("version")).toString();
    const QJsonArray faces = root.value(QStringLiteral("faces")).toArray();
    for (const QJsonValue &value : faces) {
        const QJsonObject face = value.toObject();
        const QString cardName = face.value(QStringLiteral("cardName")).toString();
        const QString faceName = face.value(QStringLiteral("faceName")).toString();
        const QString text = face.value(QStringLiteral("oracleText")).toString();
        const QString sha256 = face.value(QStringLiteral("sha256")).toString().toLower();
        const QString canonicalSha256 = textSha256(text);
        if (cardName.isEmpty() || faceName.isEmpty() || sha256.size() != 64 ||
            (canonicalSha256 != sha256 && legacyTextSha256(text) != sha256)) {
            setError(error, QStringLiteral("ruled Oracle cache contains incompatible face data"));
            cachedFaces.clear();
            return false;
        }
        cachedFaces.insert(key(cardName, faceName), CachedFace{normalizedText(text), canonicalSha256});
    }
    valid = true;
    return true;
}

bool RuledOracleCache::isValid() const
{
    return valid;
}

QString RuledOracleCache::sourceUrl() const
{
    return loadedSourceUrl;
}

QString RuledOracleCache::sourceVersion() const
{
    return loadedSourceVersion;
}

QString RuledOracleCache::compatibleFaceText(const QString &cardName,
                                              const QString &faceName,
                                              const QString &expectedSha256) const
{
    if (!valid || expectedSha256.size() != 64) {
        return {};
    }
    const auto it = cachedFaces.constFind(key(cardName, faceName));
    if (it == cachedFaces.cend() || it->sha256 != expectedSha256.toLower()) {
        return {};
    }
    return it->text;
}
