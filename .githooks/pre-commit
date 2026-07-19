#!/bin/bash
# Git pre-commit hook: security & hygiene checks before every commit.
# Checks: git email (noreply), secrets in staged files, .gitignore coverage.
#
# Exit 0 = allow commit, Exit 1 = block commit.
#
# Secret detection patterns are curated from the gitleaks ruleset
# (https://github.com/gitleaks/gitleaks). The shell array below mirrors
# the high-precision, prefix-anchored subset; review upstream periodically
# and pull in new vendor token formats. Tracked source: gitleaks v8 rules.toml.
#
# Allowlist pragma: place one of the following near a known-safe match to
# silence the scan. The marker can appear inside any comment syntax.
#   <line with secret>   # oss-checker:allow
#   # oss-checker:allow-next-line
#   <line with secret>
#
# This script is also designed to be sourced as a library by tests; the
# main entry point only runs when executed directly.

# --- Patterns ----------------------------------------------------------------
# Organized by signal strength:
#   PREFIX:     vendor-specific token prefixes (very low false-positive rate)
#   STRUCTURAL: format-specific markers like PEM blocks and JWTs
#   KEYWORD:    keyword-near-quoted-value heuristic (advisory; higher FP rate)

PATTERNS_PREFIX=(
  # AWS
  'AKIA[0-9A-Z]{16}'
  'ASIA[0-9A-Z]{16}'
  # GitHub (PAT classic, OAuth, user-to-server, server-to-server, refresh)
  'gh[pousr]_[A-Za-z0-9]{36}'
  'github_pat_[A-Za-z0-9_]{82}'
  # GitLab personal access token
  'glpat-[A-Za-z0-9_-]{20}'
  # npm automation/publish token
  'npm_[A-Za-z0-9]{36}'
  # Slack tokens and incoming webhooks
  'xox[baprs]-[A-Za-z0-9-]{10,}'
  'https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[A-Za-z0-9]{24}'
  # Stripe live secret / restricted keys
  '(sk|rk)_live_[0-9a-zA-Z]{24,}'
  # Anthropic
  'sk-ant-[A-Za-z0-9_-]{90,}'
  # OpenAI scoped keys (post-2024). Legacy bare sk- is too generic to anchor.
  'sk-(proj|svcacct|admin)-[A-Za-z0-9_-]{40,}'
  # Google Cloud API key / OAuth client secret
  'AIza[0-9A-Za-z_-]{35}'
  'GOCSPX-[A-Za-z0-9_-]{28}'
  # SendGrid
  'SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}'
  # DigitalOcean PAT
  'dop_v1_[a-f0-9]{64}'
  # Square access / OAuth tokens
  'sq0(atp|csp)-[A-Za-z0-9_-]{22,}'
  # Telegram bot token
  '[0-9]{8,10}:[A-Za-z0-9_-]{35}'
  # Discord bot token (legacy format)
  '[MN][A-Za-z0-9]{23}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27}'
)

PATTERNS_STRUCTURAL=(
  # PEM private keys (RSA/EC/DSA/OpenSSH/PGP/encrypted variants)
  '-----BEGIN ((RSA|EC|DSA|OPENSSH|PGP|ENCRYPTED) )?PRIVATE KEY( BLOCK)?-----'
  # JSON Web Token (header + payload + signature, all base64url)
  'eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}'
)

PATTERNS_KEYWORD=(
  # Keyword followed by quoted value of meaningful length.
  # Tightened over the legacy `key:` regex by requiring quotes and >=16 chars.
  "(?i)(api[_-]?key|secret[_-]?key|access[_-]?token|auth[_-]?token|password|passwd)\\s*[:=]\\s*['\"\`]([A-Za-z0-9_+/=.\\-]{16,})['\"\`]"
)

PRAGMA_INLINE='oss-checker:allow($|[^-])'
PRAGMA_NEXT='oss-checker:allow-next-line'

SCAN_SKIP_EXT='\.(png|jpg|jpeg|gif|ico|wasm|vsix|parquet|lock|pdf|woff2?|ttf|eot|map)$'

