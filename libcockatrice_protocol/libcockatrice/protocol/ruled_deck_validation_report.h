#pragma once
#include <QString>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
namespace RuledDeckValidationReport
{
QString describe(const ruled::v1::DeckValidationReport &report);
}
