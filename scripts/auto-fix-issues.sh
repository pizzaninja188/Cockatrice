#!/usr/bin/env bash
#
# auto-fix-issues.sh — unattended driver for the Cockatrice auto-fixer.
#
# Workflow (origin-authoritative):
#   You (elsewhere): edit issues.md, push to origin/master. Later: pull fix
#   branches, UI-test, merge them to master.
#   This box (cron): pull origin/master -> fix one unclaimed open issue at a time
#   on a fresh `claude -p` -> push fix/issue-N branch -> record status in
#   AUTOMATION_STATUS.md -> push that. When issues run out, generate no-new-logic
#   cards on branches. Runs until issues/cards run out or the usage window ends.
#
# Conflict-free by construction: on master the box ONLY ever commits
# AUTOMATION_STATUS.md; you only ever touch issues.md + code merges. Disjoint, so
# pull --rebase and your merges never collide.

set -uo pipefail

# ---- environment bootstrap (cron has a minimal PATH) ------------------------
export HOME="${HOME:-/home/ubuntu}"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"

# ---- config (override via env in the crontab if you like) -------------------
REPO="${REPO:-/home/ubuntu/Cockatrice}"
BASE_BRANCH="${BASE_BRANCH:-master}"
PROMPT_FILE="${PROMPT_FILE:-$REPO/.claude/automation/fix-one-issue.md}"
CARDS_PROMPT_FILE="${CARDS_PROMPT_FILE:-$REPO/.claude/automation/implement-cards.md}"
STATUS_FILE="${STATUS_FILE:-$REPO/AUTOMATION_STATUS.md}"
LOG_DIR="${LOG_DIR:-/home/ubuntu/cockatrice-auto-logs}"
CLAUDE_BIN="${CLAUDE_BIN:-/home/ubuntu/.local/bin/claude}"
MAX_ISSUES="${MAX_ISSUES:-100}"           # runaway-safety backstop on total tasks/run
                                          # (issues + cards). NOT the primary stop —
                                          # usage-window exhaustion ends the run first.
PER_ISSUE_TIMEOUT="${PER_ISSUE_TIMEOUT:-5400}"  # seconds (90 min) per task
RESUME_ENABLED="${RESUME_ENABLED:-1}"     # resume interrupted sessions w/ full context
RESUME_CAP="${RESUME_CAP:-3}"             # max attempts before an issue needs a human
CARDS_ENABLED="${CARDS_ENABLED:-1}"       # when no issues remain, implement no-new-logic cards
SYNC_REMOTE="${SYNC_REMOTE:-1}"           # 1 = pull/push origin/master each run (origin-authoritative)
# Card changes may only touch these paths ("no new logic" guardrail):
ALLOWED_CARD_PATHS='^(tricerules/tricerules-cards/data/|tricerules/CARDS\.md$|tricerules/tricerules-core/tests/scenario\.rs$)'
# CLAUDE_MODEL is read from the environment (set in the crontab).
# ----------------------------------------------------------------------------

mkdir -p "$LOG_DIR"
RUN_TS="$(date +%Y%m%d-%H%M%S)"
RUN_LOG="$LOG_DIR/run-$RUN_TS.log"

# Single-run lock.
LOCK="$LOG_DIR/.auto-fix.lock"
exec 9>"$LOCK"
if ! flock -n 9; then
  echo "Another auto-fix run is already in progress; exiting." >>"$RUN_LOG"
  exit 0
fi
exec > >(tee -a "$RUN_LOG") 2>&1

echo "================================================================"
echo "auto-fix run start: $(date)"
echo "repo=$REPO base=$BASE_BRANCH max=$MAX_ISSUES sync_remote=$SYNC_REMOTE"
echo "================================================================"

cd "$REPO" || { echo "FATAL: cannot cd to $REPO"; exit 1; }

model_args=()
[[ -n "${CLAUDE_MODEL:-}" ]] && model_args=(--model "$CLAUDE_MODEL")
PROMPT="$(cat "$PROMPT_FILE")"
PROMPT_CARDS=""
[[ -f "$CARDS_PROMPT_FILE" ]] && PROMPT_CARDS="$(cat "$CARDS_PROMPT_FILE")"

# --- helpers -----------------------------------------------------------------

