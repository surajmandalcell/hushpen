#!/bin/bash
# usage: seed-history.sh <count> [db]
# Adds <count> completed dictation rows with distinct created_at values (one second apart, the
# newest one second old) to the history database. The search index follows through its triggers.
# Row i has the text "seed row <i> <word>"; the word cycles through eight names. The app must
# have made the database already; stop the app first so the run is not slowed by a second writer.
set -euo pipefail
count=${1:?usage: seed-history.sh <count> [db]}
db=${2:-/data/history/history.db}
now=$(($(date +%s) * 1000))
sqlite3 "$db" <<SQL
BEGIN;
WITH RECURSIVE seq(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM seq WHERE i < $count)
INSERT INTO transcript
  (id, created_at, kind, status, duration_ms, model_id, language_detected,
   raw_text, rule_text, final_text, insert_outcome, target_app)
SELECT
  printf('%010X%016X', $now - i * 1000, i),
  $now - i * 1000,
  'dictation', 'completed', 1500, 'base', 'en',
  'seed row ' || i || ' ' || CASE i % 8
    WHEN 0 THEN 'harbor' WHEN 1 THEN 'lantern' WHEN 2 THEN 'quartz' WHEN 3 THEN 'meadow'
    WHEN 4 THEN 'orchard' WHEN 5 THEN 'violin' WHEN 6 THEN 'granite' ELSE 'saffron' END,
  'seed row ' || i || ' ' || CASE i % 8
    WHEN 0 THEN 'harbor' WHEN 1 THEN 'lantern' WHEN 2 THEN 'quartz' WHEN 3 THEN 'meadow'
    WHEN 4 THEN 'orchard' WHEN 5 THEN 'violin' WHEN 6 THEN 'granite' ELSE 'saffron' END,
  'seed row ' || i || ' ' || CASE i % 8
    WHEN 0 THEN 'harbor' WHEN 1 THEN 'lantern' WHEN 2 THEN 'quartz' WHEN 3 THEN 'meadow'
    WHEN 4 THEN 'orchard' WHEN 5 THEN 'violin' WHEN 6 THEN 'granite' ELSE 'saffron' END,
  'pasted', 'seed-target'
FROM seq;
COMMIT;
SQL
sqlite3 "$db" "select count(*) from transcript"
