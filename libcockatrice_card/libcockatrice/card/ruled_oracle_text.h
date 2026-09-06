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
    static QString normalizedText(const QString &text);
    static QString textSha256(const QString &text);
    void addFace(const QString &cardName, const QString &faceName, const QString &text);
    void readXml(QXmlStreamReader &xml);
    void writeXml(QXmlStreamWriter &xml) const;
    QString compatibleFaceText(const QString &cardName, const QString &faceName, const QString &sha256) const;

private:
    QList<RuledOracleTextFace> faces;
};

#endif
