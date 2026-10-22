# Mission 1 (Hushpen 2.0.0): final state and open items

Mission 1 is done. Hushpen v2.0.0 is published (2026-10-22 03:35:39Z): https://github.com/surajmandalcell/hushpen/releases/tag/v2.0.0. All 380 assertions in the mission's `validation-state.json` passed. The docs site (https://asrpro.mandalsuraj.com/, the Pages custom domain) shows Hushpen. History is in `specs/missions/mission1.completed.md` and git.

## Release facts

- Tag `v2.0.0` is on `d40493c`, the commit CI run 33 (ci.yml full) checked. CI run 34 (release.yml) built the draft.
- The macOS zip and dmg in the release were built locally from `bccb29a` (the update-helper fix, macOS only) and signed with the stable cert "Apple Development: SURAJ MANDAL (H25U2GA43B)". `SHA256SUMS`, `latest.json`, and `latest.json.minisig` were regenerated with the release key from `.env`. The eight Linux files are the CI originals.
- CI budget: 35 of 35 runs used. A new release needs a new budget from the owner.

## Open items for the next release

- CI signs macOS ad hoc. An ad-hoc update drops every macOS grant, so never ship one. Stable CI signing needs the cert's private key in a GitHub secret, which is an owner-approved key export. Until then, sign the macOS zip and dmg locally as for v2.0.0.
- The cert expires one day. A new cert makes every grant reset once.
- Linux idle RSS is 240 MB on the software renderer (no GPU in the test slot), over the 150 MB target. About 80 MB is Mesa lavapipe.

## Hard rules (still live)

- Never steal focus on the Mac. All computer control (GUI, screenshots, app launches and quits for a check) goes through `codex exec --dangerously-bypass-approvals-and-sandbox` in the background. Never Orca, osascript, JXA, System Events, CGEvent tools, cua-driver, VMs, or notifications. The orchestrator never launches an app from its own shell.
- The shipping bundle id `io.github.surajmandalcell.hushpen` exists on the owner's Mac only as `/Applications/Hushpen.app`. Test builds use `app.hushpen.test`, signed with the same cert. No staged copies, and no checks that reset or drop a grant on the owner's Mac. Unregister (`lsregister -u`) and delete every test copy after a session.
- A TCC row binds to the signature. A row from an older signature stays switched on in System Settings while macOS ignores it. Check `/Library/Application Support/com.apple.TCC/TCC.db` and `csreq -r <hex> -t` against `codesign -d -r-`.
- Game rule: nothing at all while a full-screen game is in front.
- Data, caches, and build output stay on `/Volumes/External1TB`; agent scratch goes under `/Volumes/External1TB/data/_custom/droid/hushpen`.
- Commit with the git-workflow skill; never add AI attribution; never create branches or worktrees.

## Reference map

- Mission dir: `/Users/surajmandal/.factory/missions/f0a389f7-d034-46ca-9118-308ed631c4b8` (`validation-contract.md`, `validation-state.json`, `features.json`, `library/{ci-runs,release,user-testing}.md`).
- Final evidence: `/Volumes/External1TB/data/_custom/droid/hushpen/evidence/` (`final-candidate/results.md`, `rel010-final/`, `canonical-install/`).
