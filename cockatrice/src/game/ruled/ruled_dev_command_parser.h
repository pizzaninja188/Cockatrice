/**
 * @file ruled_dev_command_parser.h
 * @ingroup Ruled
 * @brief Turns a typed dev-console line into a typed `ruled::v1::DevCommand`.
 *
 * Fork-owned. Deliberately free of Qt widgets and of `AbstractGame` / `Player` / `CardItem`, for
 * the same reason `ruled_client_state` is: it links into the headless test target, so the grammar
 * is covered without a running game.
 *
 * The text stops being text here. Everything past this file is a typed protobuf oneof, so the
 * wire contract stays self-documenting and the replay log stays meaningful — see the fork's
 * no-scripting-DSL rule. Adding a primitive is a proto arm plus a case in `parse`.
 */

#ifndef RULED_DEV_COMMAND_PARSER_H
#define RULED_DEV_COMMAND_PARSER_H

#include <QString>
#include <QVector>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

namespace RuledDevCommandParser
{

struct Result
{
    /// True when `command` is ready to send. False means `error` explains why not.
    bool ok = false;
    /// True for lines handled entirely client-side (`help`): nothing to send, `error` is empty
    /// and `message` carries the text to show.
    bool handledLocally = false;
    ruled::v1::RuledCommand command;
    QString error;
    QString message;
};

/**
 * Parse one console line.
 *
 * @param line        raw input; a leading `/` is accepted and ignored so chat muscle memory works.
 * @param localPlayerId  seat used when the line names none.
 * @param seatIds     every seat in the game, ascending. Seats are addressed by 1-based ordinal
 *                    (`put 2 bf ...`), never by raw player id — with ids typically 0 and 1 the two
 *                    readings collide, and an ordinal is what someone typing at a console means.
 */
[[nodiscard]] Result parse(const QString &line, int localPlayerId, const QVector<int> &seatIds);

/// One-screen usage text, shown by `help` and on a parse error.
[[nodiscard]] QString helpText();

} // namespace RuledDevCommandParser

#endif // RULED_DEV_COMMAND_PARSER_H