# --- Helpers -----------------------------------------------------------------

# Returns 0 (allowed) if the line carries a pragma or its previous line does.
# Set OSS_CHECKER_NO_PRAGMA=1 to disable pragma evaluation (used by the test
# harness to verify raw detection independently of the allowlist mechanism).
is_allowed() {
  [ -n "${OSS_CHECKER_NO_PRAGMA:-}" ] && return 1
  local file="$1" line_no="$2" content="$3"
  if printf '%s' "$content" | grep -qE "$PRAGMA_INLINE"; then
    return 0
  fi
  if [ "$line_no" -gt 1 ]; then
    local prev
    prev=$(sed -n "$((line_no - 1))p" "$file" 2>/dev/null || true)
    if printf '%s' "$prev" | grep -qE "$PRAGMA_NEXT"; then
      return 0
    fi
  fi
  return 1
}

# Scans the given files and appends "file:line:content" entries to FINDINGS.
# Caller must declare FINDINGS=() before invoking.
scan_files() {
  local file pattern matches match line_no content
  local all_patterns=(
    "${PATTERNS_PREFIX[@]}"
    "${PATTERNS_STRUCTURAL[@]}"
    "${PATTERNS_KEYWORD[@]}"
  )
  for file in "$@"; do
    [ -f "$file" ] || continue
    for pattern in "${all_patterns[@]}"; do
      matches=$(grep -nP -- "$pattern" "$file" 2>/dev/null || true)
      [ -n "$matches" ] || continue
      while IFS= read -r match; do
        line_no="${match%%:*}"
        content="${match#*:}"
        if ! is_allowed "$file" "$line_no" "$content"; then
          FINDINGS+=("$file:$match")
        fi
      done <<< "$matches"
    done
  done
}

# --- Main --------------------------------------------------------------------

main() {
  set -euo pipefail
  local ERRORS=()
  local WARNINGS=()
  FINDINGS=()

  # 1. Git email check
  local email
  email=$(git config user.email 2>/dev/null || echo "")
  if [ -z "$email" ]; then
    ERRORS+=("Git email is not configured")
  elif ! echo "$email" | grep -qi "noreply"; then
    ERRORS+=("Git email '$email' may expose personal info. Use a noreply address.")
  fi

  # 2. Secret scan on staged files
  local staged scannable
  staged=$(git diff --cached --name-only 2>/dev/null || echo "")
  if [ -n "$staged" ]; then
    scannable=$(echo "$staged" | grep -vE "$SCAN_SKIP_EXT" || true)
    if [ -n "$scannable" ]; then
      local files=()
      while IFS= read -r f; do
        [ -n "$f" ] && files+=("$f")
      done <<< "$scannable"
      scan_files "${files[@]}"
      local finding
      for finding in "${FINDINGS[@]}"; do
        ERRORS+=("Secret pattern in $finding")
      done
    fi
  fi

  # 3. .gitignore coverage
  if [ -f ".gitignore" ]; then
    grep -q '\.env' .gitignore || WARNINGS+=(".gitignore: missing .env* pattern")
    grep -qE '\*\.pem|\*\.key' .gitignore || WARNINGS+=(".gitignore: missing *.pem / *.key pattern")
  else
    WARNINGS+=(".gitignore file not found")
  fi

  # Output
  if [ ${#WARNINGS[@]} -gt 0 ]; then
    echo "=== Pre-commit warnings ==="
    local w
    for w in "${WARNINGS[@]}"; do echo "  WARN: $w"; done
  fi

  if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "=== Pre-commit check FAILED ==="
    local e
    for e in "${ERRORS[@]}"; do echo "  FAIL: $e"; done
    echo ""
    echo "Fix the issues above before committing."
    echo "If a finding is a known-safe fixture, add a pragma comment:"
    echo "  <line>  # oss-checker:allow"
    echo "  # oss-checker:allow-next-line"
    exit 1
  fi

  exit 0
}

# Only execute main when run directly, not when sourced (e.g. by tests).
if [ "${BASH_SOURCE[0]:-$0}" = "${0}" ]; then
  main "$@"
fi
