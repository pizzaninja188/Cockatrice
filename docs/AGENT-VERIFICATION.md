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

## Windows

The Ninja script enters the VS x64 environment and configures on first use:

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
