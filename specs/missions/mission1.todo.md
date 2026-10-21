# Mission 1 (Hushpen 2.0.0): remaining work and resume guide

State as of 2026-10-21. Read `specs/missions/mission1.completed.md` first. This file is written so a fresh agent can resume with one prompt: "Resume the Hushpen mission. Read `specs/missions/mission1.todo.md` and `specs/missions/mission1.completed.md` in the repo, then continue."

## Resume in one prompt

The mission runs in Factory Droid as orchestrator mission `f0a389f7-d034-46ca-9118-308ed631c4b8`, state in `/Users/surajmandal/.factory/missions/f0a389f7-d034-46ca-9118-308ed631c4b8`. To resume inside Factory: open that mission and call `start_mission_run` when this file says so. To resume outside Factory (plain agent): follow the "Remaining work" steps below by hand; `features.json` in the mission dir holds the exact feature descriptions, preconditions, and expected behavior for each step.

The mission is at the S3 re-verification (step 4b). S3 ran on rc.3 (run 39555285074, tag v2.0.0-rc.3 on 48dfb14): 11 assertions pass, VAL-MACS3-001 passes with notes, VAL-MACS3-011 is environment-limited partial. Results: partb-results.md in the S3 evidence folder. All 3 defects are fixed and committed: D1 self-activation (185aaeb: relaunch uses open -g, Launch enum, show_unfocused), D2 mic wedge (4cfc0ae: 4 s of exact-zero input retires the stream), D3 flow bar on release (5511d96: the shell owns the FlowBar entity). The owner's UI feedback landed in 44d122a (22 px text inputs via one ui::field fix, the word Hushpen in the sidebar band, Linux circles re-centered, all 50 pixel baselines regenerated). hp5-llm validators are done: scrutiny passed in round 2, user testing passed in round 2, 350 of 380 assertions passed.

