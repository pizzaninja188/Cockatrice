# Offscreen widget tests for the ruled dev console. Only the widget is compiled in: it emits the
# typed line and knows nothing about parsing or transport, so no protocol library is needed.
add_executable(
  ruled_dev_console_test ruled_dev_console_test.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_dev_console.cpp
)

target_include_directories(ruled_dev_console_test PRIVATE ${CMAKE_SOURCE_DIR}/cockatrice/src
                                                          ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled)

target_link_libraries(
  ruled_dev_console_test
  PRIVATE Threads::Threads
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME ruled_dev_console_test COMMAND ruled_dev_console_test)

if(NOT GTEST_FOUND)
  add_dependencies(ruled_dev_console_test gtest)
endif()