# Push master, rebasing onto origin first if it advanced (your merges). The box
# only commits AUTOMATION_STATUS.md to master, so the rebase never conflicts with
# your issues.md / code changes.
push_master() {
  [[ "$SYNC_REMOTE" == "1" ]] || return 0
  timeout 120 git push origin "$BASE_BRANCH" >/dev/null 2>&1 && return 0
  echo "  master push non-ff; fetch + rebase + retry"
  timeout 120 git fetch origin "$BASE_BRANCH" >/dev/null 2>&1 || true
  if git pull --rebase origin "$BASE_BRANCH" >/dev/null 2>&1; then
    timeout 120 git push origin "$BASE_BRANCH" >/dev/null 2>&1 || echo "  push still failing; leaving commits local for next run"
  else
    git rebase --abort >/dev/null 2>&1 || true
    echo "  rebase failed; leaving commits local for next run"
  fi
}

# First resumable interrupted issue in the status file:
# status=in-progress, attempts < cap, has session. Prints "<id> <session>".
find_resumable_session() {
  [[ -f "$STATUS_FILE" ]] || return 0
  awk -v cap="$RESUME_CAP" '
    /^#[0-9]+ / {
      id=$1; sub(/^#/,"",id); st=""; att=0; sess="";
      for (i=2; i<=NF; i++) {
        if      ($i ~ /^status=/)   st=substr($i,8)
        else if ($i ~ /^attempts=/) att=substr($i,10)
        else if ($i ~ /^session=/)  sess=substr($i,9)
      }
      if (st=="in-progress" && (att+0)<cap && sess!="") { print id, sess; exit }
    }' "$STATUS_FILE"
}

