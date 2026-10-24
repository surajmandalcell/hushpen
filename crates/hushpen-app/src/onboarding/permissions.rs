//! The permissions step and the repair page: the Allow clicks, the Privacy panes, and the
//! wait for the Accessibility prompt.

use super::{Mode, Onboarding};
use crate::hook;
use gpui_kit::{App, Context};
use hushpen_core::onboarding::{self as rules, Facts, Row, Step};
use hushpen_core::permission::{Access, Permission};
use std::time::Duration;

/// The line under a row of a lost grant whose repair got no prompt from macOS. A record from
/// an older build stays in the Privacy pane and counts as decided, so only a fresh entry helps.
pub const STALE_HINT: &str =
    "If Hushpen is already in the list, remove it with the minus button, then click Allow again.";

/// The line under the Input Monitoring row when its button opened the Accessibility pane.
pub const FOLLOWS_HINT: &str =
    "Input Monitoring comes with Accessibility. Turn Hushpen on in the list that opened.";

/// How long Accessibility gets after its prompt before a still-false trust counts as a stale
/// record. The prompt is an alert of another process, so the answer comes later than the click.
pub(super) const ACCESSIBILITY_PROMPT_WAIT: Duration = Duration::from_secs(2);

/// One Allow click on Accessibility whose answer has not shown yet. Trust that is still false
/// once the wait is over and the user is back in Hushpen sends the user to the Privacy pane.
pub(super) struct AccessibilityWait {
    /// Tells the timer of this click from the timer of an earlier one.
    click: u64,
    /// The click reset a stale record of a lost grant before its request.
    repair: bool,
    waited: bool,
    returned: bool,
}

