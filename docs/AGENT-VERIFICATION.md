# Agent verification workflow

This is the command source of truth for agent-driven changes. The root `AGENTS.md` defines when each tier is required; nested guidance adds subsystem-specific gates.

The active agent workflow assumes Windows and PowerShell. Existing Linux/macOS scripts remain in the repository for possible future use, but they are not required agent gates.

## Verification ladder

1. **Red:** add and run the smallest focused regression. Record that it fails for the intended missing or broken behavior.
2. **Green:** make one coherent implementation increment and rerun the regression.
3. **Stabilize:** run the affected package or targeted CTest group after each subsequent coherent increment.
4. **Finish:** once stable, run the full build and full suite for every affected side. Run lint, format, generated-data gates, and `git diff --check` as applicable.
5. **Manual:** use the real two-client flow when behavior depends on Qt interaction, networking, hidden information, or physical identity.

“After every code change” means after every coherent implementation increment that can be meaningfully compiled or tested. Batch inseparable edits needed to establish one compilable state; do not postpone all verification until the end.

Documentation, formatting, generated output, and mechanical moves do not require manufactured executable tests. Validate their actual contracts: links and targets, stale-reference searches, generators where applicable, and `git diff --check`.

## Quiet Windows runner

`scripts/run-quiet-command.ps1` runs one external command, stores complete output under `build/verification-logs`, prints one success line, prints the full log on failure, and exits with the wrapped command's exact status. Use it for noisy commands; do not hide failure output.

Examples from the repository root:

```powershell
# Focused Rust scenario
./scripts/run-quiet-command.ps1 `
  -Label 'Rust scenario: multi_face' `
  -WorkingDirectory tricerules `
  -Executable cargo `
  -ArgumentList @('test', '--quiet', '-p', 'tricerules-core', '--test', 'scenario', 'multi_face')

# Full Rust tests
./scripts/run-quiet-command.ps1 `
  -Label 'Rust tests' `
  -WorkingDirectory tricerules `
  -Executable cargo `
  -ArgumentList @('test', '--quiet')

# Ninja build through a child PowerShell process
$windowsPowerShell = "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe"
$buildScript = (Resolve-Path 'scripts\build-ninja.ps1').Path
./scripts/run-quiet-command.ps1 `
  -Label 'Windows Ninja build' `
  -Executable $windowsPowerShell `
  -ArgumentList @('-NoProfile', '-File', $buildScript)

# Full C++ tests
./scripts/run-quiet-command.ps1 `
  -Label 'C++ tests' `
  -Executable ctest `
  -ArgumentList @('--test-dir', 'build/windows-ninja-all', '--output-on-failure')
```

Use `-ShowLogOnSuccess` only when the successful output itself is required evidence. Test the runner with:

```powershell
./tests/scripts/run_quiet_command_test.ps1
```

## Final verification entry point

Choose the affected side after tracing the actual producers and consumers; the script does not
infer scope from filenames. From the repository root:

```powershell
./scripts/verify.ps1 -Side Rust
./scripts/verify.ps1 -Side Rust -CardData
./scripts/verify.ps1 -Side Cpp
./scripts/verify.ps1 -Side Both -CardData
./scripts/verify.ps1 -Side Both -CardData -Preview
```

The script locates the checkout from its own path, so invoking it by an absolute path from another
directory is also supported. Rust runs in `tricerules`; Ninja runs in a child Windows PowerShell;
CTest uses `build/windows-ninja-all`, rejects an empty suite, and requires ruled E2E prerequisites
with `RULED_E2E_REQUIRE=1`. The caller's E2E environment is restored after CTest.

Rust selects full tests, all-target Clippy with warnings denied, and format checking. Cpp selects
the full Ninja build and CTest. `-CardData` adds the read-only card check and requires Rust or Both.
Every selection ends with `git diff --check`. Preview prints argument arrays and working
directories without running commands or creating artifacts.

