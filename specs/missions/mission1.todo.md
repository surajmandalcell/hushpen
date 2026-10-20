# Mission 1 (Hushpen 2.0.0): remaining work and resume guide

State as of 2026-10-21. Read `specs/missions/mission1.completed.md` first. This file is written so a fresh agent can resume with one prompt: "Resume the Hushpen mission. Read `specs/missions/mission1.todo.md` and `specs/missions/mission1.completed.md` in the repo, then continue."

## Resume in one prompt

The mission runs in Factory Droid as orchestrator mission `f0a389f7-d034-46ca-9118-308ed631c4b8`, state in `/Users/surajmandal/.factory/missions/f0a389f7-d034-46ca-9118-308ed631c4b8`. To resume inside Factory: open that mission and call `start_mission_run` when this file says so. To resume outside Factory (plain agent): follow the "Remaining work" steps below by hand; `features.json` in the mission dir holds the exact feature descriptions, preconditions, and expected behavior for each step.

The mission is PAUSED at S2 part B, waiting for the owner: the Accessibility and Input Monitoring grants (Touch ID), then the real-key checklist. Do not start new milestone or feature work until step 1 below is unblocked.

## Current live state

- The test app `mac-check/Hushpen.app` is signed with the Apple Development identity (designated requirement: bundle id `app.hushpen.test` plus the certificate). The new signature dropped the old ad-hoc grants once.
- Grants for the new signature: Removable Volumes on (set through Codex in System Settings > Files & Folders), Microphone granted. Accessibility (System Settings name: Device Control and Data Access) and Input Monitoring read denied. A denied app's request is ignored, so Hushpen never adds itself to those lists. The owner removes the old "Hushpen.app" row and adds `mac-check/Hushpen.app` with + in both lists (Touch ID).
- Hushpen d2 runs from the build before 16e1b7c (check with `pgrep -x hushpen`), on the repair permission page. The compiled 16e1b7c build is in the target folder; after the grants, quit Hushpen and rerun `harness/mac-test-app.sh` to copy and sign it.
- The S2 watcher is not running; the lock file holds a dead pid. Relaunch `bin/watch.sh` (fireAndForget, 8 h deadline) after Hushpen restarts.

## Remaining work, in order

### Step 1: owner finishes the S2 checklist (BLOCKED ON OWNER)

The owner runs the 23-step checklist at `/Volumes/External1TB/data/_custom/droid/hushpen/evidence/m5-mac-session-s2/owner-checklist.md` (about 16 minutes) and says "done", with the numbers of any failed steps. Do not continue until then. If the owner already said done, go to step 2.

If the processes died (app or watcher gone): relaunch with `bin/launch.sh` and `bin/watch.sh` in the S2 evidence `bin/` folder, confirm the lock file and the 940x600 window, then hand the checklist to the owner again. After an app code change, rebuild Hushpen.app with `harness/mac-test-app.sh` (signed with the Apple Development identity, so the grants stay). Never run tccutil.

### Step 2: S2 part B (feature m5-mac-session-s2)

Resume the mission run. The pending feature order in `features.json` is: m5-mac-session-s2, m7-mac-session-s3, m7-publish. Part B marks each checklist line from app-recorded evidence only (History store, hushpen.log, watcher logs, the saved scratch.txt, owner screenshots on the Desktop), never from the owner's word. Remaining assertions are listed in the feature: VAL-MACS1-001, 003-010; VAL-MACS2-001, 003-008, 011-013; VAL-NAT-001, 003, 005, 006, 008, 013 (some partly pre-proven in `session.json` runs 1-3; keep those results). End checks: no "already borrowed" or "panicked" in hushpen.log, no SIGTRAP, front app unchanged from start.

### Step 3: hp5-llm validators (automatic)

When S2 completes, the system injects scrutiny-validator and user-testing-validator for milestone hp5-llm. Triage any failures per the orchestrator rules; do not create validator features by hand.

### Step 4: S3 Mac session on rc.3 (feature m7-mac-session-s3)

The flow bar redesign (round cross and tick, no language chip, hidden at rest unless Show when idle is on, spring motion) landed after rc.2. Build rc.3 first: add a row to `library/ci-runs.md`, then dispatch release.yml with `tag=v2.0.0-rc.3` (one CI run). S2 still runs on its old test build; its flow bar step tests the old pill.

