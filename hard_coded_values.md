# Hard-Coded Values Report

This document lists hard-coded values identified in the codebase.

## 1. Credentials & Secrets
*None identified in source code.* (Moved to `.env` and `Config` struct).

## 2. Network Configuration
*None identified in source code.* (Moved to `.env` and `config.json`).

## 3. Application Constants
*None identified in source code.* (Moved to `backend/src/constants.rs`).

## 4. SQL Queries (Hard-coded Strings)

The following files contain hard-coded SQL query strings that should be moved to `backend/src/db/queries.rs`.

| File | Context |
|------|---------|
| `backend/src/graphql/objects/s3_object.rs` | Multiple complex `SELECT` queries with joins. |
| `backend/src/graphql/objects/user.rs` | `SELECT` queries for fetching users. |
| `backend/src/graphql/queries/mutation.rs` | `INSERT`, `UPDATE`, `DELETE`, and `SELECT COUNT(*)` queries. |
| `backend/src/main.rs` | Simple `SELECT 1` verification query. |

## How to Scan for More Values

A script `scan_hardcoded.sh` has been created to help you find these values in the future.

Usage:
```bash
./scan_hardcoded.sh
```
