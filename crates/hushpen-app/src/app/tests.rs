use super::*;
use crate::main_window::testkit::{install_fake, install_reentrant};
use crate::views::View;
use gpui_kit::TestAppContext;

fn shell(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Shell>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Shell::new(window, cx))
        })
        .expect("open test window")
    })
}

#[gpui_kit::test]
fn open_history_shows_the_window_on_the_history_view(cx: &mut TestAppContext) {
    let (handle, shell) = shell(cx);
    let calls = cx.update(|cx| install_fake(cx, handle, true));
    let open = open_history(shell.clone(), None);
    cx.update(|cx| open("row-1", cx));
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.active()),
        View::History
    );
    assert_eq!(calls.shown.get(), 1);
}

#[gpui_kit::test]
fn open_history_makes_its_native_calls_after_the_app_is_free(cx: &mut TestAppContext) {
    let (handle, shell) = shell(cx);
    let native = cx.update(|cx| install_reentrant(cx, handle, true, cx.to_async()));
    let open = open_history(shell, None);
    cx.update(|cx| open("row-1", cx));
    cx.run_until_parked();
    assert_eq!(native.calls.get(), 2, "the order front and the activation");
}

#[gpui_kit::test]
fn a_second_start_shows_the_window_after_the_app_is_free(cx: &mut TestAppContext) {
    let (handle, _shell) = shell(cx);
    let native = cx.update(|cx| install_reentrant(cx, handle, true, cx.to_async()));
    let (requests, shown) = mpsc::unbounded::<()>();
    cx.update(|cx| serve_show_requests(cx, shown));
    requests.unbounded_send(()).unwrap();
    cx.run_until_parked();
    assert_eq!(native.calls.get(), 2, "the order front and the activation");
    requests.unbounded_send(()).unwrap();
    cx.run_until_parked();
    assert_eq!(native.calls.get(), 4);
}
