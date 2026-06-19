#!/usr/bin/env bash
#
# auto-fix-issues.sh — unattended "morning" driver for issues.md.
#
# Solves eligible Open issues ONE AT A TIME, each in a FRESH `claude -p` process
# (clean context, not a growing subagent), back to back, until one of:
#   - no eligible issues remain,
#   - the agent reports it is blocked,
#   - the Claude usage window is exhausted (claude exits non-zero), or
#   - the MAX_ISSUES safety cap is hit.
#
# Intended to be run from cron at 08:00 and 13:00 America/Chicago. Each run gets
# (roughly) its own 5-hour usage window.
#
# Everything stays LOCAL except feature branches: the agent commits a tracking
# "claim" to local master and pushes only `fix/issue-N` branches. Review/test the
# branches in the evening; merge the good ones.

set -uo pipefail

# ---- environment bootstrap (cron has a minimal PATH) ------------------------
# cron runs with PATH=/usr/bin:/bin and may not set HOME, but the build needs
# cargo (~/.cargo/bin) and claude (~/.local/bin). Make this self-sufficient.
export HOME="${HOME:-/home/ubuntu}"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"

# ---- config (override via env in the crontab if you like) -------------------
REPO="${REPO:-/home/ubuntu/Cockatrice}"
BASE_BRANCH="${BASE_BRANCH:-master}"
PROMPT_FILE="${PROMPT_FILE:-$REPO/.claude/automation/fix-one-issue.md}"
LOG_DIR="${LOG_DIR:-/home/ubuntu/cockatrice-auto-logs}"
CLAUDE_BIN="${CLAUDE_BIN:-/home/ubuntu/.local/bin/claude}"
MAX_ISSUES="${MAX_ISSUES:-10}"            # hard cap on issues per run
PER_ISSUE_TIMEOUT="${PER_ISSUE_TIMEOUT:-5400}"  # seconds (90 min) per issue
RESUME_ENABLED="${RESUME_ENABLED:-1}"     # 1 = resume interrupted sessions w/ full context
RESUME_CAP="${RESUME_CAP:-3}"             # max attempts before an issue needs a human
CARDS_ENABLED="${CARDS_ENABLED:-1}"       # when no issues remain, implement no-new-logic cards
CARDS_PROMPT_FILE="${CARDS_PROMPT_FILE:-$REPO/.claude/automation/implement-cards.md}"
# Card changes may only touch these paths ("no new logic" guardrail):
ALLOWED_CARD_PATHS='^(tricerules/tricerules-cards/data/|tricerules/CARDS\.md$|tricerules/tricerules-core/tests/scenario\.rs$)'
# CLAUDE_MODEL="${CLAUDE_MODEL:-}"        # e.g. "opus"; empty = your configured default
# ----------------------------------------------------------------------------

mkdir -p "$LOG_DIR"
RUN_TS="$(date +%Y%m%d-%H%M%S)"
RUN_LOG="$LOG_DIR/run-$RUN_TS.log"

# Single-run lock so an 08:00 run still going at 13:00 doesn't get a second one
# stomping on it.
LOCK="$LOG_DIR/.auto-fix.lock"
exec 9>"$LOCK"
if ! flock -n 9; then
  echo "Another auto-fix run is already in progress; exiting." >>"$RUN_LOG"
  exit 0
fi

# Mirror all output to the run log.
exec > >(tee -a "$RUN_LOG") 2>&1

echo "================================================================"
echo "auto-fix run start: $(date)"
echo "repo=$REPO base=$BASE_BRANCH max=$MAX_ISSUES timeout=${PER_ISSUE_TIMEOUT}s"
echo "================================================================"

cd "$REPO" || { echo "FATAL: cannot cd to $REPO"; exit 1; }

# Refuse to run on a dirty tree — never clobber in-progress human work.
if [[ -n "$(git status --porcelain)" ]]; then
  echo "FATAL: working tree is dirty; refusing to run. Resolve before automating."
  git status --short
  exit 1
fi

git checkout "$BASE_BRANCH" >/dev/null 2>&1 || { echo "FATAL: cannot checkout $BASE_BRANCH"; exit 1; }

PROMPT="$(cat "$PROMPT_FILE")"
model_args=()
[[ -n "${CLAUDE_MODEL:-}" ]] && model_args=(--model "$CLAUDE_MODEL")

