#ifndef RULED_PROTOCOL_VERSION_H
#define RULED_PROTOCOL_VERSION_H

// Upstream protocol 14 and older fork builds disagree about create-game field 15.
// A distinct dialect rejects that ambiguity before either endpoint sends game commands.
inline constexpr unsigned int kRuledProtocolVersion = 10015;

#endif
