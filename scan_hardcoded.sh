#!/bin/bash

echo "=== Scanning for Hard-Coded Values ==="
echo ""

echo "--- Constants (const ...) ---"
git grep -nE "const [A-Z][A-Z0-9_]*"
echo ""

echo "--- URLs (http, https, ws, wss) ---"
git grep -nE "(https?|wss?)://"
echo ""

echo "--- IPs and Ports (127.0.0.1, :8000, etc) ---"
git grep -nE "([0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}|:(8000|3000|9000|5432))"
echo ""

echo "--- Potential Secrets (password, secret, key, token) ---"
# Excluding lock files and docs to reduce noise
git grep -niE "secret|password|key|token|credential" -- ':!*.lock' ':!*.yaml' ':!*.md' ':!*.json' ':!*.csv'
echo ""

echo "--- Environment Variables (env!, std::env) ---"
git grep -nE "(env!|option_env!|std::env::var|dotenv)" -- "*.rs"
echo ""

echo "--- Hardcoded UI Strings (Heuristic) ---"
# Looks for strings starting with a capital letter, at least 2 chars long, inside .rs files.
# Excludes const definitions, logging, and attributes.
# We use a negative lookahead/exclusion via grep -v to filter out common false positives.
git grep -nE "\"[A-Z][a-zA-Z0-9 '.,?!-]{1,}\"" -- "*.rs" | grep -vE "(const |use |log::|tracing::|println!|eprintln!|panic!|expect\(|debug_error!|#\[)"
echo ""

echo "--- Hardcoded Hex Colors ---"
git grep -nE "#[0-9a-fA-F]{3,6}" -- "*.rs" "*.css" "*.scss" "*.html"
echo ""

echo "--- Local Variables with Literals (let ... = \"...\") ---"
git grep -nE "let [a-z_]+.*= \".*\"" -- ':!*.lock' ':!*.yaml' ':!*.md' ':!*.json'
echo ""

echo "--- Local Variables with Numbers (let ... = 123) ---"
git grep -nE "let [a-z_]+.*= [0-9_]+" -- ':!*.lock' ':!*.yaml' ':!*.md' ':!*.json'
echo ""

echo "--- SQL Queries (SELECT, INSERT, UPDATE, DELETE) ---"
git grep -nE "(SELECT|INSERT|UPDATE|DELETE) " -- "*.rs"
echo ""

echo "=== Scan Complete ==="