# Find the first resumable interrupted issue under "## In Progress":
# Status: in-progress, Attempts < RESUME_CAP, and a Session id present.
# Prints "<issue-id> <session-id>" (space separated), or nothing.
find_resumable_session() {
  awk -v cap="$RESUME_CAP" '
    function flush() {
      if (id != "" && st == "in-progress" && (att+0) < cap && sess != "") {
        print id, sess; found=1; exit
      }
    }
    /^## / { if (insec) flush(); insec = ($0 ~ /^## In Progress/) }
    insec && /^- \[/ {
      flush(); id=""; st=""; att=0; sess="";
      t=$0; sub(/^[^#]*#/,"",t); sub(/[^0-9].*$/,"",t); id=t; next
    }
    insec && /^[[:space:]]+-[[:space:]]*Status:/   { t=$0; sub(/^[[:space:]]+-[[:space:]]*Status:[[:space:]]*/,"",t); st=t }
    insec && /^[[:space:]]+-[[:space:]]*Attempts:/ { t=$0; sub(/^[[:space:]]+-[[:space:]]*Attempts:[[:space:]]*/,"",t); att=t }
    insec && /^[[:space:]]+-[[:space:]]*Session:/  { t=$0; sub(/^[[:space:]]+-[[:space:]]*Session:[[:space:]]*/,"",t); sess=t }
    END { if (!found) flush() }
  ' issues.md
}

PROMPT_CARDS=""
[[ -f "$CARDS_PROMPT_FILE" ]] && PROMPT_CARDS="$(cat "$CARDS_PROMPT_FILE")"

resolved=0
cards_added=0
mode="issues"   # issues -> (when drained) cards

for (( i=1; i<=MAX_ISSUES; i++ )); do
  echo
  echo "---- iteration $i ($mode)  $(date '+%H:%M:%S') ----"

  # Start each task from a clean base branch.
  git checkout "$BASE_BRANCH" >/dev/null 2>&1
  if [[ -n "$(git status --porcelain)" ]]; then
    echo "Base branch dirty before iteration $i (previous agent left a mess); stopping."
    git status --short
    break
  fi

  # Once no issue rows remain at all, switch to card mode (or stop).
  if [[ "$mode" == "issues" ]] \
     && ! grep -qE '^[[:space:]]*-[[:space:]]*\[ \][[:space:]]*#' issues.md; then
    if [[ "$CARDS_ENABLED" == "1" && -n "$PROMPT_CARDS" ]]; then
      echo "No issue rows left in issues.md; switching to card mode."
      mode="cards"
    else
      echo "No issue rows left and card mode disabled; stopping."
      break
    fi
  fi

  TASK_LOG="$LOG_DIR/task-$RUN_TS-$(printf '%02d' "$i").log"
  rc=0
  pre_master="$(git rev-parse "$BASE_BRANCH")"

  if [[ "$mode" == "issues" ]]; then
    # ---- ISSUE iteration: resume an interrupted session or start fresh ----
    resume_id=""; resume_sid=""
    if [[ "$RESUME_ENABLED" == "1" ]]; then
      read -r resume_id resume_sid < <(find_resumable_session) || true
    fi

    if [[ -n "$resume_sid" ]]; then
      echo "resuming session $resume_sid for interrupted issue #$resume_id (log: $TASK_LOG)"
      timeout "$PER_ISSUE_TIMEOUT" "$CLAUDE_BIN" -p \
"Your previous session was interrupted (the usage window ended). You were working issue #$resume_id on branch fix/issue-$resume_id. Re-check git state and continue to completion, following the workflow and output discipline you were already given. Keep the line \`Session: $resume_sid\` in that issue's In Progress block. Finish with the required AUTOFIX_RESULT line." \
          --resume "$resume_sid" \
          "${model_args[@]}" \
          --dangerously-skip-permissions \
          --output-format text \
          >"$TASK_LOG" 2>&1
      rc=$?
      if [[ $rc -ne 0 ]] && grep -qiE "session.*(not found|does not exist|invalid|unknown|expired)|no such session|could not.*resume" "$TASK_LOG"; then
        echo "resume failed (stale session); falling back to a fresh agent."
        resume_sid=""
      fi
    fi

    if [[ -z "$resume_sid" ]]; then
      new_sid="$(uuidgen)"
      echo "fresh issue agent, session $new_sid (log: $TASK_LOG)"
      timeout "$PER_ISSUE_TIMEOUT" "$CLAUDE_BIN" -p \
"$PROMPT

## Your session id
This run's session id is: $new_sid
When you create or update the chosen issue's \`## In Progress\` block (on claim
OR on resume), include/refresh the line \`  - Session: $new_sid\` so a future
interrupted run can resume this exact session with full context." \
          --session-id "$new_sid" \
          "${model_args[@]}" \
          --dangerously-skip-permissions \
          --output-format text \
          >"$TASK_LOG" 2>&1
      rc=$?
    fi
  else
    # ---- CARD iteration: fresh agent, no resume ----
    new_sid="$(uuidgen)"
    echo "fresh card agent, session $new_sid (log: $TASK_LOG)"
    timeout "$PER_ISSUE_TIMEOUT" "$CLAUDE_BIN" -p "$PROMPT_CARDS" \
        --session-id "$new_sid" \
        "${model_args[@]}" \
        --dangerously-skip-permissions \
        --output-format text \
        >"$TASK_LOG" 2>&1
    rc=$?
  fi

  tail -n 40 "$TASK_LOG"

  if [[ $rc -eq 124 ]]; then
    echo "Agent hit the ${PER_ISSUE_TIMEOUT}s timeout; stopping."
    break
  fi
  if [[ $rc -ne 0 ]]; then
    echo "claude exited non-zero ($rc) — likely usage-window exhausted or a crash. Stopping."
    break
  fi

  result_line="$(grep -aE '^AUTOFIX_RESULT:' "$TASK_LOG" | tail -n 1)"
  echo "result: ${result_line:-<none>}"

  if [[ "$mode" == "issues" ]]; then
    case "$result_line" in
      *"RESOLVED"*)
        resolved=$(( resolved + 1 )); echo "issue resolved; continuing." ;;
      *"NO_ELIGIBLE_ISSUES"*)
        if [[ "$CARDS_ENABLED" == "1" && -n "$PROMPT_CARDS" ]]; then
          echo "no eligible issues; switching to card mode."; mode="cards"
        else
          echo "no eligible issues; stopping."; break
        fi ;;
      *"BLOCKED #"*)
        echo "issue parked as blocked; moving on to the next." ;;
      *"BLOCKED"*)
        echo "structural BLOCKED (no issue id); stopping."; break ;;
      *)
        echo "no clear AUTOFIX_RESULT; stopping."; break ;;
    esac
  else
    case "$result_line" in
      *"CARD_ADDED master "*)
        # Vanilla batch committed to master: enforce data-only, else revert it.
        git checkout "$BASE_BRANCH" >/dev/null 2>&1
        bad="$(git diff --name-only "$pre_master".."$BASE_BRANCH" | grep -vE "$ALLOWED_CARD_PATHS" || true)"
        if [[ -n "$bad" ]]; then
          echo "VIOLATION: vanilla commit touched non-allowed paths — reverting:"; echo "$bad"
          git reset --hard "$pre_master" >/dev/null 2>&1
          echo "reverted to $pre_master; stopping."; break
        fi
        cards_added=$(( cards_added + 1 )); echo "vanilla batch added; continuing." ;;
      *"CARD_ADDED cards/"*)
        slug="${result_line##*cards/}"; branch="cards/${slug%% *}"
        bad="$(git diff --name-only "$BASE_BRANCH".."$branch" 2>/dev/null | grep -vE "$ALLOWED_CARD_PATHS" || true)"
        [[ -n "$bad" ]] && { echo "WARNING: $branch touched non-allowed paths (needs human review):"; echo "$bad"; }
        cards_added=$(( cards_added + 1 )); echo "card $branch added; continuing." ;;
      *"CARD_NONE"*)
        echo "no no-new-logic cards left; stopping."; break ;;
      *"CARD_BLOCKED"*)
        echo "card attempt blocked; moving on." ;;
      *)
        echo "no clear AUTOFIX_RESULT; stopping."; break ;;
    esac
  fi
done

git checkout "$BASE_BRANCH" >/dev/null 2>&1
echo
echo "================================================================"
echo "auto-fix run end: $(date)"
echo "  issues resolved: $resolved    card tasks added: $cards_added"
echo "  fix branches:";   git branch --list 'fix/issue-*' | sed 's/^/    /'
echo "  card branches:";  git branch --list 'cards/*'     | sed 's/^/    /'
echo "================================================================"