impl Onboarding {
    /// The line under the row of `permission` when macOS gave no prompt for its repair.
    pub fn stale_hint(&self, permission: Permission) -> Option<&'static str> {
        self.stale.contains(&permission).then_some(STALE_HINT)
    }

    /// The line under the row of `permission`, if it has one.
    pub fn hint(&self, permission: Permission) -> Option<&'static str> {
        self.stale_hint(permission)
            .or_else(|| self.follows.contains(&permission).then_some(FOLLOWS_HINT))
    }

    pub fn facts(&self, cx: &App) -> Facts {
        let controller = self.controller.read(cx);
        let mic = self.mic.read(cx);
        Facts {
            platform: self.platform,
            session: self.session,
            keys: controller.keys_availability(),
            paste: controller.paste_availability(),
            mic_listed: mic.listed(),
            mic_present: !mic.devices().is_empty(),
            mac: controller.access(),
            relaunch: controller.keys_reopen_needed(),
        }
    }

    pub fn rows(&self, cx: &App) -> Vec<Row> {
        rules::permission_rows(&self.facts(cx))
    }

    /// The permissions page is on screen: the first step of the setup, or the repair page.
    pub fn permissions_page_shown(&self) -> bool {
        self.active() && self.step == Step::Permissions
    }

    /// Reads the permissions again, and the page repaints. Reading is live and never prompts.
    /// It does nothing while the permissions page is not on screen.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.permissions_page_shown() {
            return;
        }
        self.controller
            .update(cx, |controller, cx| controller.refresh_permissions(cx));
        let access = self.controller.read(cx).access();
        self.stale
            .retain(|permission| access.get(*permission) != Access::Granted);
        let accessibility_missing = access.get(Permission::Accessibility) != Access::Granted;
        self.follows.retain(|permission| {
            accessibility_missing && access.get(*permission) != Access::Granted
        });
        cx.notify();
    }

    /// The user came back to Hushpen, often from System Settings: the rows show the state now.
    pub fn app_activated(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
        if let Some(wait) = self.accessibility_wait.as_mut() {
            wait.returned = true;
        }
        self.settle_accessibility_wait(cx);
    }

    pub(super) fn reopen_for_repair(&mut self, cx: &mut Context<Self>) {
        self.repair("no-microphone", cx);
    }

    fn repair(&mut self, reason: &str, cx: &mut Context<Self>) {
        if self.mode != Mode::Hidden {
            return;
        }
        self.mode = Mode::Repair;
        self.step = Step::Permissions;
        hook::record_event("onboarding", &format!("repair {reason}"));
        self.sync_gate(cx);
    }

    /// The start reads of the permissions are over. A grant lost then opens the repair page,
    /// and the window comes back, as a start with the loss read at once never hid it. The show
    /// stays behind the front app: a start must never take the front on its own.
    pub(super) fn grants_settled(&mut self, cx: &mut Context<Self>) {
        if !self.waiting_for_grants || self.controller.read(cx).permissions_settling() {
            return;
        }
        self.waiting_for_grants = false;
        if self.controller.read(cx).lost_permissions().is_empty() {
            return;
        }
        self.repair("permission-lost", cx);
        if crate::main_window::away(cx).is_some() {
            crate::main_window::show_unfocused(cx);
        }
    }

    /// A click on Allow: call the request API of the permission, whatever the state is. macOS
    /// then lists Hushpen in the Privacy pane and shows its prompt. Reading the state first
    /// would skip the request for Accessibility and Input Monitoring, which never read as not
    /// asked, and Hushpen would never reach the list.
    ///
    /// macOS shows no prompt to a client it already decided on. Input Monitoring then still
    /// reads Denied right after the request, so the Privacy pane opens, where the denied row
    /// waits for the user's switch. Accessibility always shows its alert, so it waits for the
    /// answer instead. Both checks run once the request is over, which can be after the click
    /// returned.
    ///
    /// A grant that was held and is gone is a stale record from an older build. Its first Allow
    /// click resets Hushpen's record before the request, and the row shows [`STALE_HINT`] if the
    /// permission is still missing. Any other click never resets: a reset would delete the
    /// denied row that the user has to switch on.
    pub fn allow(&mut self, permission: Permission, cx: &mut Context<Self>) {
        let repair = self.repair_needed(permission, cx);
        let pending = self.guide.request(permission, repair);
        hook::record_event("onboarding", &format!("allow {}", permission.key()));
        self.notice = None;
        if repair {
            self.stale.retain(|stale| *stale != permission);
        }
        match pending {
            None => self.requested(permission, repair, cx),
            Some(finished) => cx
                .spawn(async move |this, cx| {
                    let _ = finished.await;
                    let _ = this.update(cx, |me, cx| me.requested(permission, repair, cx));
                })
                .detach(),
        }
        cx.notify();
    }

    /// The permission was granted before, reads missing now, and no repair has run for it yet.
    fn repair_needed(&self, permission: Permission, cx: &Context<Self>) -> bool {
        self.controller
            .read(cx)
            .lost_permissions()
            .contains(&permission)
            && !self.stale.contains(&permission)
    }

    fn requested(&mut self, permission: Permission, repair: bool, cx: &mut Context<Self>) {
        match permission {
            Permission::InputMonitoring | Permission::Microphone => {
                if self.read_access(permission, cx) == Access::Denied {
                    self.open_pane(permission, repair, cx);
                }
            }
            Permission::Accessibility => self.wait_for_accessibility(repair, cx),
        }
        cx.notify();
    }

    /// The permission whose Privacy pane lists the switch for `permission`. On macOS 27 tccd
    /// answers a listen-event request from the Accessibility record, so no request lists Hushpen
    /// under Input Monitoring while Accessibility is missing, and the Accessibility switch
    /// grants both. Input Monitoring that is denied sends the user to that pane first.
    fn pane_of(&self, permission: Permission, cx: &App) -> Permission {
        let access = self.controller.read(cx).access();
        let follows = permission == Permission::InputMonitoring
            && access.get(permission) == Access::Denied
            && access.get(Permission::Accessibility) != Access::Granted;
        if follows {
            Permission::Accessibility
        } else {
            permission
        }
    }

    /// What the system says about `permission` now. It runs only after an Allow click.
    fn read_access(&mut self, permission: Permission, cx: &mut Context<Self>) -> Access {
        self.controller
            .update(cx, |controller, cx| controller.refresh_permissions(cx));
        self.controller.read(cx).access().get(permission)
    }

    /// Opens the Privacy pane of a permission that stayed missing after its request. After a
    /// repair the row also shows [`STALE_HINT`], for a record that the reset could not clear.
    fn open_pane(&mut self, permission: Permission, repaired: bool, cx: &App) {
        let pane = self.pane_of(permission, cx);
        hook::record_event("onboarding", &format!("pane {}", pane.key()));
        if let Err(error) = self.guide.open(pane.settings_url()) {
            log::warn!("System Settings could not be opened: {error}");
        }
        self.note_pane(permission, pane);
        if repaired && !self.stale.contains(&permission) {
            self.stale.push(permission);
        }
    }

    fn note_pane(&mut self, permission: Permission, pane: Permission) {
        self.follows.retain(|each| *each != permission);
        if pane != permission {
            self.follows.push(permission);
        }
    }

    fn wait_for_accessibility(&mut self, repair: bool, cx: &mut Context<Self>) {
        self.allow_clicks += 1;
        let click = self.allow_clicks;
        self.accessibility_wait = Some(AccessibilityWait {
            click,
            repair,
            waited: false,
            returned: false,
        });
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ACCESSIBILITY_PROMPT_WAIT)
                .await;
            let _ = this.update(cx, |me, cx| {
                if let Some(wait) = me.accessibility_wait.as_mut()
                    && wait.click == click
                {
                    wait.waited = true;
                    me.settle_accessibility_wait(cx);
                }
            });
        })
        .detach();
    }

    /// Decides the pending Accessibility click once the wait is over and the user is back: a
    /// grant ends it, trust that is still false opens the pane for the row to switch on.
    fn settle_accessibility_wait(&mut self, cx: &mut Context<Self>) {
        let Some(wait) = &self.accessibility_wait else {
            return;
        };
        if !(wait.waited && wait.returned) {
            return;
        }
        let repair = wait.repair;
        self.accessibility_wait = None;
        if !self.permissions_page_shown() {
            return;
        }
        if self.read_access(Permission::Accessibility, cx) != Access::Granted {
            self.open_pane(Permission::Accessibility, repair, cx);
        }
        cx.notify();
    }

    /// A click on Open Settings: open the Privacy pane that lists the switch of the permission
    /// and nothing else.
    pub fn open_settings(&mut self, permission: Permission, cx: &mut Context<Self>) {
        hook::record_event("onboarding", &format!("open {}", permission.key()));
        let pane = self.pane_of(permission, cx);
        self.note_pane(permission, pane);
        self.notice = self.guide.open(pane.settings_url()).err().map(|_| {
            "System Settings could not be opened. Open Privacy and Security yourself.".to_owned()
        });
        cx.notify();
    }

    /// A click on Quit and Reopen: start a new copy once this one ends, then quit.
    pub fn relaunch(&mut self, cx: &mut Context<Self>) {
        hook::record_event("onboarding", "relaunch");
        match self.guide.relaunch() {
            Ok(()) => cx.quit(),
            Err(error) => {
                log::warn!("Hushpen could not reopen itself: {error}");
                self.notice =
                    Some("Hushpen could not reopen itself. Quit it and open it again.".to_owned());
                cx.notify();
            }
        }
    }
}
