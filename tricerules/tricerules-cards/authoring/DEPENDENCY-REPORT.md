# Complete-identity dependency reports (v1)

The candidate report remains v1 and unchanged. It groups unsupported clauses, not complete
playable cards. The separate dependency report inventories **every** Oracle identity in the
pinned input, including non-rules failures, handwritten definitions and partial definitions.
Neither report changes generation eligibility, card data, fingerprints, or issues.

From the repository root, with the existing pinned bulk and adjacent SHA-verified metadata:

```powershell
./scripts/gen-cards.ps1 --dependency-report build/dependencies.json
./scripts/gen-cards.ps1 --dependency-report build/dependencies-reviewed.json `
  --dependency-evidence build/reviewed-evidence.json `
  --dependency-issues build/issues-current.json
```

Use a **new** `.json` destination under `build/` or outside the repository. The parent directory
must exist; overwrites and destinations elsewhere inside the repository are refused. This mode
cannot be combined with generation, check, presentation audit, scaffolding, candidate-report,
target-name filtering, output-directory overrides, or Oracle Tags. Wrapper contracts are unchanged.

Always invoke through Cargo/the wrapper after editing source: the report uses the compiled
registry and generator. Do not use an old binary against new source. Do not edit source or input
files concurrently with a report run.

## What the report means

- `snapshot` binds the compressed source SHA, metadata SHA, Git revision, actual source-content
  hash (including dirty/untracked source), and optional issue-snapshot SHA. Legality is exactly
  `legalities.standard == "legal"` in that source, **not current live Standard legality**.
- `identities` are ordered by Oracle ID. Reprints deduplicate; conflicting characteristics,
  text, or legality for one identity fail the command. Duplicate whole-card names across distinct
  IDs cannot acquire registry/full-support claims. Face indices are one-based and are not engine
  object IDs. Each face retains its original text. Clause units preserve original nonblank lines
  and half-open UTF-8 byte spans within that text. A line may contain several functional sentences;
  the reviewer must cover all of them. No prose is interpreted into executable mechanics.
- `observations` records exact recipe IDs and diagnostic Rust emission descriptions. These are
  independent probes, **not** evidence of composition or runtime support. The production evaluator
  alone supplies `generator_eligible`, its first failure, and successful recipe labels. Aggregate
  probes cover whole-face assemblies; failed probes stay unclassified. The diagnostic emission
  string is for inspection, not a stable mechanics interchange format.
- `registry` reports absent/generated/handwritten status and existing `partial-cards.tsv` notes.
  `declared_full` means registered with no partial note. `verified_registered_full` additionally
  requires complete reviewed evidence with no remaining requirements. Registry presence, a recipe
  name, and an effect enum name are never semantic proofs by themselves.
- Required review scopes are `card`, `presentation`, every `face_assembly`, and every `clause`.
  Card review covers layout, characteristics, costs, target binding, result references, and
  cross-face relationships. Face review covers ordering, aggregate allowlists and assembly.
  Presentation review covers every emitted ability/choice and its client consumer.
- Missing scopes/evidence, unknown requirements, and ambiguous matches keep `analysis_complete`
  false. Partial-card notes cannot be waived with an empty dependency set. Such cards appear in
  `incomplete_analysis` and contribute **zero** projected unlocks.
- `full_corpus` and `standard` contain identity lists, exclusions, already verified cards,
  fully reviewed but unregistered zero-gap cards, and projected opportunities. A projection counts
  an identity exactly once only if **all** its remaining verified requirements are covered.
  Shared clauses/requirements never multiply counts. Singles are ranked together with bounded
  pairs: observed two-gap sets and pairs with standalone unlocks. Pairs that merely reproduce one
  single's unlocks are omitted. Ties use typed requirement ordering. No triples or inferred
  throughput are reported. Capability `effort_estimate` is an optional supplied estimate, never a
  measured rate. Counts stay **projected** until generation, registry and semantic validation pass.

## Reviewed analysis input

Evidence is an optional, strict versioned JSON snapshot. Keep session-specific reviews and issue
exports under `build/` or outside the repository; they are not a second campaign tracker. The
generator never reads them in any generation mode. Copy `snapshot` and `identity_sha256` from a
fresh inventory, then review the mechanics and code; copying hashes alone does not constitute a
review. Do not mass-fill empty assessments to make counts increase.

The input has exactly these top-level fields:

