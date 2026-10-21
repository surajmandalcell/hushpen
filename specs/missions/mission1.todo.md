# Mission 1 (Hushpen 2.0.0): remaining work and resume guide

State as of 2026-10-21. Read `specs/missions/mission1.completed.md` first. This file is written so a fresh agent can resume with one prompt: "Resume the Hushpen mission. Read `specs/missions/mission1.todo.md` and `specs/missions/mission1.completed.md` in the repo, then continue."

## Resume in one prompt

The mission runs in Factory Droid as orchestrator mission `f0a389f7-d034-46ca-9118-308ed631c4b8`, state in `/Users/surajmandal/.factory/missions/f0a389f7-d034-46ca-9118-308ed631c4b8`. To resume inside Factory: open that mission and call `start_mission_run` when this file says so. To resume outside Factory (plain agent): follow the "Remaining work" steps below by hand; `features.json` in the mission dir holds the exact feature descriptions, preconditions, and expected behavior for each step.

The mission is at the S3 re-verification (step 4b). S3 ran on rc.3 (run 39555285074, tag v2.0.0-rc.3 on 48dfb14): 11 assertions pass, VAL-MACS3-001 passes with notes, VAL-MACS3-011 is environment-limited partial. Results: partb-results.md in the S3 evidence folder. All 3 defects are fixed and committed: D1 self-activation (185aaeb: relaunch uses open -g, Launch enum, show_unfocused), D2 mic wedge (4cfc0ae: 4 s of exact-zero input retires the stream), D3 flow bar on release (5511d96: the shell owns the FlowBar entity). The owner's UI feedback landed in 44d122a (22 px text inputs via one ui::field fix, the word Hushpen in the sidebar band, Linux circles re-centered, all 50 pixel baselines regenerated). hp5-llm validators are done: scrutiny passed in round 2, user testing passed in round 2, 350 of 380 assertions passed.

Because the tree now differs from rc.3 by real code, the publish gate (step 6: tag v2.0.0 only if the tree differs from the last rc in version strings and harness files) needs rc.4 on HEAD. Plan: pre-flight the fixed code on a local signed release-profile bundle (evidence bin/bundle-release.sh), then tag v2.0.0-rc.4 and dispatch release.yml (CI run 31), re-run the S3 release-build checks on the rc.4 draft (install, onboarding Allow, cold start, idle RSS, flow bar at rest and idle pill, update rc.3 to rc.4 from the local signed feed, About rc.4), mark the 13 S3 assertions, then hp7, then publish. CI budget after rc.4 + v2.0.0 + docs: 33 of 35. VAL-MACS3-010/011 contract wording moves from rc.2-to-rc.3 to rc.3-to-rc.4.

## Current live state

- The test app `mac-check/Hushpen.app` is signed with the Apple Development identity (designated requirement: bundle id `app.hushpen.test` plus the certificate). The new signature dropped the old ad-hoc grants once.
- Grants for the new signature: all three granted (Microphone, Accessibility, Input Monitoring) plus Removable Volumes. On this macOS, Input Monitoring is composed from Accessibility: tccd answers ListenEvent from the Accessibility record, so a stale Accessibility record also denies Input Monitoring. Allow on a permission that is not granted first runs `tccutil reset <Service> <own bundle id>` (logged as `PERMISSION_RESET <key>`), then requests it. Allow on Accessibility fixed both rows; the running app saw the grants only after a relaunch.
- The test app is built at dcb8911 and its live smoke passed (`smoke-dcb8911/README.md` in the S2 evidence folder). Start it with `bin/launch.sh`. The S2 watcher is stopped: its guard cancels any run while TextEdit is not in front and it overwrites the clipboard, so it does not fit agent runs.


## Remaining work, in order

### Steps 1 and 2: S2 (DONE)

The agent ran every S2 step itself (the owner runs no checklist): hook dictation into a background TextEdit through the test-only insert target, GUI steps through Codex in the background, pixel checks on per-window captures. Results per assertion: `partb-results.md` in the S2 evidence folder. The contract now states the 2026-10-21 rule: modifier-only key holds go through the hook pipeline events, "TextEdit frontmost" reads as "front app unchanged", and steps that make macOS activate Hushpen are left out. S2 found and fixed one defect: the macOS sidebar was opaque (c48784e).
### Step 3: hp5-llm validators (DONE)

Scrutiny passed in round 2, user testing in round 2 (`validation/hp5-llm/` in the mission dir).

### Step 4: S3 Mac session on rc.3 (feature m7-mac-session-s3)

The flow bar redesign (round cross and tick, no language chip, hidden at rest unless Show when idle is on, spring motion) landed after rc.2. Build rc.3 first: add a row to `library/ci-runs.md`, then dispatch release.yml with `tag=v2.0.0-rc.3` (one CI run). S2 still runs on its old test build; its flow bar step tests the old pill.

