# Mission 1 (Hushpen 2.0.0): completed work

State as of 2026-10-17. Read this file together with `specs/missions/mission1.todo.md`.

## What the mission is

Hushpen 2.0.0 is a local-first, AGPL-3.0-or-later Wispr Flow clone, rewritten in Rust and GPUI. It replaces the old Electron code on `master` of `surajmandalcell/asrpro`. Hold a key, speak, get cleaned-up text pasted into the app in front. Platforms: macOS arm64 and Linux x64/arm64 on X11. No Windows, no 1.x migration, no code signing.

## Done: milestones

| Milestone | State | Note |
|---|---|---|
| m0-foundation (Electron mission) | done | renderer and main split, security baseline; Electron removed after |
| hp0-foundation | done, validated | Cargo workspace, app shell, store, test hook, Linux harness |
| hp1-engine | done, validated | whisper-rs-sys fork, engine process, mic capture, model library, dictation |
| hp2-insertion | done, validated | pipeline, hold key, insert, guards and permissions, paste-last sounds |
| hp3-history | done, validated | rule cleanup, dictionary, history, audio retention, export |
| hp4-chrome | done, validated | flow bar, tray menu, shortcuts, onboarding, settings and about |
| hp5-native | done, validated | Mac permissions, native chrome, design system, views restyle |
| hp5-llm (code) | done; Mac session S2 part A done, part B pending | LLM worker, smart cleanup, endpoint, undo, command mode; 7 Mac fix features (002d9ef..203c810 chain) |
| hp6-packaging | done, validated | file import, packaging, licenses, update check, self-update |
| hp7-release (partial) | rename-docs, release-workflow, release-candidate done | rc.1 and rc.2 drafts on GitHub; S3 and publish pending |

## Done: release engineering

- `release.yml` workflow plus `xtask release-manifest` and `xtask release-verify` (feature m7-release-workflow, commits a54082b and 9521ee8).
- Update signing key created by the mission: minisign key id `F2DF93A133758E59`. Secret in repo-root `.env` (mode 600, gitignored); tracked `.env.example`; public key `assets/update-key.pub`; GitHub secret set.
- Tag `v2.0.0-rc.1` on 9521ee8, CI run 25 produced a 13-asset draft.
- Tag `v2.0.0-rc.2` on 5805b02. Full CI green (runs 26-29). Linux suite 169 of 169 passed. Notices sha `697ede7f` identical in all 10 artifacts. Both rc drafts still exist on GitHub (m7-publish deletes them).
- Fixes on the way: mic_stream test race (9bfaf2c), engine smoke import fixtures skipped on CI (6e9e6bb), optimized hook build for the start-latency check (19ea9de), idle memory mapping list (5805b02), harness checks documented (bd5ab58, current HEAD).
- Linux idle RAM measured 179-239 MB with a reason recorded in `library/release.md`. Mac idle RSS still to measure (folded into S3).

## Validation state

- Contract: 380 assertions in `validation-contract.md`. Passed: 271. Pending: 109 (owned by S2, S3, m7-publish, and the auto-injected validators for hp5-llm and hp7-release).
- VAL-REL-010 reworded: one pass under the load limits plus slot-checks on the real AppImage.
- VAL-AUP-005 now runs in ubuntu:24.04.
- VAL-ONB-003 is owned by m5n-mac-permissions (component test); it needs no Mac session proof.
- Mac session assertions were reworded in method only (Codex computer use); pass conditions never changed.

## S2 Mac session state (mid-flight)

