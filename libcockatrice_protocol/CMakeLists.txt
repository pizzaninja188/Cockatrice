# Top-level wrapper for the protobuf library

add_subdirectory(libcockatrice/protocol/pb)

add_library(libcockatrice_protocol STATIC)
target_sources(libcockatrice_protocol PRIVATE libcockatrice/protocol/ruled_diagnostic_archive.cpp)
target_sources(libcockatrice_protocol PRIVATE libcockatrice/protocol/ruled_diagnostic_reader.cpp)

set(SOURCES libcockatrice/protocol/ruled_diagnostic_journal.cpp libcockatrice/protocol/ruled_diagnostics.cpp libcockatrice/protocol/debug_pb_message.cpp libcockatrice/protocol/featureset.cpp
            libcockatrice/protocol/get_pb_extension.cpp libcockatrice/protocol/pending_command.cpp
)

set(HEADERS libcockatrice/protocol/ruled_diagnostic_journal.h libcockatrice/protocol/ruled_diagnostics.h libcockatrice/protocol/debug_pb_message.h libcockatrice/protocol/featureset.h
            libcockatrice/protocol/get_pb_extension.h libcockatrice/protocol/pending_command.h
)

target_sources(libcockatrice_protocol PRIVATE ${SOURCES} ${HEADERS})

add_executable(ruled-capture-tool libcockatrice/protocol/ruled_capture_tool.cpp)
target_link_libraries(ruled-capture-tool PRIVATE libcockatrice_protocol)
# Reflection must see every proto2 extension, including events absent from this tool's code paths.
if(MSVC)
  target_link_options(ruled-capture-tool PRIVATE "/WHOLEARCHIVE:$<TARGET_FILE:libcockatrice_protocol_pb>")
  add_custom_command(TARGET ruled-capture-tool POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "$<TARGET_FILE:${QT_CORE_MODULE}>" "$<TARGET_FILE_DIR:ruled-capture-tool>")
elseif(APPLE)
  target_link_options(ruled-capture-tool PRIVATE "LINKER:-force_load,$<TARGET_FILE:libcockatrice_protocol_pb>")
else()
  target_link_libraries(ruled-capture-tool PRIVATE "-Wl,--whole-archive" libcockatrice_protocol_pb "-Wl,--no-whole-archive")
endif()

add_dependencies(libcockatrice_protocol libcockatrice_protocol_pb)

# Link the actual generated protobuf library
target_link_libraries(libcockatrice_protocol PUBLIC ${QT_CORE_MODULE} libcockatrice_protocol_pb libcockatrice_utility)

# Expose include paths
target_include_directories(
  libcockatrice_protocol PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}
                                ${CMAKE_CURRENT_BINARY_DIR} # points to the generated headers
)
