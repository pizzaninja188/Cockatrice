# Headless Linux development

The engine, authoring CLI, Servatrice and automated client tests can run without
a desktop session. Full C++ verification builds the Qt client and runs widget and
replay tests on Qt's offscreen platform. Windows clients can connect to a Linux
server for visible GUI acceptance.

## Toolchain

Use PowerShell 7 (`pwsh`), Rust stable with Clippy/rustfmt, Python 3, Git, CMake 3.23 or newer,
Ninja, and the Qt/development packages in
[the Ubuntu 24.04 CI image](../.ci/Ubuntu24.04/Dockerfile).
Use native toolchains and Qt plugins for the host architecture. PowerShell's
[official releases](https://github.com/PowerShell/PowerShell/releases) provide Linux
ARM64 and x86-64 archives. Verify the matching archive's published checksum.

Install the Ubuntu packages used by the CI image.

```sh
sudo apt-get update
sudo apt-get install build-essential ca-certificates cmake git ninja-build \
  libcurl4-openssl-dev libgl-dev liblzma-dev libmariadb-dev-compat libprotobuf-dev \
  libssl-dev protobuf-compiler qt6-declarative-dev qt6-image-formats-plugins \
  qt6-l10n-tools qt6-multimedia-dev qt6-shadertools-dev qt6-svg-dev qt6-tools-dev \
  qt6-tools-dev-tools qt6-websockets-dev libqt6sql6-mysql
rustup component add clippy rustfmt
```

Limit concurrency to available memory, especially on a host running other services.

```powershell
$env:CARGO_BUILD_JOBS = '2'
$env:CMAKE_BUILD_PARALLEL_LEVEL = '2'
$env:RUST_TEST_THREADS = '2'
```

## Move local inputs and evidence

Pull committed source into a clean checkout, preserving existing edits first.
Build native Linux artifacts instead of copying Windows build or Cargo target files.

Git excludes the Oracle bulk snapshot, its adjacent `.meta.json`, and build/evidence
directories. Copy the existing `oracle-cards.jsonl.gz` and
`oracle-cards.jsonl.gz.meta.json` together, plus the exact `cards.xml` used by the
Windows checks. Keep their bytes unchanged. Downloading or refreshing datasets
requires separate authorization. The default Linux XML location is
`${XDG_DATA_HOME:-$HOME/.local/share}/Cockatrice/Cockatrice/cards.xml`.
Use explicit source arguments to select paths outside the profile defaults.

Preserve needed campaign receipts, source packets and acceptance notes from
`build/deck-coverage` and `build/verification-logs` separately. Old logs remain
historical evidence. Record routine RON baselines afresh on Linux because the
paths, toolchain, environment and build commands have changed. Configure Git/GitHub
credentials separately on the Linux host.

## Verification

Run these commands from the repository root.

```sh
pwsh -NoProfile -File scripts/verify.ps1 -Side Both -Preview
pwsh -NoProfile -File scripts/verify.ps1 -Side Both -CardData \
  -OracleBulk /path/to/oracle-cards.jsonl.gz -CardsXml /path/to/cards.xml
```

The shared gate retains logs and exact failure codes. Linux C++ verification
configures and builds `linux-ninja-all`, then runs all CTest entries with
`QT_QPA_PLATFORM=offscreen` and `RULED_E2E_REQUIRE=1`. Missing server/sidecar
prerequisites fail instead of silently establishing E2E success through a skip.
The preset also supplies offscreen Qt during build-time test discovery.

Use the preset for focused C++ work too.

```sh
pwsh -NoProfile -File scripts/build-linux.ps1 --target ruled_client_test
ctest --preset linux-ninja-all -R ruled_client_test
```

Run the headless workflow over SSH. Use the Windows launch and capture helpers
for Windows sessions, and retain Windows build and packaging coverage for Windows delivery.
Use the explicit Linux gate for ruled E2E evidence. The ordinary Linux desktop CI
configuration can skip those tests when the Rust sidecar is absent.

The default preflight requires a 10 GiB free-space reserve to guard against
disk exhaustion during a build. It checks the repository,
temporary directory and Cargo target filesystem, plus native path canonicalization.
A low-space failure is environment evidence. Keep evidence and unrelated service
data intact. After measuring build usage on the host, pass `-MinimumFreeGiB` with an
explicit reserve if needed for later runs. Native path canonicalization and the other
verification checks still run.
The default is informed by a Windows build measurement, not a Linux capacity guarantee;
fresh C++ builds and different toolchains may need a larger reserve. See the
[measurement and limits](AGENT-VERIFICATION.md#final-verification-entry-point).

Explicit `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_INCREMENTAL=0` trade debug information
and incremental rebuild speed for smaller Rust artifacts. Changing these settings
invalidates previous verification baselines.

## Headless processes

Build with the full preset first. The sidecar listens on loopback, so run
`./tricerules/target/release/tricerules-server` on the same host as Servatrice. In another terminal,
run `./build/linux-ninja-all/servatrice/servatrice --config /path/to/servatrice.ini`.
Choose bind address, authentication and access rules deliberately before allowing
remote clients. The normal `scripts/launch-ruled-game.ps1 -Dev` remains the Windows
two-client GUI launcher.

No MTG rules surface area.
