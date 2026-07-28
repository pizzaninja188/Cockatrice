#include "ruled_dev_command_parser.h"

#include <QMap>
#include <QRegularExpression>
#include <QStringList>

namespace {

/// Zone words accepted on the command line. Several spellings each, because the point of a dev
/// console is to type fast.
bool zoneForWord(const QString &word, ruled::v1::DevZone &out)
{
    static const QMap<QString, ruled::v1::DevZone> zones = {
        {QStringLiteral("hand"), ruled::v1::DEV_ZONE_HAND},
        {QStringLiteral("battlefield"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("bf"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("board"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("play"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("graveyard"), ruled::v1::DEV_ZONE_GRAVEYARD},
        {QStringLiteral("grave"), ruled::v1::DEV_ZONE_GRAVEYARD},
        {QStringLiteral("gy"), ruled::v1::DEV_ZONE_GRAVEYARD},
        {QStringLiteral("exile"), ruled::v1::DEV_ZONE_EXILE},
        {QStringLiteral("ex"), ruled::v1::DEV_ZONE_EXILE},
        {QStringLiteral("library"), ruled::v1::DEV_ZONE_LIBRARY},
        {QStringLiteral("lib"), ruled::v1::DEV_ZONE_LIBRARY},
        {QStringLiteral("deck"), ruled::v1::DEV_ZONE_LIBRARY},
    };
    const auto it = zones.constFind(word.toLower());
    if (it == zones.constEnd()) {
        return false;
    }
    out = *it;
    return true;
}

RuledDevCommandParser::Result failure(const QString &error)
{
    RuledDevCommandParser::Result r;
    r.ok = false;
    r.error = error;
    return r;
}

/// Consume a leading seat ordinal for `put`, where a numeric first token can only be a seat — no
/// zone word is numeric. Returns false (with `error` set) when the ordinal names no seat, since
/// there is no other thing it could have meant.
bool takeSeatForPut(QStringList &tokens, const QVector<int> &seatIds, int defaultSeat, int &out, QString &error)
{
    out = defaultSeat;
    if (tokens.isEmpty()) {
        return true;
    }
    bool numeric = false;
    const int ordinal = tokens.first().toInt(&numeric);
    if (!numeric) {
        return true;
    }
    if (ordinal < 1 || ordinal > seatIds.size()) {
        error = QStringLiteral("No seat %1 — this game has %2.").arg(ordinal).arg(seatIds.size());
        return false;
    }
    out = seatIds.at(ordinal - 1);
    tokens.removeFirst();
    return true;
}

/// Consume a leading seat ordinal for `mana`, where a leading number is genuinely ambiguous:
/// `mana 12` is twelve generic, while `mana 2 UU` is two blue for the second seat.
///
/// Resolved by only treating it as a seat when it is a valid ordinal *and* symbols follow it.
/// Anything else stays part of the symbols, so an out-of-range number like `mana 3 RR` reads as
/// mana rather than erroring. Write `mana 2UU` (no space) when you mean generic.
void takeSeatForMana(QStringList &tokens, const QVector<int> &seatIds, int defaultSeat, int &out)
{
    out = defaultSeat;
    if (tokens.size() < 2) {
        return;
    }
    bool numeric = false;
    const int ordinal = tokens.first().toInt(&numeric);
    if (!numeric || ordinal < 1 || ordinal > seatIds.size()) {
        return;
    }
    out = seatIds.at(ordinal - 1);
    tokens.removeFirst();
}

} // namespace

namespace RuledDevCommandParser {

QString helpText()
{
    return QStringLiteral(
        "put [seat] <zone> <card name> [ready]  — put a card into a zone.\n"
        "    Moved if that seat already owns one, otherwise conjured from outside the game.\n"
        "    zone: hand | bf | gy | exile | library   (conjuring supports hand and bf only)\n"
        "    ready: enters without summoning sickness, so it can attack this turn.\n"
        "mana [seat] <symbols>                  — add mana, e.g. 3RR or WWU. Empties at the\n"
        "    next step change, like real mana, so add it in the phase you will spend it.\n"
        "    Write generic without a space (mana 2UU): a lone leading number is read as a seat.\n"
        "seat: 1-based ordinal; omit for yourself.\n"
        "Examples:  put hand Serra Angel  |  put bf Grizzly Bears ready  |  mana 3RR");
}

Result parse(const QString &line, int localPlayerId, const QVector<int> &seatIds)
{
    QString text = line.trimmed();
    if (text.startsWith(QLatin1Char('/'))) {
        text.remove(0, 1);
        text = text.trimmed();
    }
    if (text.isEmpty()) {
        return failure(QString());
    }

    QStringList tokens = text.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
    const QString verb = tokens.takeFirst().toLower();

    if (verb == QLatin1String("help") || verb == QLatin1String("?")) {
        Result r;
        r.ok = false;
        r.handledLocally = true;
        r.message = helpText();
        return r;
    }

    int seat = localPlayerId;

    if (verb == QLatin1String("put")) {
        QString seatError;
        if (!takeSeatForPut(tokens, seatIds, localPlayerId, seat, seatError)) {
            return failure(seatError);
        }
        if (tokens.isEmpty()) {
            return failure(QStringLiteral("put needs a zone and a card name."));
        }
        ruled::v1::DevZone zone{};
        if (!zoneForWord(tokens.first(), zone)) {
            return failure(QStringLiteral("Unknown zone '%1'. Try hand, bf, gy, exile or library.")
                               .arg(tokens.first()));
        }
        tokens.removeFirst();

        // `ready` is only stripped as a trailing token, and only when something precedes it, so a
        // card whose name ends in that word still parses. None does today; the rule keeps the
        // grammar from depending on the card pool.
        bool ready = false;
        if (tokens.size() > 1 && tokens.last().toLower() == QLatin1String("ready")) {
            ready = true;
            tokens.removeLast();
        }
        // The card name is the rest of the line verbatim — names contain spaces and the zone token
        // already disambiguated, so there are no quoting rules to get wrong.
        const QString cardName = tokens.join(QLatin1Char(' '));
        if (cardName.isEmpty()) {
            return failure(QStringLiteral("put needs a card name."));
        }

        Result r;
        r.ok = true;
        auto *dev = r.command.mutable_dev_command();
        dev->set_target_player_id(seat);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName.toStdString());
        put->set_zone(zone);
        put->set_ready(ready);
        return r;
    }

    if (verb == QLatin1String("mana")) {
        takeSeatForMana(tokens, seatIds, localPlayerId, seat);
        if (tokens.isEmpty()) {
            return failure(QStringLiteral("mana needs symbols, e.g. 3RR."));
        }
        const QString symbols = tokens.join(QString());
        quint32 w = 0, u = 0, b = 0, rr = 0, g = 0, c = 0;
        // Digits accumulate into one generic amount ("12" is twelve, not one and two); each colour
        // letter adds one pip. Mirrors how a mana cost reads.
        quint32 pendingGeneric = 0;
        bool sawGeneric = false;
        for (const QChar &ch : symbols) {
            if (ch.isDigit()) {
                pendingGeneric = pendingGeneric * 10 + static_cast<quint32>(ch.digitValue());
                sawGeneric = true;
                continue;
            }
            switch (ch.toUpper().unicode()) {
                case 'W':
                    ++w;
                    break;
                case 'U':
                    ++u;
                    break;
                case 'B':
                    ++b;
                    break;
                case 'R':
                    ++rr;
                    break;
                case 'G':
                    ++g;
                    break;
                case 'C':
                    ++c;
                    break;
                default:
                    return failure(QStringLiteral("Unknown mana symbol '%1'. Use W U B R G C and digits.")
                                       .arg(ch));
            }
        }
        if (sawGeneric) {
            c += pendingGeneric;
        }
        if (w == 0 && u == 0 && b == 0 && rr == 0 && g == 0 && c == 0) {
            return failure(QStringLiteral("That adds no mana."));
        }

        Result r;
        r.ok = true;
        auto *dev = r.command.mutable_dev_command();
        dev->set_target_player_id(seat);
        auto *mana = dev->mutable_add_mana();
        mana->set_w(w);
        mana->set_u(u);
        mana->set_b(b);
        mana->set_r(rr);
        mana->set_g(g);
        mana->set_c(c);
        return r;
    }

    return failure(QStringLiteral("Unknown command '%1'. Type help.").arg(verb));
}

} // namespace RuledDevCommandParser
