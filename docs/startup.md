# ASR Pro Open On Sign-In

> TLDR: use Configuration, Application, `Launch at startup`. Turning it on writes the current executable path, so it replaces the saved sign-in target when the app has moved.

## Intent

Opening on sign-in should be simple for normal users and repairable for portable-style installs. If someone moves ASR Pro from one folder to another, enabling it again should point the operating system at the new executable.

| Behavior | Detail |
|---|---|
| Toggle on | Registers the current executable for login. |
| Toggle off | Removes the saved ASR Pro sign-in entry for this install. |
| Toggle on again after moving | Replaces the saved sign-in target with the current executable path. |
| Stored preference | Saved in `asrpro-data/config/app-settings.json` for portable Windows, Linux, and non-Applications macOS installs. |

## Where To Find It

| Screen | Control |
|---|---|
| Configuration | Application |
| Application | Launch at startup |

The row shows the executable path the app currently sees. That path is useful when checking whether a moved install has been reconfigured.

## Platform Behavior

| Platform | Mechanism |
|---|---|
| Windows | Electron login item registration for the current `.exe` path. Portable `.exe` builds use `PORTABLE_EXECUTABLE_FILE` when electron-builder provides it. |
| macOS | Electron login item registration for the current app. |
| Linux | Writes `asrpro.desktop` to the user autostart folder. |

Linux writes to `$XDG_CONFIG_HOME/autostart/asrpro.desktop` when `XDG_CONFIG_HOME` is set, otherwise it writes to `~/.config/autostart/asrpro.desktop`.

## Moving The App

| Scenario | Fix |
|---|---|
| Windows folder moved | Open ASR Pro from the new folder, then turn `Launch at startup` on again. |
| Linux executable moved | Open ASR Pro from the new folder, then turn `Launch at startup` on again. |
| macOS app outside Applications moved | Open the moved `.app`, then turn `Launch at startup` on again. |
| Sign-in opens an old copy | Toggle off, then toggle on. The next enabled state replaces the saved target. |

## Boundaries

| This does | This does not |
|---|---|
| Starts ASR Pro after login. | Sync models or history between folders. |
| Rewrites the sign-in target when enabled from a new path. | Move `asrpro-data/` for the user. |
| Keeps the setting in app config. | Change system security approvals on behalf of the user. |

On macOS, the operating system may still require user approval for some login item changes. ASR Pro reports that state when Electron exposes it.