```json
{
  "format_version": 1,
  "snapshot": {
    "source_sha256": "from inventory",
    "metadata_sha256": "from inventory",
    "code_revision": "from inventory",
    "code_sha256": "from inventory",
    "issue_snapshot_sha256": null
  },
  "capabilities": [
    {
      "id": {"kind": "runtime_capability", "key": "reviewed-specific-gap"},
      "deliverable": "Exact bounded behavior, including all required dependent work",
      "issue_urls": [],
      "effort_estimate": null
    }
  ],
  "cards": [
    {
      "oracle_id": "from source",
      "identity_sha256": "from inventory",
      "assessments": [
        {
          "scope": {"scope": "clause", "face": 1, "line": 2},
          "remaining": [{"kind": "runtime_capability", "key": "reviewed-specific-gap"}],
          "evidence": {
            "emitter": {"path": "repository-relative file", "anchor": "exact symbol or excerpt"},
            "validator": {"path": "repository-relative file", "anchor": "exact symbol or excerpt"},
            "engine_consumer": {"path": "repository-relative file", "anchor": "exact symbol or excerpt"},
            "tests": [{"path": "repository-relative file", "anchor": "exact test symbol"}],
            "player_scope": "Who chooses, owns, controls and receives the effect",
            "rationale": "Why the evidence proves support or this precise gap; include interactions"
          }
        }
      ]
    }
  ]
}
```

Typed identifier kinds are `recognition`, `generator_composition`, `runtime_capability`,
`presentation_client_capability`, and `unclassified`. The key identifies one explicitly defined
bounded capability, not a free-form runtime effect. A missing recipe does not determine its kind.
Unclassified identifiers never unlock cards. Empty `remaining` means **reviewed as satisfied**;
missing/`null` evidence means unresolved. For a gap, evidence must trace the closest emitter,
validator, actual consumer and tests and explain why they cannot express the required behavior.
For supported behavior, cite actual semantic coverage, not only catalog tests. Missing semantic
coverage is itself an unresolved requirement. Grouping a deliverable is allowed only when its
scope explicitly includes the required recipe, assembly, presentation, data and validation work;
never count delivery of a primitive as delivery of those other requirements.

Scopes use `{"scope":"card"}`, `{"scope":"presentation"}`,
`{"scope":"face_assembly","face":1}` or `{"scope":"clause","face":1,"line":2}`.
All fields are strict: unknown fields, duplicate reviews/scopes/capabilities, undefined requirement
IDs, stale identity hashes or snapshot bindings fail the report. Evidence anchors must exist in
repository files. These checks detect drift, not the truth of a human semantic review.

For issue mappings, export the current relevant issues with `gh issue view N --repo
pizzaninja188/Cockatrice --json number,url,title,body,state,updatedAt,comments`, combine those objects
into one JSON array, and pass it with `--dependency-issues` on both inventory and reviewed runs.
Mapped URLs must exist in that snapshot. Refresh it before each analysis/delivery; any changed
source, code, metadata or issue snapshot invalidates the review. Reports intentionally make no
network requests and cannot detect an unrefreshed external issue export. The raw evidence file's
SHA is included in the output. Identical pinned inputs, code and reviewed evidence produce
identically ordered bytes.

## Regression provenance and verification

`src/bin/gen_cards/fixtures/issue_428_dependencies.json` retains six source records from pinned
Scryfall `oracle_cards` snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`, compressed SHA
`9611b5d93b20478a0ee46bae8b20a9eb39ee980f0ef4f5f6f6aaa8f7ab010ab2`. The fixture preserves exact
Oracle identity, characteristics, clauses, legality and research URLs, not historical support
status. Tests re-evaluate them and query the current registry; #444/#445/#446 are analysis inputs,
not hardcoded ownership or eligibility rules.

```powershell
./scripts/run-quiet-command.ps1 -Label 'Dependency report tests' -WorkingDirectory tricerules `
  -Executable cargo -ArgumentList @('test','--features','gencards','-p','tricerules-cards','--bin','gen-cards')
./scripts/run-quiet-command.ps1 -Label 'Generator Clippy' -WorkingDirectory tricerules `
  -Executable cargo -ArgumentList @('clippy','--features','gencards','-p','tricerules-cards','--all-targets')
./scripts/verify.ps1 -Side Rust -CardData
```

Interaction checklist: authority is offline analysis only; Oracle IDs remain separate from runtime
identity; timing/target/result interactions require explicit evidence; unknown/ambiguous paths
fail closed; all input is public source data; only report consumers change. Runtime schemas,
protobuf, Servatrice, Qt, freeform, automated network E2E and manual two-client checks are N/A.
No new MTG rules surface area. This tooling neither implements nor closes any card blocker.