# Mark in-review issues whose branch has merged into origin/master as done.
mark_done_merged() {
  [[ -f "$STATUS_FILE" ]] || return 0
  local id br changed=0
  while read -r id br; do
    [[ -z "$br" ]] && continue
    if git rev-parse --verify "origin/$br" >/dev/null 2>&1 \
       && git merge-base --is-ancestor "origin/$br" "origin/$BASE_BRANCH" 2>/dev/null; then
      sed -i "s|^#${id} status=in-review|#${id} status=done merged=$(date +%F)|" "$STATUS_FILE"
      echo "  #$id ($br) merged -> marked done"
      changed=1
    fi
  done < <(awk '/^#[0-9]+ / {
      id=$1; sub(/^#/,"",id); st=""; br="";
      for (i=2;i<=NF;i++){ if($i~/^status=/)st=substr($i,8); else if($i~/^branch=/)br=substr($i,8) }
      if (st=="in-review" && br!="") print id, br
    }' "$STATUS_FILE")
  if [[ $changed -eq 1 ]]; then
    git add "$STATUS_FILE" && git commit -q -m "automation: mark merged issues done" && push_master
  fi
}

# --- run-start sync ----------------------------------------------------------
if [[ "$SYNC_REMOTE" == "1" ]]; then
  timeout 120 git fetch origin >/dev/null 2>&1 || echo "fetch failed (offline?); proceeding on local $BASE_BRANCH"
fi
git checkout "$BASE_BRANCH" >/dev/null 2>&1 || { echo "FATAL: cannot checkout $BASE_BRANCH"; exit 1; }
if [[ "$SYNC_REMOTE" == "1" ]]; then
  git pull --rebase origin "$BASE_BRANCH" >/dev/null 2>&1 \
    || { git rebase --abort >/dev/null 2>&1 || true; echo "pull --rebase failed; proceeding on local $BASE_BRANCH"; }
fi
if [[ ! -f "$STATUS_FILE" ]]; then
  printf '# Automation status\n\nAuto-generated by scripts/auto-fix-issues.sh. Do not hand-edit.\n\n## Issues\n' > "$STATUS_FILE"
  git add "$STATUS_FILE" && git commit -q -m "automation: init status file" && push_master
fi
[[ "$SYNC_REMOTE" == "1" ]] && mark_done_merged

# A dirty tree here means a previous run left something behind (the per-task
# reconciliation should normally prevent this). Stash it to self-heal rather than
# block every future run; never discard.
if [[ -n "$(git status --porcelain)" ]]; then
  echo "WARNING: working tree dirty at start; stashing to unblock:"; git status --short
  git stash push -u -m "auto-fix start-stash $RUN_TS" >/dev/null 2>&1 || true
fi

# --- main loop ---------------------------------------------------------------
resolved=0
cards_added=0
mode="issues"

for (( i=1; i<=MAX_ISSUES; i++ )); do
  echo
  echo "---- iteration $i ($mode)  $(date '+%H:%M:%S') ----"

  git checkout "$BASE_BRANCH" >/dev/null 2>&1
  if [[ -n "$(git status --porcelain)" ]]; then
    echo "Base branch dirty before iteration $i; stopping."; git status --short; break
  fi

  TASK_LOG="$LOG_DIR/task-$RUN_TS-$(printf '%02d' "$i").log"
  rc=0

  if [[ "$mode" == "issues" ]]; then
    resume_id=""; resume_sid=""
    [[ "$RESUME_ENABLED" == "1" ]] && read -r resume_id resume_sid < <(find_resumable_session) || true

    if [[ -n "$resume_sid" ]]; then
      echo "resuming session $resume_sid for interrupted issue #$resume_id"
      timeout "$PER_ISSUE_TIMEOUT" "$CLAUDE_BIN" -p \
"Your previous session was interrupted (usage window ended). You were working issue #$resume_id on branch fix/issue-$resume_id. Re-check git state and continue to completion, following the workflow and output discipline you were given. Keep \`session=$resume_sid\` on that issue's line in AUTOMATION_STATUS.md. Your VERY LAST line of output must be exactly (bare text, no markdown): AUTOFIX_RESULT: RESOLVED #$resume_id  — or AUTOFIX_RESULT: BLOCKED #$resume_id - <reason> if stuck." \
          --resume "$resume_sid" "${model_args[@]}" \
          --dangerously-skip-permissions --output-format text >"$TASK_LOG" 2>&1
      rc=$?
      if [[ $rc -ne 0 ]] && grep -qiE "session.*(not found|does not exist|invalid|unknown|expired)|no such session|could not.*resume" "$TASK_LOG"; then
        echo "resume failed (stale session); falling back to a fresh agent."; resume_sid=""
      fi
    fi

    if [[ -z "$resume_sid" ]]; then
      new_sid="$(uuidgen)"
      echo "fresh issue agent, session $new_sid"
      timeout "$PER_ISSUE_TIMEOUT" "$CLAUDE_BIN" -p \
"$PROMPT

## Your session id
This run's session id is: $new_sid
When you create or update this issue's line in AUTOMATION_STATUS.md (claim OR
resume), set \`session=$new_sid\` so a future interrupted run can resume it." \
          --session-id "$new_sid" "${model_args[@]}" \
          --dangerously-skip-permissions --output-format text >"$TASK_LOG" 2>&1
      rc=$?
    fi
  else
    new_sid="$(uuidgen)"
    echo "fresh card agent, session $new_sid"
    timeout "$PER_ISSUE_TIMEOUT" "$CLAUDE_BIN" -p "$PROMPT_CARDS" \
        --session-id "$new_sid" "${model_args[@]}" \
        --dangerously-skip-permissions --output-format text >"$TASK_LOG" 2>&1
    rc=$?
  fi

  tail -n 40 "$TASK_LOG"

  # If the agent was interrupted (e.g. usage limit) mid-work, it leaves the tree
  # dirty. Preserve that WIP on its task branch so (a) the tree returns clean and
  # future runs aren't blocked, and (b) a resume continues from the committed
  # state. NEVER discard: stash anything that isn't on a task branch.
  agent_branch="$(git branch --show-current)"
  if [[ -n "$(git status --porcelain)" ]]; then
    cur="$agent_branch"
    if [[ "$cur" == fix/* || "$cur" == cards/* ]]; then
      git add -A && git commit -q -m "wip: auto-preserved (interrupted run)" \
        && echo "  preserved interrupted WIP on $cur"
      [[ "$SYNC_REMOTE" == "1" ]] && timeout 120 git push -u origin "$cur" >/dev/null 2>&1 || true
    else
      echo "  WARNING: dirty tree on '$cur' (not a task branch); stashing to unblock."
      git stash push -u -m "auto-fix stray $RUN_TS" >/dev/null 2>&1 || true
    fi
  fi

  # Flush any AUTOMATION_STATUS.md commits the agent made on master.
  git checkout "$BASE_BRANCH" >/dev/null 2>&1

  # Drift check: if the agent accidentally committed AUTOMATION_STATUS.md onto the
  # task branch instead of master, apply it here before pushing. This happens when
  # the agent bundles the status update with the code commit rather than switching
  # to master first.
  if [[ "$agent_branch" == fix/* || "$agent_branch" == cards/* ]]; then
    if git rev-parse --verify "$agent_branch" >/dev/null 2>&1; then
      branch_status="$(git show "$agent_branch:AUTOMATION_STATUS.md" 2>/dev/null || true)"
      master_status="$(cat "$STATUS_FILE" 2>/dev/null || true)"
      if [[ -n "$branch_status" && "$branch_status" != "$master_status" ]]; then
        echo "  WARNING: AUTOMATION_STATUS.md drifted onto $agent_branch; recovering to $BASE_BRANCH."
        printf '%s' "$branch_status" > "$STATUS_FILE"
        git add "$STATUS_FILE" && git commit -q -m "automation: recover status from $agent_branch (drift fix)"
      fi
    fi
  fi

  push_master

  if [[ $rc -eq 124 ]]; then echo "task hit ${PER_ISSUE_TIMEOUT}s timeout; stopping."; break; fi
  if [[ $rc -ne 0 ]]; then echo "claude exited non-zero ($rc) — usage window exhausted or crash. Stopping."; break; fi

  # Match anywhere on the line (not just column 0) and normalize from the keyword
  # onward, stripping any backticks an agent wrapped it in (a common markdown habit).
  # `tail -n 1` keeps the genuine final line from beating an earlier prose mention.
  # Accept both `AUTOFIX_RESULT:` and `AUTOFIX_RESULT ` — resumed agents sometimes
  # omit the colon and emit a status-file-style line instead of the sentinel.
  result_line="$(grep -aoE 'AUTOFIX_RESULT[: ].*' "$TASK_LOG" | tail -n 1 | tr -d '`')"
  echo "result: ${result_line:-<none>}"

  if [[ "$mode" == "issues" ]]; then
    case "$result_line" in
      # `status=in-review` is the fallback when a resumed agent prints the
      # AUTOMATION_STATUS.md line format instead of the proper sentinel.
      *"RESOLVED"*|*"status=in-review"*) resolved=$(( resolved + 1 )); echo "issue resolved; continuing." ;;
      *"NO_ELIGIBLE_ISSUES"*)
        if [[ "$CARDS_ENABLED" == "1" && -n "$PROMPT_CARDS" ]]; then
          echo "no eligible issues; switching to card mode."; mode="cards"
        else echo "no eligible issues; stopping."; break; fi ;;
      *"BLOCKED #"*)       echo "issue parked as blocked/needs-human; moving on." ;;
      *"BLOCKED"*)         echo "structural BLOCKED (no id); stopping."; break ;;
      *)                   echo "no clear result; stopping."; break ;;
    esac
  else
    case "$result_line" in
      *"CARD_ADDED cards/"*)
        branch="${result_line##*CARD_ADDED }"; branch="${branch%% *}"
        bad="$(git diff --name-only "$BASE_BRANCH".."$branch" 2>/dev/null | grep -vE "$ALLOWED_CARD_PATHS" || true)"
        [[ -n "$bad" ]] && { echo "WARNING: $branch touched non-allowed paths (needs review):"; echo "$bad"; }
        cards_added=$(( cards_added + 1 )); echo "card branch $branch added; continuing." ;;
      *"CARD_NONE"*)       echo "no no-new-logic cards left; stopping."; break ;;
      *"CARD_BLOCKED"*)    echo "card attempt blocked; moving on." ;;
      *)                   echo "no clear result; stopping."; break ;;
    esac
  fi
done

git checkout "$BASE_BRANCH" >/dev/null 2>&1
push_master
echo
echo "================================================================"
echo "auto-fix run end: $(date)"
echo "  issues resolved: $resolved    card branches added: $cards_added"
echo "  fix branches:";  git branch --list 'fix/issue-*' | sed 's/^/    /'
echo "  card branches:"; git branch --list 'cards/*'     | sed 's/^/    /'
echo "================================================================"