Full release-candidate session on the rc.3 zip and dmg: install, permissions, real-key and speech steps, update from rc.2 to rc.3 from a local signed feed (test-feed builds with the update-test-feed feature and a test minisign key). Same driver and rules as S2: Codex computer use (yolo flag) in the background; release builds have no test hook, so verify from app logs and Codex captures. Flow: part A, owner checklist round 1, part B, round 2 if needed. Also folded in:
- VAL-MACS2-009 and VAL-MACS2-010: AI cleanup (Gemma 4 E2B on Metal; first load is slow, not a timeout) and Undo cleanup in TextEdit.
- At launch-at-login start and at one cold start with all grants: no permission or repair page may appear, including the no-microphone reason. If it does, report a defect (CoreAudio may list devices late at login).
- The new flow bar on the Mac: shows only while a dictation runs, the round cross cancels, the round tick finishes, no language control, TextEdit keeps focus.
- Measure the macOS .app idle RSS (rc.3 zip, no model loaded, 60 s after Home shows, `ps -o rss`) and record it in `library/release.md` next to the Linux value (179-239 MB), against the 150 MB target with a reason if over.
- Mac look of the permission status icon (orange exclamation, "Needs attention" tooltip); a start with all grants opens Home.

### Step 5: hp7 validators (automatic)

Same as step 3, for milestone hp7-release.

### Step 6: publish v2.0.0 (feature m7-publish)

Only after every assertion is passed (end-of-mission gate: all 380 assertions "passed" in `validation-state.json`):
1. Fix the DESIGN.md App Icon paragraph paths: `src/assets/` becomes `assets/`, and drop the sentence about Windows .ico tray glyphs. This is the approved baseline that VAL-REL-002 compares against; change nothing else in DESIGN.md.
2. Delete the draft releases v2.0.0-rc.1, rc.2, and rc.3 (`gh release delete`, drafts only; keep the tags).
3. Tag v2.0.0 on master HEAD (only if the tree differs from rc.3 in version strings and harness files only), push the tag (starts no workflow).
4. Dispatch release.yml with `tag=v2.0.0` (CI run 31), verify job passes, confirm every assertion passed, then publish the draft (`gh release edit --draft=false`).
5. Update `docs/` for Hushpen in one push (CI run 32, starts the docs deploy) and check the site.
6. The GitHub repo rename stays a later owner-confirmed step. Not part of this mission.

### Step 7: README gate and final report

Create or update the repo-root `README.md` for the final state (what Hushpen is, build/run/test instructions, environment needs). Then the final mission report.

## Hard rules for whoever resumes

- Never steal focus on the Mac. No osascript, JXA, System Events, CGEvent tools, cua-driver, VMs, or notifications. GUI driving is Codex computer use through `codex exec --dangerously-bypass-approvals-and-sandbox` only, in the background, plus hookctl on debug builds. Verify the frontmost app before and after every step; stop at the first change.
- Standing go-ahead for all Mac sessions: never ask permission to proceed, never return only to ask. Owner-performed steps go on a written checklist.
- Game rule: nothing synthetic while a full-screen game is in front. Wait it out.
- CI budget: 29 of 35 runs used (hard limit 50). Every new run needs a row in `library/ci-runs.md` before it starts. Planned remaining: 3 (rc.3, v2.0.0, docs). apt DNS flakes in e2e-x64 get exactly one failed-job re-run.
- Publish only artifacts built on CI ubuntu-22.04 runners.
- Rerun the license check after any Cargo.lock change.
- Data, models, caches, and build output stay on `/Volumes/External1TB`; agent scratch under `/Volumes/External1TB/data/_custom/droid/hushpen`. The jq temp file for feature edits is `.../droid/hushpen/tmp/f.json`.
- Edit files with the Edit tool only. Commit at checkpoints with the git-workflow skill; identity is already configured; never add AI attribution trailers; never create branches or worktrees.
- When the owner mentions a requirement mid-mission, update mission.md, AGENTS.md, library files, the contract, and features.json so every file states the new truth before workers resume.

## Reference map

- Feature descriptions with preconditions and expected behavior: `features.json` in the mission dir (m5-mac-session-s2, m7-mac-session-s3, m7-publish are the only pending features).
- Validation contract: `validation-contract.md` (380 assertions; 271 passed, 109 pending). State: `validation-state.json`.
- Worker procedures: `skills/mac-session-worker/SKILL.md` (S2/S3 driver, cx.sh, checklists), `skills/platform-worker/SKILL.md` (publish), and the rest under `skills/`.
- Operational facts: `AGENTS.md`, `services.yaml` (all commands; no services on ports), `library/ci-runs.md`, `library/release.md`, `library/user-testing.md`, `library/engine.md`, `research/codex-computer-use.md`, all in the mission dir.
- S2 evidence and tools: `/Volumes/External1TB/data/_custom/droid/hushpen/evidence/m5-mac-session-s2/` (`session.json`, `owner-checklist.md`, `bin/`, watcher logs).