Full release-candidate session on the rc.3 zip and dmg: install, permissions (Allow is pressed live here: the release bundle starts with no grants), real-key and speech steps, update from rc.2 to rc.3 from a local signed feed (test-feed builds with the update-test-feed feature and a test minisign key). Same driver and rules as S2: Codex computer use (yolo flag) in the background; release builds have no test hook, so verify from app logs and Codex captures. The owner runs no checklist; the agent does every step. The release build has no hook and no allowed driver can hold a key, so the dictation and insertion assertions (VAL-MACS2-009/010, VAL-MACS3-006..009) are proven on the signed test build of the identical commit, and the release build proves install, onboarding with a live Allow per permission, cold start, update, RSS, and the flow bar at rest (contract: "S3 release-build rule 2026-10-21"). Also folded in:
- VAL-MACS2-009 and VAL-MACS2-010: AI cleanup (Gemma 4 E2B on Metal; first load is slow, not a timeout) and Undo cleanup in TextEdit.
- At launch-at-login start and at one cold start with all grants: no permission or repair page may appear, including the no-microphone reason. If it does, report a defect (CoreAudio may list devices late at login).
- The new flow bar on the Mac: shows only while a dictation runs, the round cross cancels, the round tick finishes, no language control, TextEdit keeps focus.
- Measure the macOS .app idle RSS (rc.3 zip, no model loaded, 60 s after Home shows, `ps -o rss`) and record it in `library/release.md` next to the Linux value (179-239 MB), against the 150 MB target with a reason if over.
- Mac look of the permission status icon (orange exclamation, "Needs attention" tooltip); a start with all grants opens Home.

### Step 5: hp7 validators (automatic)

Same as step 3, for milestone hp7-release.

### Step 6: publish v2.0.0 (feature m7-publish)

Only after every assertion is passed (end-of-mission gate: all 380 assertions "passed" in `validation-state.json`):
1. DONE (5178772): the DESIGN.md App Icon paragraph points at `assets/` and has no Windows sentence. This is the approved baseline that VAL-REL-002 compares against.
2. Delete the draft releases v2.0.0-rc.1, rc.2, and rc.3 (`gh release delete`, drafts only; keep the tags).
3. Tag v2.0.0 on master HEAD (only if the tree differs from rc.3 in version strings and harness files only), push the tag (starts no workflow).
4. Dispatch release.yml with `tag=v2.0.0` (CI run 31), verify job passes, confirm every assertion passed, then publish the draft (`gh release edit --draft=false`).
5. Update `docs/` for Hushpen in one push (CI run 32, starts the docs deploy) and check the site.
6. The GitHub repo rename stays a later owner-confirmed step. Not part of this mission.

### Step 7: README gate and final report

Create or update the repo-root `README.md` for the final state (what Hushpen is, build/run/test instructions, environment needs). Then the final mission report.

## Hard rules for whoever resumes

- Never steal focus on the Mac. No osascript, JXA, System Events, CGEvent tools, cua-driver, VMs, or notifications. GUI driving is Codex computer use through `codex exec --dangerously-bypass-approvals-and-sandbox` only, in the background, plus hookctl on debug builds. Verify the frontmost app before and after every step; stop at the first change.
- Standing go-ahead for all Mac sessions: never ask permission to proceed, never return only to ask. The owner performs no step; a step no allowed driver can do is named as a gap.
- Game rule: nothing synthetic while a full-screen game is in front. Wait it out.
- CI budget: 29 of 35 runs used (hard limit 50). Every new run needs a row in `library/ci-runs.md` before it starts. Planned remaining: 3 (rc.3, v2.0.0, docs). apt DNS flakes in e2e-x64 get exactly one failed-job re-run.
- Publish only artifacts built on CI ubuntu-22.04 runners.
- Rerun the license check after any Cargo.lock change.
- Data, models, caches, and build output stay on `/Volumes/External1TB`; agent scratch under `/Volumes/External1TB/data/_custom/droid/hushpen`. The jq temp file for feature edits is `.../droid/hushpen/tmp/f.json`.
- Edit files with the Edit tool only. Commit at checkpoints with the git-workflow skill; identity is already configured; never add AI attribution trailers; never create branches or worktrees.
- When the owner mentions a requirement mid-mission, update mission.md, AGENTS.md, library files, the contract, and features.json so every file states the new truth before workers resume.

## Reference map

- Feature descriptions with preconditions and expected behavior: `features.json` in the mission dir (m5-mac-session-s2, m7-mac-session-s3, m7-publish are the only pending features).
- Validation contract: `validation-contract.md` (380 assertions; 299 passed, 81 pending). State: `validation-state.json`.
- Worker procedures: `skills/mac-session-worker/SKILL.md` (S2/S3 driver, cx.sh, checklists), `skills/platform-worker/SKILL.md` (publish), and the rest under `skills/`.
- Operational facts: `AGENTS.md`, `services.yaml` (all commands; no services on ports), `library/ci-runs.md`, `library/release.md`, `library/user-testing.md`, `library/engine.md`, `research/codex-computer-use.md`, all in the mission dir.
- S2 evidence and tools: `/Volumes/External1TB/data/_custom/droid/hushpen/evidence/m5-mac-session-s2/` (`session.json`, `owner-checklist.md`, `bin/`, watcher logs).
