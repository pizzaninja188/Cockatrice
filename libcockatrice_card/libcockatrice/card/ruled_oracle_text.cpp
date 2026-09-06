#include "ruled_oracle_text.h"

#include <QCryptographicHash>
#include <QRegularExpression>
#include <QStringList>
#include <QXmlStreamReader>
#include <QXmlStreamWriter>

QString RuledOracleText::normalizedText(const QString &text)
{
    static const QRegularExpression loyaltyCost(QStringLiteral(R"(^\[((?:\+|−|-)?(?:\d+|X))\]:)"));
    QString normalized = text;
    normalized.replace(QStringLiteral("\r\n"), QStringLiteral("\n"));
    normalized.replace(QLatin1Char('\r'), QLatin1Char('\n'));
    QStringList lines;
    for (const QString &line : normalized.split(QLatin1Char('\n'))) {
        QString trimmed = line.trimmed();
        if (!trimmed.isEmpty()) {
            trimmed.replace(loyaltyCost, QStringLiteral("\\1:"));
            lines.append(trimmed);
        }
    }
    return lines.join(QLatin1Char('\n'));
}

QString RuledOracleText::textSha256(const QString &text)
{
    return QString::fromLatin1(
        QCryptographicHash::hash(normalizedText(text).toUtf8(), QCryptographicHash::Sha256).toHex());
}

void RuledOracleText::addFace(const QString &cardName, const QString &faceName, const QString &text)
{
    if (cardName.isEmpty() || faceName.isEmpty()) {
        return;
    }
    for (auto &face : faces) {
        if (face.cardName == cardName && face.faceName == faceName) {
            face.oracleText = normalizedText(text);
            return;
        }
    }
    faces.append({cardName, faceName, normalizedText(text)});
}

void RuledOracleText::readXml(QXmlStreamReader &xml)
{
    faces.clear();
    bool invalid = false;
    while (xml.readNextStartElement()) {
        if (xml.name() != QLatin1String("face")) {
            invalid = true;
            xml.skipCurrentElement();
            continue;
        }
        const QString cardName = xml.attributes().value("card-name").toString();
        const QString faceName = xml.attributes().value("face-name").toString();
        QString text;
        bool hasText = false;
        for (const auto &face : faces) {
            invalid |= face.cardName == cardName && face.faceName == faceName;
        }
        while (xml.readNextStartElement()) {
            if (xml.name() == QLatin1String("text") && !hasText) {
                text = xml.readElementText();
                hasText = true;
            } else {
                invalid = true;
                xml.skipCurrentElement();
            }
        }
        invalid |= !hasText || cardName.isEmpty() || faceName.isEmpty();
        addFace(cardName, faceName, text);
    }
    if (invalid || xml.hasError()) {
        faces.clear();
    }
}

void RuledOracleText::writeXml(QXmlStreamWriter &xml) const
{
    if (faces.isEmpty()) {
        return;
    }
    xml.writeStartElement("ruled-oracle");
    for (const auto &face : faces) {
        xml.writeStartElement("face");
        xml.writeAttribute("card-name", face.cardName);
        xml.writeAttribute("face-name", face.faceName);
        xml.writeTextElement("text", face.oracleText);
        xml.writeEndElement();
    }
    xml.writeEndElement();
}

RuledOracleText::FaceResult
RuledOracleText::inspectFace(const QString &cardName, const QString &faceName, const QString &sha256) const
{
    static const QRegularExpression fingerprint(QStringLiteral("^[0-9a-fA-F]{64}$"));
    if (!fingerprint.match(sha256).hasMatch()) {
        return {{}, FaceStatus::InvalidFingerprint, {}};
    }
    for (const auto &face : faces) {
        if (face.cardName == cardName && face.faceName == faceName) {
            const QString actualSha256 = textSha256(face.oracleText);
            if (actualSha256 != sha256.toLower()) {
                return {{}, FaceStatus::FingerprintMismatch, actualSha256};
            }
            return {face.oracleText, face.oracleText.isEmpty() ? FaceStatus::EmptyText : FaceStatus::Matched,
                    actualSha256};
        }
    }
    return {{}, FaceStatus::MissingFace, {}};
}

QString
RuledOracleText::compatibleFaceText(const QString &cardName, const QString &faceName, const QString &sha256) const
{
    return inspectFace(cardName, faceName, sha256).text;
}
