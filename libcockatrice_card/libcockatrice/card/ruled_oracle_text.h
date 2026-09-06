#ifndef RULED_ORACLE_TEXT_H
#define RULED_ORACLE_TEXT_H

#include <QList>
#include <QString>

class QXmlStreamReader;
class QXmlStreamWriter;

struct RuledOracleTextFace
{
    QString cardName;
    QString faceName;
    QString oracleText;
};

/// Exact external faces, separate from the combined text used for ordinary card display.
class RuledOracleText
{
public:
    enum class FaceStatus
    {
        Matched,
        MissingFace,
        InvalidFingerprint,
        FingerprintMismatch,
        EmptyText
    };
    struct FaceResult
    {
        QString text;
        FaceStatus status;
        QString actualSha256;
    };
    static QString normalizedText(const QString &text);
    static QString textSha256(const QString &text);
    void addFace(const QString &cardName, const QString &faceName, const QString &text);
    void readXml(QXmlStreamReader &xml);
    void writeXml(QXmlStreamWriter &xml) const;
    FaceResult inspectFace(const QString &cardName, const QString &faceName, const QString &sha256) const;
    QString compatibleFaceText(const QString &cardName, const QString &faceName, const QString &sha256) const;

private:
    QList<RuledOracleTextFace> faces;
};

#endif
