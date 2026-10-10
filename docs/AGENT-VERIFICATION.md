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

Rust checks the production card-data dependency boundary, then selects full tests, all-target
Clippy with warnings denied, and a separate format check for every package declared in the
workspace. `check-rust-format.ps1` reads Cargo's target inventory
and runs check-only rustfmt with each target's edition in bounded argument chunks. This preserves
formatting coverage when even one package exceeds Windows command-line limits. Package discovery
is read-only during Preview; target discovery runs only during the selected check. Cpp selects
the full Ninja build and CTest. `-CardData` adds the read-only card check and requires Rust or Both.

For a card-data build-boundary refactor, run
`tests/scripts/card_data_incremental_build_test.ps1` with no competing writers. It performs a
warm baseline build, temporarily appends whitespace to one RON file and the presentation
fingerprint table, and checks Cargo's library artifact freshness. It restores each file's exact
bytes in `finally`, rebuilds restored inputs, and retains timings and Cargo logs. This is an
explicit mutation test, not part of ordinary final verification.

Every real run starts with `check-verification-environment.ps1`. It requires at least 20 GiB
free on the repository, temporary-directory, and Cargo-target volumes, and probes Windows
path canonicalization in the checkout and temporary directory. Replay uses this capability
to reject capture paths that escape their root. Some host sandboxes permit file IO but deny
canonicalization; retry through the supported approval route rather than weakening replay
validation. A failed preflight retains a normal failure log and summary and stops all later gates.
It never deletes artifacts or changes permissions. `CARGO_TARGET_DIR` overrides are included;
relative overrides resolve from `tricerules`. The capacity check is a minimum headroom check,
not a guarantee that an arbitrary build will fit. Standalone preflight accepts `-MinimumFreeGiB`.

The workspace development profile (also inherited by tests) uses line-level debug information
and disables incremental compilation. This reduces artifact growth across long engine-authoring
campaigns and avoids the host's failed incremental-cache hard links. Release settings are unchanged.
Explicit Cargo profile/environment overrides still take precedence. After changing profiles,
old artifacts remain on disk: with no competing builds, verify the target path and use
`cargo clean --profile dev` from `tricerules` to reclaim disposable debug output. Keep campaign
checkpoints and verification logs outside that target directory; do not automatically clean them.

Every selection ends with `git diff --check`. Preview prints argument arrays and working
directories without running commands or creating artifacts.
With `-CardData`, the complete read-only card check follows preflight, before the full suites.
It rejects invalid evidence/presentation metadata and generated-data drift early; its own
referenced-target compilation/listing still runs. All affected-side tests and lint remain
mandatory afterward. Do not add a second routine standalone Check before this final gate.

Each run retains logs and `summary.json` in a unique directory under `build/verification-logs`.
The summary records selected gates, commands, working directories, exit codes, and log paths.
The first failure prints its full log and stops the run with that exit code; remaining gates are
marked `NotRun`. Each invocation runs all selected gates; the script does not cache prior results.
For later delivery, reuse passing final evidence when tested content, dependencies, and the
relevant environment remain unchanged. Rerun affected gates when changes, failures, or unresolved
concerns invalidate that evidence. A commit request alone does not require another invocation.
Missing sources or tools are failures,
not permission to omit a required gate. Diagnose unrelated baseline drift before changing it.

For handwritten card additions, run the batch preparation command after focused semantic green.
It checks formatting, canonical IDs, generic registry conformance, optional exact actual-card
tests, authoring-feature lint, and evidence structure/test references before refreshing metadata.
It stops on failure (including an empty exact test selection) and retains the command logs:

```powershell
./scripts/prepare-card-batch.ps1
# Optional focused.json is an array of {package,target,test,features?}; tests use exact names.
./scripts/prepare-card-batch.ps1 -FocusedTests build/batch/focused.json `
  -ReviewMapPath tricerules/tricerules-cards/authoring/review-maps/example.json
# Review the prepared diff and obtain independent semantic approval, then:
./scripts/verify.ps1 -Side Rust -CardData
```

Preparation honors ambient/default Cargo jobs and test threads (`-Workers 1..192` overrides), and
restores the caller's environment afterward. It accepts the same local `-OracleBulk` and `-CardsXml`
overrides as Refresh. It checks formatting without editing code, runs specified scenarios, and
checks unconfirmed maps structurally through inspect-only packets; this does not approve maps or replace
the final gate. When the exact prechecks already passed on unchanged content, use the underlying
`update-card-data.ps1 -Mode Refresh -MetadataOnly` directly rather than repeating them. Preparation's
optional map subset is never accepted by the final evidence gate, which checks every map and
requires independent review confirmation. Freeze a compatible ready batch after preparation and
review it together; run one final affected-side gate on the stable combined batch. Additions to an
already verified batch invalidate its evidence, so deliver it before starting the next increment.

## Engine-capability preparation

After intended red/green and relevant stabilization, supply exact selected regressions:

```powershell
./scripts/prepare-engine-batch.ps1 -FocusedTests build/capability/focused.json `
  -OutDirectory build/capability/prepared `
  -Path @('tricerules/tricerules-core/src/engine/replacement.rs',
          'tricerules/tricerules-core/tests/scenario/entry_copy_auras.rs') `
  -Evidence @('build/capability/contract.md', 'build/capability/red.log')
```

