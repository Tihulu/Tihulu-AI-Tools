#!/usr/bin/env bash
# AI-08: Exact-head, read-only source selection. All overlays are in an
# ephemeral GitHub Actions worktree; nothing is pushed, merged or installed.
set -Eeuo pipefail

BASE_SHA=5d69b6cc989746fbae694dd2ef709119dbbf1293
REGISTRY_SHA=7c3030d09c30a841ed42813b6427d7e7d7f7b589
MARKET_SHA=d015a806966d97e340f807b6b5ef418b17d0a492
CROSS_SHA=ca08368b27cdb25866b1a01b9474485ef1b62b4a
UI_SHA=5c7e62b23656b145c21222fd2c44152725e8a3a2
APP_SHA=10fcdc76ec5ac3ad8fa013971cdac7316890db3f
WIN_SHA=e29a4ae59dcf0110b492fcd8865de95669404517

[ -n "${GITHUB_RUN_ID:-}" ] || { echo "Requires disposable CI runner" >&2; exit 1; }
[ -n "${HOME:-}" ] || exit 1
QA_HEAD="$(git rev-parse HEAD)"
git fetch --no-tags --force origin \
  '+refs/pull/20/head:refs/remotes/origin/qa-p0-20' \
  '+refs/pull/22/head:refs/remotes/origin/qa-p0-22' \
  '+refs/pull/25/head:refs/remotes/origin/qa-p0-25' \
  '+refs/pull/28/head:refs/remotes/origin/qa-p0-28' \
  '+refs/pull/29/head:refs/remotes/origin/qa-p0-29' \
  '+refs/pull/30/head:refs/remotes/origin/qa-p0-30'

for pair in \
  "20:$REGISTRY_SHA" "22:$MARKET_SHA" "25:$APP_SHA" \
  "28:$UI_SHA" "29:$CROSS_SHA" "30:$WIN_SHA"; do
  pr="${pair%%:*}"; expected="${pair#*:}"
  actual="$(git rev-parse "refs/remotes/origin/qa-p0-$pr")"
  if [ "$expected" != "$actual" ]; then
    echo "::error::PR #$pr changed head: expected $expected, got $actual" >&2
    exit 1
  fi
done
for sha in "$REGISTRY_SHA" "$MARKET_SHA" "$APP_SHA" "$CROSS_SHA" "$UI_SHA" "$WIN_SHA"; do
  git merge-base --is-ancestor "$BASE_SHA" "$sha" || {
    echo "::error::Unexpected Phase 4 base for $sha" >&2; exit 1;
  }
done
git merge-base --is-ancestor "$REGISTRY_SHA" "$CROSS_SHA" || {
  echo "::error::PR #29 does not descend from PR #20" >&2; exit 1;
}

WORKTREE="$HOME/phase5-p0-qa-$GITHUB_RUN_ID"
git worktree add --detach "$WORKTREE" "$QA_HEAD"
# PR #29 includes exact PR #20 registry history and Market #22's
# implementation with source-owner remediation. It is NOT a git descendant
# of #22; ownership changes are reviewed separately.
git diff --binary "$BASE_SHA" "$CROSS_SHA" -- \
  crates/tihulu-ai-registry crates/tihulu-ai-market | git -C "$WORKTREE" apply --3way -
git diff --binary "$BASE_SHA" "$APP_SHA" -- \
  crates/tihulu-ai-registry/src/app_adapters.rs \
  crates/tihulu-ai-registry/tests/app_adapter_contract.rs | git -C "$WORKTREE" apply --3way -
git diff --binary "$BASE_SHA" "$UI_SHA" -- \
  apps/tihulu-ai-web/native-bridge.mjs \
  apps/tihulu-ai-web/tests/native-bridge.test.mjs | git -C "$WORKTREE" apply --3way -
git diff --binary "$BASE_SHA" "$WIN_SHA" -- \
  apps/tihulu-ai-desktop/src-tauri/icons/icon.ico \
  scripts/tests/test_phase5_windows_icon.py | git -C "$WORKTREE" apply --3way -
git -C "$WORKTREE" diff --cached --check
printf 'PHASE5_QA_WORKTREE=%s\n' "$WORKTREE" >> "$GITHUB_ENV"
echo "::notice::DISPOSABLE targeted P0 source union:"
echo "QA HEAD: $QA_HEAD"
echo "BASE: $BASE_SHA"
echo "REGISTRY20: $REGISTRY_SHA"
echo "MARKET22 SOURCE: $MARKET_SHA (not ancestor of PR29)"
echo "REGISTRY+MARKET29: $CROSS_SHA"
echo "APP25: $APP_SHA"
echo "UI28: $UI_SHA"
echo "WIN30: $WIN_SHA"
git -C "$WORKTREE" status --short
# Do not turn these exact-head synthetic checks into real native acceptance.
