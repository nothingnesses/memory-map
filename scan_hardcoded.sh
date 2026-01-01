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

