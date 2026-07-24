# Ruled E2E smoke test (refactor roadmap Step 3): real servatrice + real tricerules-server +
# two scripted protobuf-level clients over TCP. Only built when the server target exists.
# The test SKIPs at runtime if either binary is missing (RULED_E2E_REQUIRE=1 makes that a failure).

add_executable(ruled_e2e_smoke_test ${VERSION_STRING_CPP} ruled_e2e_smoke_test.cpp)

if(NOT GTEST_FOUND)
  add_dependencies(ruled_e2e_smoke_test gtest)
endif()

# The test spawns the servatrice and tricerules-server binaries; inject their paths.
if(NOT TRICERULES_BIN)
  # WITH_RULES_ENGINE not enabled in this tree; fall back to the standard cargo output path
  # (a manual `cargo build --release` also lands there).
  if(WIN32)
    set(TRICERULES_BIN "${CMAKE_SOURCE_DIR}/tricerules/target/release/tricerules-server.exe")
  else()
    set(TRICERULES_BIN "${CMAKE_SOURCE_DIR}/tricerules/target/release/tricerules-server")
  endif()
endif()
target_compile_definitions(
  ruled_e2e_smoke_test PRIVATE RULED_E2E_SERVATRICE_PATH="$<TARGET_FILE:servatrice>"
                               RULED_E2E_TRICERULES_PATH="${TRICERULES_BIN}"
)

# Make sure the binaries the test spawns are up to date when the test builds.
add_dependencies(ruled_e2e_smoke_test servatrice)
if(TARGET tricerules)
  add_dependencies(ruled_e2e_smoke_test tricerules)
endif()

target_link_libraries(
  ruled_e2e_smoke_test
  PRIVATE libcockatrice_protocol
  PRIVATE Threads::Threads
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME ruled_e2e_smoke_test COMMAND ruled_e2e_smoke_test)
set_tests_properties(ruled_e2e_smoke_test PROPERTIES TIMEOUT 300)