rc.4 shipped on 33d34fb (run 39589125333 success, row 31). The S3 re-verification on rc.4 (partc-results.md Parts C-E) verified D1 and D3 fixed live, the rc.3-to-rc.4 update, and 6 of 8 dictation assertions at HEAD. The owner then reported 3 live defects: D4 menu bar glyph black and 2x size (fixed 1ffd717: gpui-tray 0.2.0 template icon at scale 2; the old tray_host window walk never found the out-of-process status item), D5 Hushpen never in the Input Monitoring list (measured on macOS 27: no request can create a ListenEvent row, tccd answers listen-event from the Accessibility record; fixed b213f7a + 8547792: launch registers an Accessibility row with no prompt, Allow never resets a denied row, Accessibility counts as Input Monitoring on macOS 27, FOLLOWS_HINT sends the IM row to the Accessibility pane), and the idle pill always showing (cause: S3 tests left overlay.idleVisible=true in the owner's real settings; reset to false, default is false). A full sweep found 0 broken, 6 visible flaws, 9 polish items (evidence/sweep/findings.md), all fixed (S1-S8, S10-S15; S9 was the rename leftovers). hp7 validator on rc.4: 364/380 passed; its 2 failures (cargo deny advisories, 9 Linux scenarios from the D2 silence change) were fixed (798ccb6, c879479). Other fixes: asrpro rename leftovers incl. update feed URL and 13 asset files (31c6a50, b8d938d, 02b5747), harness mic.sh vacuous VAL-MIC-003 (f89d376), History suite matched to the new row text (d40493c). All pushed; origin/master = d40493c.

CI budget forced the final shape (32 of 35 used): no rc.5. The final-candidate rule (validation-contract.md): one ci.yml full run on the final commit (run 33 = 39646189561 on d40493c, row added before dispatch), then tag v2.0.0 on d40493c and release.yml (run 34); the v2.0.0 draft is the final candidate, and every rc-scoped assertion is proven on its artifacts before publish (they were reset to pending, 358/380); then publish and the docs push (run 35). Local proof before run 33: pre-flight at 5257248+d40493c passed (Mac + Linux gates, local full suite 167/169 with the history suite fixed and green on re-run, slot-checks, README commands on both hosts = VAL-REL-004, Mac smoke: permissions granted, dictation to History, silent run = no speech, no pill at rest). Staged for the final session (evidence/final-candidate/README.md has the exact commands): signed test app at d40493c, release-profile bundle, fresh data dir mac-check/s3/dfinal, and the rc.4-to-2.0.0 update pair with a signed local feed (mac-check/s3/updfinal, port 4345). Build gotcha: "Apple Development" is ambiguous (two certs); use HUSHPEN_SIGN_IDENTITY="Apple Development: SURAJ MANDAL (H25U2GA43B)". Remaining owner action: ONE Accessibility toggle (Touch ID) on the release build; on macOS 27 that grants Input Monitoring too. The mic system alert click may also be needed once.

## Current live state

- The test app `mac-check/Hushpen.app` is signed "Apple Development: SURAJ MANDAL (H25U2GA43B)" (designated requirement: bundle id `app.hushpen.test` plus the certificate), built at d40493c. Grants survive rebuilds with this cert.
- macOS 27 permission model (measured, evidence d5-input-monitoring/): no request creates a row in the Input Monitoring list; tccd answers listen-event access from the Accessibility record. The app registers its Accessibility row at launch with no prompt (`PERMISSION_REGISTER`), Allow resets a record only when it repairs a lost grant, and on macOS 27 Accessibility counts as Input Monitoring (the IM row shows Ready, the key tap runs under AX alone).
- Test-app grants: Microphone, Accessibility granted (Input Monitoring reads granted through Accessibility).
- The test app is built at dcb8911 and its live smoke passed (`smoke-dcb8911/README.md` in the S2 evidence folder). Start it with `bin/launch.sh`. The S2 watcher is stopped: its guard cancels any run while TextEdit is not in front and it overwrites the clipboard, so it does not fit agent runs.


## Remaining work, in order

### Steps 1 and 2: S2 (DONE)

The agent ran every S2 step itself (the owner runs no checklist): hook dictation into a background TextEdit through the test-only insert target, GUI steps through Codex in the background, pixel checks on per-window captures. Results per assertion: `partb-results.md` in the S2 evidence folder. The contract now states the 2026-10-21 rule: modifier-only key holds go through the hook pipeline events, "TextEdit frontmost" reads as "front app unchanged", and steps that make macOS activate Hushpen are left out. S2 found and fixed one defect: the macOS sidebar was opaque (c48784e).
### Step 3: hp5-llm validators (DONE)

Scrutiny passed in round 2, user testing in round 2 (`validation/hp5-llm/` in the mission dir).

### Step 4: S3 Mac session, final candidate (feature m7-mac-session-s3)

rc.3 and rc.4 sessions are done (partb/partc results). Remaining: the final-candidate session on the v2.0.0 draft artifacts (run 34), per the final-candidate rule in the contract. Everything is staged; `evidence/final-candidate/README.md` holds the exact commands for each check:
- Install from the draft zip and dmg, codesign and version checks, cold start with no activation, About 2.0.0, status icon, sidebar band, flow bar at rest and idle pill, idle RSS.
- Onboarding on an empty data folder with live Allow (owner: mic alert click + one Accessibility toggle with Touch ID; macOS 27 rule: IM follows Accessibility), onboarding finish (VAL-MACS3-004/005).
- Update rc.4 -> 2.0.0 from the staged local feed (mac-check/s3/updfinal), History survives; lost-grant repair tail (VAL-MACS3-010/011).
- Dictation set on the signed test build at d40493c: VAL-MACS2-009/010, VAL-MACS3-006..009 (parte recipes; insert target com.apple.TextEdit; a locked screen turns on secure input session-wide, so insert checks need an unlocked screen).
- Live-only sweep leftovers (evidence/sweep/findings.md "Not covered"): window chrome, hover, scroll bars, live Light appearance.
- VAL-REL-010 tail: full harness suite + slot-checks on the draft linux-arm64 AppImage (local, one slot at a time). VAL-REL-011: sizes/hashes of the 10 draft artifacts in library/release.md. VAL-LIC-003: notices in all 10.

### Step 5: hp7 validators (automatic)

Same as step 3, for milestone hp7-release.

### Step 6: publish v2.0.0 (feature m7-publish)

Only after every assertion is passed (end-of-mission gate: all 380 assertions "passed" in `validation-state.json`):
1. Tag v2.0.0 on d40493c (the commit run 33 checked) and push it: DONE 2026-10-21; the tag push started no workflow.
2. release.yml with `tag=v2.0.0` = CI run 34 (39650173203): DONE/dispatched; the draft it makes is the final candidate. When it passes, run step 4's checks on the draft artifacts.
2b. 2026-10-21 (owner complaint: permissions re-asked every build): the CI macOS artifacts were ad-hoc signed, so every build was a new app to TCC. The draft's mac zip + dmg were re-signed locally with the stable cert "Apple Development: SURAJ MANDAL (H25U2GA43B)" and SHA256SUMS + latest.json + latest.json.minisig were regenerated with the local key and uploaded (release-verify green). Every later draft check uses the re-signed files. Grants reset once on the rc.4 (ad-hoc) to 2.0.0 (cert) update, then survive. CI still signs ad-hoc; stable CI signing needs an owner-approved key export (gate in library/release.md).
3. Delete the draft releases v2.0.0-rc.1 through rc.4 (`gh release delete`, drafts only; keep the tags).
4. Confirm every assertion passed in validation-state.json, then publish (`gh release edit v2.0.0 --draft=false`).
5. Update `docs/` for Hushpen in one push (CI run 35, starts the docs deploy) and check the site. The repo rename to hushpen is done (owner); docs links must use surajmandalcell/hushpen.

### Step 7: README gate and final report

Create or update the repo-root `README.md` for the final state (what Hushpen is, build/run/test instructions, environment needs). Then the final mission report.

## Hard rules for whoever resumes

- Never steal focus on the Mac. No osascript, JXA, System Events, CGEvent tools, cua-driver, Orca, VMs, or notifications. All computer control (GUI driving, screenshots, app launches and quits for a check) is Codex through `codex exec --dangerously-bypass-approvals-and-sandbox` only, in the background, plus hookctl on debug builds. The orchestrator never launches an app from its own shell.
- The shipping bundle id `io.github.surajmandalcell.hushpen` exists on the owner's Mac only as `/Applications/Hushpen.app` (cert-signed v2.0.0). Test checks use `app.hushpen.test` (cert-signed). No staged copies, no checks that reset or drop a grant (live Allow, lost-grant repair) on the owner's Mac; unit tests cover them. Unregister and delete every copy after a session. Verify the frontmost app before and after every step; stop at the first change.
- Standing go-ahead for all Mac sessions: never ask permission to proceed, never return only to ask. The owner performs no step; a step no allowed driver can do is named as a gap.
- Game rule: nothing synthetic while a full-screen game is in front. Wait it out.
- CI budget: 34 of 35 runs used (hard limit 35). Every new run needs a row in `library/ci-runs.md` before it starts. Remaining: 1 (the docs push). No re-runs are affordable; investigate a failure locally first.
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