- Feature `m5-mac-session-s2` is pending with `successState: partial`. Part A passed on 2026-10-17 (run3 in `session.json`).
- Done in the background through Codex computer use plus hookctl: window look and pixel checks (content pane exactly #1E1E1E, neutral selection), appearance switch light/dark/system, launch-at-login toggle, History playback (position moved, no error), TXT export content (byte-equal), tray menu model, first-launch look on an empty data folder, clean quits (0 "already borrowed"/"panicked", 0 SIGTRAP).
- One exit-90 stop: the owner switched the front app to Pumble during a window-locked Codex call; all further Codex calls were stopped per the skill. Remaining captures ride on owner checklist lines and part B capture-only calls.
- Written for the owner: 23-step checklist, about 16 minutes, at `/Volumes/External1TB/data/_custom/droid/hushpen/evidence/m5-mac-session-s2/owner-checklist.md`.
- Running right now: Hushpen test build (instance d2, pids 70145 and 70160, 940x600 window, flow bar on, AI cleanup off) and the watcher `watch.sh` (pid 13289, 8 h deadline from 05:07, paste guard and WAV feeder armed, lock at `evidence/mac-session.lock`). Feed queue `feed-queue.txt` holds 11 lines for checklist steps 2,4,5,6,7,8,10,12,14,16,16.

## CI budget

- 29 runs used of the 35-run cap (hard limit 50), counted from 2026-10-05. Full ledger in `library/ci-runs.md` in the mission dir.
- Remaining planned: 2 for m7-publish (release.yml dispatch for v2.0.0, one docs deploy push). Headroom for re-runs: about 4.
- Rule: publish only packages built on CI ubuntu-22.04 runners (noble container builds need glibc 2.39). In `library/release.md` and the m7 feature descriptions.
- e2e-x64 can fail on an apt DNS flake in "Build the harness image": re-run the failed job once, no code change (proven in run 29).

## Rules the owner set (all still binding)

- Never steal focus. No osascript, JXA, System Events, CGEvent tools, cua-driver, VMs, or notifications on the Mac. The only GUI driver is Codex computer use through `codex exec` with `--dangerously-bypass-approvals-and-sandbox`, background only, plus hookctl. Also saved in `~/.agents/AGENTS.md` and the global `local-runtime` skill.
- Standing go-ahead for all Mac sessions: never ask, never return only to ask.
- Run checks locally; CI capped as above; push in batches; ad-hoc Mac builds.
- The mission creates the signing key (done).
- Game rule: no synthetic input or window changes while a game is in front; verify the frontmost app after each step.
- Rerun the license check after any Cargo.lock change.
- Edit files with the Edit tool only. `/proc` thread names are cut to 15 characters.

## Key paths

- Mission dir (all mission state): `/Users/surajmandal/.factory/missions/f0a389f7-d034-46ca-9118-308ed631c4b8` — `features.json`, `mission.md`, `validation-contract.md`, `validation-state.json`, `architecture.md`, `AGENTS.md`, `services.yaml`, `init.sh`, `skills/` (app-worker, rust-worker, platform-worker, refactor-worker, mac-session-worker), `library/`, `research/`, `validation/`, `handoffs/`.
- Repo: `/Volumes/External1TB/dev/personal/asrpro`, branch master, HEAD bd5ab58, clean tree. Tags v2.0.0-rc.1 (9521ee8) and v2.0.0-rc.2 (5805b02).
- Data root: `/Volumes/External1TB/data/_custom/droid/hushpen` (env.sh, evidence/, tmp/f.json for jq edits).
- S2 evidence: `.../droid/hushpen/evidence/m5-mac-session-s2/` — `session.json` (runs 1-3), `owner-checklist.md`, `bin/cx.sh` (Codex wrapper with the yolo flag), `bin/launch.sh`, `bin/watch.sh`, `scratch.txt`, `feed-queue.txt`, watcher logs (front.log, hook.jsonl, events.log, clip.log, guard.log, frontmost.log).
- Test build under test: `.../droid/hushpen/mac-check/Hushpen.app`, rebuilt from HEAD by `harness/mac-test-app.sh` and signed with the Apple Development identity, so owner grants survive rebuilds. Never run tccutil.
- Older evidence and the reports 01-18 live under the evidence root and `specs/`; reports 01-14 and decisions D-01..D-40 belong to the replaced Electron mission and no longer matter.