The JSON array uses `{package,target,test,features?}`; `target: "lib"` selects library tests,
otherwise it names an integration target. Empty, duplicate, unknown-field and zero-execution
selections fail closed. Preparation runs formatting, each exact non-ignored regression, and
all-target lint for selected packages with default and specified feature configurations.
It honors ambient workers unless explicitly overridden. No card metadata is refreshed.

The new output directory retains logs, exit codes and command timestamps in `summary.json`.
With explicit `-Path`, its `review/` subdirectory contains the existing workbench's frozen patch,
source copies, supplied evidence, test plan and preparation logs. Run workbench `check` to verify
freshness before review. Without `-Path`, it only checks and retains evidence. Both modes leave
semantic approval false and the final gate pending. This is not red evidence, design review,
package stabilization, semantic approval or final affected-side verification. After independent
frozen-patch review, run the normal full gate on unchanged content. Reuse passing preparation
instead of executing it again solely to assemble another review packet.

## Worker example

For a four-worker cap on all commands in the current PowerShell session, including final gates:

Campaign overrides, including ambient/default workers, take precedence over this example.

```powershell
$env:CARGO_BUILD_JOBS = '4'
$env:RUST_TEST_THREADS = '4'
$env:CMAKE_BUILD_PARALLEL_LEVEL = '4'
$env:CTEST_PARALLEL_LEVEL = '4'
```

Check is the default update mode. It first validates checked-in direct-RON maps and exact non-ignored
test references with `scripts/check-card-evidence.ps1`. The checker builds referenced test targets
once per package using Cargo JSON artifacts, then lists normal and ignored tests directly from
those exact executables. It never searches for old binaries or persists a test inventory cache;
missing or ambiguous artifacts and failed listings fail the gate. Listing is not execution evidence: the full
Rust suite must also pass on the same content. It runs the generator check and validates a temporary checklist,
then compares that checklist with `tricerules/CARDS.md`, ignoring only CRLF/LF differences. It
writes only build artifacts. For handwritten cards, Refresh with `-MetadataOnly` updates fingerprints
without evaluating recipes or rewriting generated RON, then validates and replaces `CARDS.md`.
Omit `-MetadataOnly` when generator recipes or generated card output need refreshing.
Refresh is preparation only: it does not repeat Check or validate semantic evidence. Review its
diff and run the final gate, which includes the full Check regardless of refresh mode. Do not
also run a standalone Check routinely. Neither command stages files, downloads sources, or
enables `--include-new`. `-MetadataOnly` is rejected with Check; it cannot narrow the final gate.

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
powershell.exe -NoProfile -File tests/scripts/prepare_card_batch_test.ps1
powershell.exe -NoProfile -File tests/scripts/card_evidence_test.ps1
powershell.exe -NoProfile -File tests/scripts/card_evidence_workflow_test.ps1
powershell.exe -NoProfile -File tests/scripts/verify_workflow_test.ps1
powershell.exe -NoProfile -File tests/scripts/verification_environment_test.ps1
powershell.exe -NoProfile -File tests/scripts/rust_format_workflow_test.ps1
powershell.exe -NoProfile -File tests/scripts/launch_ruled_game_test.ps1
```

Also run these with `pwsh.exe` when PowerShell 7 is available. After changing orchestration, run
the real combined gate once; fixture success alone does not establish the toolchain integration.

The capability index checker is opt-in documentation tooling. It is not called by `verify.ps1` or
CardData Check, and capability-index documentation changes do not require the full Rust gates. For
checker changes, run its isolated regression suite under both available PowerShell versions:

```powershell
powershell.exe -NoProfile -File tests/scripts/capability_index_test.ps1
pwsh.exe -NoProfile -File tests/scripts/capability_index_test.ps1
```

Run the checker separately when reviewing index edits:

```powershell
powershell.exe -NoProfile -File scripts/check-capability-index.ps1
powershell.exe -NoProfile -File scripts/check-capability-index.ps1 -CheckFreshness
```

It performs local structural and lexical reference checks only; it does not build, execute tests,
or decide semantic support. Freshness checks are opt-in and limited to each entry's **Reviewed
paths**. Documentation-only changes still require link/target review and `git diff --check`.

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
# From the repository root; bounded target coverage on Windows:
cd ..
./scripts/check-rust-format.ps1 -Package tricerules-proto
./scripts/check-rust-format.ps1 -Package tricerules-core
./scripts/check-rust-format.ps1 -Package tricerules-cards
./scripts/check-rust-format.ps1 -Package tricerules-server
```

