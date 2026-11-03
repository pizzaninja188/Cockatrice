# Offscreen dev-console widget/runner tests. The runner reuses the pure command parser and an
# injected acknowledged sender; no game objects or network connection are required.
add_executable(
  ruled_dev_console_test ruled_dev_console_test.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_dev_console.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_dev_console_runner.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_dev_command_parser.cpp
)

target_include_directories(ruled_dev_console_test PRIVATE ${CMAKE_SOURCE_DIR}/cockatrice/src
                                                          ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled)

target_link_libraries(
  ruled_dev_console_test
  PRIVATE libcockatrice_protocol
  PRIVATE Threads::Threads
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME ruled_dev_console_test COMMAND ruled_dev_console_test)

if(NOT GTEST_FOUND)
  add_dependencies(ruled_dev_console_test gtest)
endif()