Each run retains logs and `summary.json` in a unique directory under `build/verification-logs`.
The summary records selected gates, commands, working directories, exit codes, and log paths.
The first failure prints its full log and stops the run with that exit code; remaining gates are
marked `NotRun`. Previous passing results are never reused. Missing sources or tools are failures,
not permission to omit a required gate. Diagnose unrelated baseline drift before changing it.

For card additions, explicitly refresh and review generated changes before final verification:

```powershell
./scripts/update-card-data.ps1 -Mode Refresh
./scripts/update-card-data.ps1 -Mode Check
```

Check is the default. It runs the canonical generator check and validates a temporary checklist,
then compares that checklist with `tricerules/CARDS.md`, ignoring only CRLF/LF differences. It
writes only build artifacts. Refresh updates existing generated RON and fingerprints, validates
the new checklist before replacing `CARDS.md`, then runs Check. Review all resulting changes;
neither command stages files, downloads sources, or enables `--include-new`.

Optional `-OracleBulk` and `-CardsXml` override the existing local source defaults. Relative paths
resolve from the repository root. Bulk metadata remains the adjacent `<input>.meta.json` and the
generator verifies its SHA. Failed checklist name validation preserves the old checklist; earlier
generated RON/fingerprint writes during Refresh remain available for review.

The legacy `gen-card-checklist.ps1 --check` validates names **and writes its output**. Use the new
Check entry point for verification that must leave tracked files untouched.

Workflow script regressions use isolated checkouts and native command fixtures:

```powershell
powershell.exe -NoProfile -File tests/scripts/run_quiet_command_test.ps1
powershell.exe -NoProfile -File tests/scripts/generator_wrapper_test.ps1
powershell.exe -NoProfile -File tests/scripts/card_data_wrapper_test.ps1
powershell.exe -NoProfile -File tests/scripts/update_card_data_test.ps1
powershell.exe -NoProfile -File tests/scripts/verify_workflow_test.ps1
```

Also run these with `pwsh.exe` when PowerShell 7 is available. After changing orchestration, run
the real combined gate once; fixture success alone does not establish the toolchain integration.

## Windows

Use the final entry point above for completion. For focused work or diagnosing a failed gate,
the underlying commands remain available. The Ninja script enters the VS x64 environment and
configures on first use:

```powershell
./scripts/build-ninja.ps1
./scripts/build-ninja.ps1 --target servatrice
ctest --test-dir build/windows-ninja-all --output-on-failure

cd tricerules
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Use the single-config Ninja tree without `-C` and without manually rewriting `PATH`. The vendored Qt kit is `6.6.3/msvc2019_64`. MSBuild presets remain for CI parity and Visual Studio use, but Ninja is the normal development loop.

If `Enter-VsDevShell` fails because both `Path` and `PATH` exist, invoke the VS environment through `cmd.exe` and `vcvars64.bat`. If a link fails because an exact Cockatrice executable is running, stop that process and rebuild; do not broaden the process kill.

## Affected-side matrix

| Change touches | Iteration build | Focused tests | Final gate |
|---|---|---|---|
| `tricerules/**/*.rs` or card RON only | No C++ build | Matching scenario or `tricerules-cards` registry test | Full Rust test, clippy, fmt; checklist for card data |
| `ruled_v1.proto` | Rust and C++ | Relevant Rust scenario, relay/client tests, E2E | Full Rust plus full Ninja build and CTest |
| Server or relay | `servatrice` and touched tests | `ruled_batch_test`, `ruled_utils_test`, `ruled_e2e_smoke_test` | Full affected build and CTest |
| Client only | `cockatrice` and touched tests | `ruled_client_test`, `game_prompt_widget_test`, and touched client test | Full affected build and CTest |
| Documentation only | None | Link/target and stale-reference searches | `git diff --check` |

`ruled_e2e_smoke_test` drives a real Servatrice and sidecar session. Run it after relay, protobuf, or ruled `server_game` changes and around extraction work. It skips when required binaries are absent; a skip is not proof of the end-to-end contract.

Before a commit, run the full gate for each affected side even when focused iteration stayed green. Report command exit codes in the final summary.
