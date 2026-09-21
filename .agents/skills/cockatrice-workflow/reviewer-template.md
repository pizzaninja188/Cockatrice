# Card-authoring reviewer delegation

The root fills the fields below and includes this contract in every initial or follow-up
card-authoring review delegation. This template does not itself authorize spawning agents.
Use the host's permitted delegation mechanism and the campaign's configured model/effort.

## Scope supplied by the root

- Review type: initial review or material delta review.
- Repository, branch, base SHA and current HEAD.
- Owning issue(s), exact Oracle identities, changed paths, frozen patch path and SHA-256.
- Pinned source/rulings, typed mappings and relevant validators/runtime consumers.
- Existing packet/map paths, focused test logs, exit codes, final-gate status, and known risks.
- For a delta: prior reviewed patch/hash, prior verdict, exact changed paths/hunks, findings
  addressed, and which evidence remains valid or requires the root to rerun it. Preserve both
  patch versions; do not overwrite the prior artifact and ask the reviewer to reconstruct it.

## Instructions to the reviewer

You are an independent inspection-only reviewer. Inspect the frozen scope and its actual evidence.
Verify patch identity before drawing conclusions; report a mismatch rather than reviewing a moving
worktree. Trace Oracle semantics through the actual definitions, validators, consumers and tests.
Check that tests exercise the named cards and assert independently reviewed outcomes, including
applicable illegal paths, player scope, identity, timing and hidden-information behavior.

Allowed operations: read existing files and logs, search source, inspect Git status/diffs/history,
and compute hashes without writing files. You may inspect supplied source/rulings or perform
read-only source lookups. Do not mutate files, the index, GitHub issues or other external state.

Do NOT run Cargo in any mode, tests or test executables, builds, lint/format commands, verification
scripts, generators, metadata Refresh or Check, or review-packet generation. Do not redirect output
to files or apply/reverse-apply patches. These restrictions include commands described as
"read-only checks" that write logs, caches, build outputs or temporary files. Do not commit or push.
If another command is needed, give the root the exact command and the question it must resolve.

Assess root-supplied logs for matching scope/content and actual exit codes. State that execution
evidence was inspected, not independently reproduced. A final gate intentionally scheduled after
review is a pending root delivery requirement, not by itself a reason for another review cycle.
Do not approve delivery without required verification; distinguish semantic review from gate status.

For delta review, inspect changed hunks and their affected interactions. Reuse the prior review;
do not restart whole-batch review unless the delta invalidates its assumptions. Do not broaden
scope for optional improvements. Do not fix findings yourself.

## Required concise response

1. Verdict: approved for reviewed semantics, changes required, or unable to assess (with reason).
2. Blocking correctness defects: file/line, concrete failure, and smallest required correction.
3. Missing required evidence: exact unproven behavior or invalid evidence, and requested root action.
   Separately identify any pending final gate; do not mislabel its planned absence as a code defect.
4. Optional improvements: explicitly non-blocking; use "none" where appropriate.
5. Evidence inspected and limits: patch hash, logs/exit codes inspected, remaining delivery checks.

Do not convert preference, redundant coverage, stylistic cleanup, or inability to personally rerun
tests into a blocking finding. Conversely, an unsupported claim or untested required behavior
remains a required evidence gap even when production code appears correct.
