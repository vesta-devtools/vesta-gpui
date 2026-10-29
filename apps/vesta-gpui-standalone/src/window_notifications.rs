//! Window-scoped notification return targets (ADR-0173).

use super::*;

impl App {
    pub(super) fn push_toast(&mut self, notice: ToastNotice<DemoAction>, now: Instant) {
        self.expire_toasts(now);
        let id = notice.id;
        let expires_at = notice.expires_at;
        let targets = std::iter::once(STANDALONE_WINDOW)
            .chain(self.native_windows.values().map(|host| host.ui_window))
            .filter_map(|window| {
                self.window_focus(window)
                    .map(|element| InputTarget { window, element })
            })
            .collect::<Vec<_>>();
        if self.demo.toast_queue.push(notice, now).is_ok() {
            self.toast_focus.record(id, expires_at, targets);
            self.request_all_redraw();
        }
    }

    pub(super) fn restore_toast_focus(&mut self, id: ToastId, window: UiWindowId) {
        if let Some(target) = self.toast_focus.opener(id, window) {
            self.restore_editor_opener(target);
        }
    }
    pub(super) fn prune_toast_focus(&mut self, now: Instant) {
        self.toast_focus.prune(now);
    }

    pub(super) fn expire_toasts(&mut self, now: Instant) {
        let previous = self
            .demo
            .toast_queue
            .current()
            .filter(|notice| notice.expires_at <= now)
            .map(|notice| notice.id);
        if self.demo.toast_queue.expire(now) {
            if let Some(id) = previous {
                let windows = std::iter::once(STANDALONE_WINDOW)
                    .chain(self.native_windows.values().map(|host| host.ui_window))
                    .collect::<Vec<_>>();
                for window in windows {
                    let Some(scope) = self.gallery_scopes(window) else {
                        continue;
                    };
                    let root = scope.element(TOAST_ROOT + id.0 * 6);
                    let target = self.window_actions(window).and_then(|route| {
                        let scope =
                            FocusScope::new(window, route, self.focus_root_for_window(window));
                        self.toast_focus.return_target(
                            id,
                            window,
                            self.window_focus(window)
                                .map(|element| InputTarget { window, element }),
                            root,
                            &scope,
                        )
                    });
                    if let Some(target) = target {
                        self.restore_editor_opener(target);
                    }
                }
            }
            self.request_all_redraw();
        }
        self.prune_toast_focus(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::install_action_route;

    #[test]
    fn delayed_notification_actions_expire_before_mutating_and_preserve_live_successors() {
        let mut app = App::new();
        install_action_route(&mut app);
        let opener = app.scopes.header.element(APP_MENU);
        app.set_focus_target(Some(opener));
        let before = app.demo.number;
        let previous_event = Instant::now() - Duration::from_secs(10);
        let expired = ToastId(101);
        let live = ToastId(102);
        for (id, deadline) in [
            (expired, previous_event + Duration::from_secs(4)),
            (live, Instant::now() + Duration::from_secs(60)),
        ] {
            app.push_toast(
                ToastNotice {
                    id,
                    message: format!("Notice {}", id.0),
                    tone: StatusTone::Information,
                    action_label: Some("Increment".to_owned()),
                    action: Some(DemoAction::ToastDetails(id)),
                    expires_at: deadline,
                },
                previous_event,
            );
        }
        install_action_route(&mut app);
        let expired_action = app.scopes.gallery.element(TOAST_ROOT + expired.0 * 6 + 2);
        assert!(app.ui_actions.focus_order().contains(&expired_action));
        app.set_focus_target(Some(expired_action));

        // Emulate an input queued while the host was busy, before the idle
        // expiry callback has had a chance to reconcile its current route.
        app.apply_demo_action(DemoAction::ToastDetails(expired));
        assert_eq!(app.demo.number, before, "expired action must be inert");
        assert_eq!(
            app.demo.toast_queue.current().map(|notice| notice.id),
            Some(live)
        );
        assert_eq!(
            app.input_ownership.focus().map(|focus| focus.element),
            Some(opener)
        );
        assert_eq!(app.toast_focus.len(), 1);
        app.apply_demo_action(DemoAction::DismissToast(expired));
        assert_eq!(
            app.demo.toast_queue.current().map(|notice| notice.id),
            Some(live)
        );

        install_action_route(&mut app);
        let live_action = app.scopes.gallery.element(TOAST_ROOT + live.0 * 6 + 2);
        app.set_focus_target(Some(live_action));
        app.apply_demo_action(DemoAction::ToastDetails(live));
        assert_eq!(app.demo.number, before + 1);
        assert_eq!(
            app.input_ownership.focus().map(|focus| focus.element),
            Some(opener)
        );
        assert!(app.demo.toast_queue.is_empty());
        assert!(app.toast_focus.is_empty());
        app.apply_demo_action(DemoAction::ToastDetails(live));
        assert_eq!(app.demo.number, before + 1, "retired action cannot repeat");

        let outside = app
            .ui_actions
            .focus_order()
            .iter()
            .copied()
            .find(|element| *element != opener && *element != live_action)
            .expect("another live focus target");
        let expired_dismiss = ToastId(103);
        app.push_toast(
            ToastNotice {
                id: expired_dismiss,
                message: "Expired dismissal".to_owned(),
                tone: StatusTone::Information,
                action_label: None,
                action: None,
                expires_at: previous_event + Duration::from_secs(4),
            },
            previous_event,
        );
        install_action_route(&mut app);
        app.set_focus_target(Some(outside));
        app.apply_demo_action(DemoAction::DismissToast(expired_dismiss));
        assert_eq!(
            app.input_ownership.focus().map(|focus| focus.element),
            Some(outside),
            "expired dismissal must not steal focus from outside the notice"
        );
        assert!(app.demo.toast_queue.is_empty());
        assert!(app.toast_focus.is_empty());
    }
}