Use the single-config Ninja tree without `-C` and without manually rewriting `PATH`. The vendored Qt kit is `6.6.3/msvc2019_64`. MSBuild presets remain for CI parity and Visual Studio use, but Ninja is the normal development loop.

If `Enter-VsDevShell` fails because both `Path` and `PATH` exist, invoke the VS environment through `cmd.exe` and `vcvars64.bat`. If a link fails because an exact Cockatrice executable is running, stop that process and rebuild; do not broaden the process kill.

## Affected-side matrix

### Offline authoring tooling

The [authoring workbench](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#offline-authoring-workbench)
is optional development tooling. Production constructors still load embedded data. After changing
its Rust implementation, run focused tests with `--features authoring`, feature-enabled Clippy,
and the normal full Rust/CardData gate. Default gates do not enable the draft constructor.

```powershell
# Run from tricerules through run-quiet-command.ps1:
cargo test -p tricerules-cards --features authoring --lib authoring::tests
cargo test -p tricerules-core --features authoring --test scenario semantic_fixtures
cargo test -p tricerules-core --features authoring --test scenario authoring_drafts::draft_constructor
cargo clippy -p tricerules-core -p tricerules-cards --features authoring --all-targets -- -D warnings
cargo build -p tricerules-cards --features authoring --bin card-author

# Run from the repository root through the quiet runner:
python tests/scripts/card_author_workflow_test.py tricerules/target/debug/card-author.exe
python tests/scripts/authoring_batch_test.py
powershell.exe -NoProfile -File tests/scripts/draft_card_workflow_test.ps1
```

Repeat the draft wrapper regression with `pwsh.exe` when available. The loop test retains its
synthetic draft/manifest and logs under build, proves a changed effect fails old expectations,
then passes corrected independent expectations without changing the test executable hash or mtime.
It does not admit those synthetic cards. Feature-only execution and embedded final verification
are separate evidence; both are required for changes to this tooling.

The batch workbench also requires actual `prepare`, `validate-batch`, assessment `preflight`,
and freeze/check integration with the built CLI. Source packets and structural preflight are not
semantic approval; retain evidence of actual row execution. `authoring_schema` is shared between
the early checker and default/feature-enabled scenario rows; exercise `scenario semantic_fixtures`
under both configurations. These tools honor ambient worker settings.

### Parameterized card evidence

Follow the [semantic evidence contract](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#reusable-semantic-evidence)
when reusing scenario fixtures. Every required card row must execute accepted commands, finish
resolution and choices within an explicit bound, and assert independently reviewed results.
Report exercised, N/A with a reason, or fixture-blocked; a skipped/rejected action or parked
resolution is not passing semantic evidence. A drain result alone proves only completion.
Keep primitive, composition, per-card mapping and dedicated interaction coverage distinct.
Applicable illegal paths still need coverage; untargeted effects do not need artificial target
tests. Complete-definition review remains required. New successful generic conformance cases
need no baseline rows; unclassified fixture gaps and changes to existing reviewed rows still fail.

For the reusable fixture pilot, run focused `scenario semantic_fixtures`, the ported Divination
and Pawpatch scenarios, and the `issue_448_semantic_mapping` and `issue_412_modal_mode_registry`
registry binaries through the quiet runner. Then run generator tests with
`cargo test --features gencards -p tricerules-cards --bin gen-cards` and finish with
`./scripts/verify.ps1 -Side Rust -CardData`. Generator implementation changes additionally need
feature-enabled Clippy. Shared fixture reuse does not reduce the final affected-side gates.

| Change touches | Iteration build | Focused tests | Final gate |
|---|---|---|---|
| `tricerules/**/*.rs` or card RON only | No C++ build | Matching scenario or `tricerules-cards` registry test | Full Rust test, clippy, fmt; checklist for card data |
| `ruled_v1.proto` | Rust and C++ | Relevant Rust scenario, relay/client tests, E2E | Full Rust plus full Ninja build and CTest |
| Server or relay | `servatrice` and touched tests | `ruled_batch_test`, `ruled_utils_test`, `ruled_e2e_smoke_test` | Full affected build and CTest |
| Client only | `cockatrice` and touched tests | `ruled_client_test`, `game_prompt_widget_test`, and touched client test | Full affected build and CTest |
| Documentation only | None | Link/target and stale-reference searches | `git diff --check` |

`ruled_e2e_smoke_test` drives a real Servatrice and sidecar session. Run it after relay, protobuf, or ruled `server_game` changes and around extraction work. It skips when required binaries are absent; a skip is not proof of the end-to-end contract.

Before a commit, ensure passing full-gate evidence for each affected side, even when focused iteration stayed green. Reuse valid final evidence under the rule above; otherwise run the affected full gates. Report command exit codes in the final summary.
