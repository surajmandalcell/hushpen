use super::*;

#[test]
fn ssh_pairing_prefers_overlay_addresses_and_keeps_direct_fallbacks() {
    assert_eq!(
        ssh_pair_hosts(Some("office.example"), "100.64.0.1\n", "192.0.2.1"),
        ["100.64.0.1", "office.example", "192.0.2.1"],
    );
    assert_eq!(
        ssh_pair_hosts(Some("100.64.0.1"), "100.64.0.1", "100.64.0.1"),
        ["100.64.0.1"],
    );
    assert_eq!(ssh_pair_hosts(None, "", "192.0.2.1"), ["192.0.2.1"]);
}

#[test]
fn ssh_pairing_rejects_invalid_and_non_overlay_addresses() {
    for output in [
        "100.63.255.255",
        "100.128.0.0",
        "127.0.0.1",
        "100.64.0.1:22",
        "100.64.0.999",
        "name.example",
        "100.64.0.1\n100.64.0.2",
    ] {
        assert_eq!(
            ssh_pair_hosts(None, output, "office.example"),
            ["office.example"]
        );
    }
    assert_eq!(
        ssh_pair_hosts(None, "100.127.255.254", "office.example"),
        ["100.127.255.254", "office.example"],
    );
}

#[test]
fn moving_route_devices_keeps_local_last_and_stops_at_edges() {
    let mut route = Route::new(vec!["phone".into(), "server".into()], false);
    assert!(!route.move_device("phone", true));
    assert!(!route.move_device("server", false));
    assert!(!route.move_device(THIS_COMPUTER, true));
    assert!(!route.move_device("missing", false));
    assert!(route.move_device("server", true));
    assert_eq!(route.ids(), ["server", "phone", THIS_COMPUTER]);
    assert!(route.move_device("server", false));
    assert_eq!(route.ids(), ["phone", "server", THIS_COMPUTER]);
}

#[test]
fn moving_a_device_to_a_place_keeps_local_last() {
    let mut route = route(&["a", "b", "c"], false);
    assert!(route.move_to("c", 0));
    assert_eq!(route.ids(), ["c", "a", "b", THIS_COMPUTER]);
    assert!(route.move_to("c", 1));
    assert_eq!(route.ids(), ["a", "c", "b", THIS_COMPUTER]);
    assert!(
        route.move_to("a", 9),
        "a place past the devices is the last of them"
    );
    assert_eq!(route.ids(), ["c", "b", "a", THIS_COMPUTER]);
    assert!(!route.move_to("a", 2), "the device is already there");
    assert!(!route.move_to(THIS_COMPUTER, 0));
    assert!(!route.move_to("missing", 0));
    let mut remote_only = Route::new(vec!["a".into(), "b".into()], true);
    assert!(remote_only.move_to("a", 1));
    assert_eq!(remote_only.ids(), ["b", "a"]);
}

fn route(ids: &[&str], no_local: bool) -> Route {
    Route::new(ids.iter().map(|id| (*id).to_owned()).collect(), no_local)
}

fn ready(now: u64) -> Health {
    let mut health = Health::default();
    health.record(Probe::Ready { latency_ms: 12 }, now);
    health
}

#[test]
fn the_local_device_is_unique_last_and_removed_only_by_the_switch() {
    let ids = [THIS_COMPUTER, "phone", "phone", "", "server", THIS_COMPUTER];
    assert_eq!(route(&ids, false).ids(), ["phone", "server", THIS_COMPUTER]);
    assert_eq!(route(&ids, true).ids(), ["phone", "server"]);
    assert_eq!(route(&[], false).ids(), [THIS_COMPUTER]);
}

#[test]
fn routes_choose_the_first_ready_device_then_local_or_refuse() {
    let mut health = BTreeMap::new();
    let route = route(&["phone", "server"], false);
    assert_eq!(
        pick(&route, &health, 0),
        Choice::Device(THIS_COMPUTER.into())
    );
    health.insert("server".into(), ready(0));
    assert_eq!(pick(&route, &health, 1), Choice::Device("server".into()));
    health.insert("phone".into(), ready(0));
    assert_eq!(pick(&route, &health, 1), Choice::Device("phone".into()));
    health
        .get_mut("phone")
        .unwrap()
        .record(Probe::Unreachable, 2);
    assert_eq!(pick(&route, &health, 2), Choice::Device("server".into()));
    assert_eq!(
        pick(&route, &health, 30_000),
        Choice::Device(THIS_COMPUTER.into())
    );
    let route = Route::new(route.ids().to_vec(), true);
    assert_eq!(
        pick(&route, &health, 30_000),
        Choice::Refuse(Refusal::NoReadyDevice)
    );
    assert!(!Refusal::NoReadyDevice.message().is_empty());
}

#[test]
fn a_probe_only_delays_the_route_when_the_device_is_due_and_precedes_the_ready_choice() {
    let route = route(&["phone", "server"], false);
    let mut health = BTreeMap::new();
    assert_eq!(next_probe(&route, &health, 0), Some("phone"));
    let mut phone = Health::default();
    phone.record(Probe::Asleep, 0);
    health.insert("phone".into(), phone);
    assert_eq!(next_probe(&route, &health, 999), Some("server"));
    health.insert("server".into(), ready(0));
    assert_eq!(next_probe(&route, &health, 999), None);
    assert_eq!(next_probe(&route, &health, 1000), Some("phone"));
}

#[test]
fn every_probe_result_can_follow_unknown_and_a_success_resets_backoff() {
    for (probe, status) in [
        (Probe::Ready { latency_ms: 4 }, Status::Ready),
        (Probe::Asleep, Status::Asleep),
        (Probe::Unreachable, Status::Unreachable),
        (Probe::ServerModeOff, Status::ServerModeOff),
    ] {
        let mut health = Health::default();
        assert_eq!(health.status, Status::Unknown);
        assert!(health.probe_due(0));
        health.record(probe, 10);
        assert_eq!(health.status, status);
        health.record(Probe::Ready { latency_ms: 5 }, 20);
        assert_eq!(health.latency_ms, Some(5));
        assert!(health.ready(30_019));
        health.tick(30_020);
        assert_eq!(health.status, Status::Unknown);
        assert!(!health.ready(30_020));
        assert!(health.probe_due(30_020));
        health.record(Probe::Unreachable, 30_020);
        assert!(!health.probe_due(31_019));
        assert!(health.probe_due(31_020));
    }
}

#[test]
fn failures_back_off_and_never_overflow_or_exceed_one_minute() {
    let mut health = Health::default();
    for (attempt, delay) in [1000, 2000, 4000, 8000, 16_000, 32_000, 60_000, 60_000]
        .into_iter()
        .enumerate()
    {
        let now = attempt as u64 * 100_000;
        health.record(Probe::Unreachable, now);
        assert!(!health.probe_due(now + delay - 1));
        assert!(health.probe_due(now + delay));
    }
    health.record(Probe::Asleep, u64::MAX - 2);
    assert!(!health.probe_due(u64::MAX - 1));
    assert!(health.probe_due(u64::MAX));
}

#[test]
fn unload_rules_follow_each_platform_and_keep_the_idle_boundary() {
    use UnloadKind::*;
    assert_eq!(allowed_rules(Os::Ios), [WhileServerModeOpen]);
    for os in [Os::Linux, Os::Android] {
        assert_eq!(
            allowed_rules(os),
            [KeepLoaded, AfterIdle, WhenMemoryLow, WhileServerModeOpen]
        );
    }
    for os in [Os::Mac, Os::Windows] {
        assert_eq!(allowed_rules(os), [KeepLoaded, AfterIdle]);
        assert!(!UnloadRule::WhenMemoryLow.allowed_on(os));
    }
    assert!(!UnloadRule::AfterIdle(0).allowed_on(Os::Linux));
    assert!(!UnloadRule::AfterIdle(1441).allowed_on(Os::Linux));
    assert!(!UnloadRule::KeepLoaded.should_unload(u64::MAX, true, false));
    assert!(!UnloadRule::AfterIdle(5).should_unload(299_999, false, true));
    assert!(UnloadRule::AfterIdle(5).should_unload(300_000, false, true));
    assert!(UnloadRule::WhenMemoryLow.should_unload(0, true, true));
    assert!(!UnloadRule::WhenMemoryLow.should_unload(0, false, true));
    assert!(UnloadRule::WhileServerModeOpen.should_unload(0, false, false));
    assert!(!UnloadRule::WhileServerModeOpen.should_unload(0, true, true));
}

fn offer(pairing: &mut Pairing) {
    pairing.apply(
        PairEvent::Offer {
            payload: "QR payload".into(),
            expires_at: 100,
        },
        0,
    );
}

#[test]
fn pairing_progresses_from_offer_to_confirmation_to_paired() {
    let mut pairing = Pairing::default();
    pairing.apply(
        PairEvent::Paired {
            device_id: "phone".into(),
        },
        0,
    );
    assert_eq!(pairing, Pairing::Idle);
    offer(&mut pairing);
    assert!(matches!(pairing, Pairing::Offered { .. }));
    pairing.apply(
        PairEvent::Confirm {
            number: "000123".into(),
            computer_name: "Office computer".into(),
        },
        1,
    );
    assert!(
        matches!(pairing, Pairing::Confirming { ref number, ref computer_name, .. } if number == "000123" && computer_name == "Office computer")
    );
    pairing.apply(
        PairEvent::LocalConfirm {
            number: "000123".into(),
        },
        1,
    );
    pairing.apply(
        PairEvent::PeerConfirm {
            number: "000123".into(),
        },
        1,
    );
    pairing.apply(
        PairEvent::Paired {
            device_id: "phone".into(),
        },
        2,
    );
    assert_eq!(
        pairing,
        Pairing::Paired {
            device_id: "phone".into()
        }
    );
    pairing.apply(PairEvent::Tick, 1000);
    assert!(matches!(pairing, Pairing::Paired { .. }));
    pairing.apply(PairEvent::Cancel, 1000);
    assert_eq!(pairing, Pairing::Idle);
}

#[test]
fn offers_and_confirmations_expire_at_the_exact_deadline() {
    for approvals in 0..=3 {
        let mut pairing = Pairing::Idle;
        offer(&mut pairing);
        if approvals > 0 {
            pairing.apply(
                PairEvent::Confirm {
                    number: "123456".into(),
                    computer_name: "Computer".into(),
                },
                1,
            );
        }
        if approvals > 1 {
            pairing.apply(
                PairEvent::LocalConfirm {
                    number: "123456".into(),
                },
                1,
            );
        }
        if approvals > 2 {
            pairing.apply(
                PairEvent::PeerConfirm {
                    number: "123456".into(),
                },
                1,
            );
        }
        pairing.apply(PairEvent::Tick, 99);
        assert!(!matches!(pairing, Pairing::Failed(_)));
        pairing.apply(
            PairEvent::Paired {
                device_id: "phone".into(),
            },
            100,
        );
        assert_eq!(pairing, Pairing::Failed(PairFailure::Expired));
    }
}

#[test]
fn invalid_offers_numbers_and_rejections_fail_and_can_be_cancelled_or_retried() {
    for (payload, expires_at) in [("", 100), ("  ", 100), ("offer", 0)] {
        let mut pairing = Pairing::Idle;
        pairing.apply(
            PairEvent::Offer {
                payload: payload.into(),
                expires_at,
            },
            0,
        );
        assert_eq!(pairing, Pairing::Failed(PairFailure::InvalidOffer));
        offer(&mut pairing);
        assert!(matches!(pairing, Pairing::Offered { .. }));
    }
    for number in ["12345", "1234567", "12345x", "１２３"] {
        let mut pairing = Pairing::Idle;
        offer(&mut pairing);
        pairing.apply(
            PairEvent::Confirm {
                number: number.into(),
                computer_name: "Computer".into(),
            },
            1,
        );
        assert_eq!(pairing, Pairing::Failed(PairFailure::InvalidNumber));
    }
    for confirm in [false, true] {
        let mut pairing = Pairing::Idle;
        offer(&mut pairing);
        if confirm {
            pairing.apply(
                PairEvent::Confirm {
                    number: "123456".into(),
                    computer_name: "Computer".into(),
                },
                1,
            );
        }
        pairing.apply(PairEvent::Reject, 2);
        assert_eq!(pairing, Pairing::Failed(PairFailure::Rejected));
        pairing.apply(PairEvent::Cancel, 3);
        assert_eq!(pairing, Pairing::Idle);
    }
}

#[test]
fn pairing_needs_both_matching_approvals_before_verified_completion() {
    for peer_first in [false, true] {
        let mut pairing = Pairing::Idle;
        offer(&mut pairing);
        pairing.apply(
            PairEvent::Confirm {
                number: "012345".into(),
                computer_name: "Computer".into(),
            },
            1,
        );
        let paired = PairEvent::Paired {
            device_id: "phone".into(),
        };
        pairing.apply(paired.clone(), 1);
        assert!(matches!(
            pairing,
            Pairing::Confirming {
                local_confirmed: false,
                peer_confirmed: false,
                ..
            }
        ));
        let local = PairEvent::LocalConfirm {
            number: "012345".into(),
        };
        let peer = PairEvent::PeerConfirm {
            number: "012345".into(),
        };
        let (first, second) = if peer_first {
            (peer, local)
        } else {
            (local, peer)
        };
        pairing.apply(first.clone(), 2);
        pairing.apply(first, 2);
        pairing.apply(paired.clone(), 2);
        assert!(matches!(pairing, Pairing::Confirming { .. }));
        pairing.apply(second, 3);
        pairing.apply(
            PairEvent::Paired {
                device_id: String::new(),
            },
            3,
        );
        assert!(matches!(
            pairing,
            Pairing::Confirming {
                local_confirmed: true,
                peer_confirmed: true,
                ..
            }
        ));
        pairing.apply(paired, 3);
        assert_eq!(
            pairing,
            Pairing::Paired {
                device_id: "phone".into()
            }
        );
    }
}

#[test]
fn mismatched_approval_discards_pairing_and_cancel_discards_late_events() {
    for approval in [
        PairEvent::LocalConfirm {
            number: "654321".into(),
        },
        PairEvent::PeerConfirm {
            number: "654321".into(),
        },
    ] {
        let mut pairing = Pairing::Idle;
        offer(&mut pairing);
        pairing.apply(
            PairEvent::Confirm {
                number: "123456".into(),
                computer_name: "Computer".into(),
            },
            1,
        );
        pairing.apply(approval, 2);
        assert_eq!(pairing, Pairing::Failed(PairFailure::Mismatch));
        pairing.apply(
            PairEvent::Paired {
                device_id: "phone".into(),
            },
            3,
        );
        assert_eq!(pairing, Pairing::Failed(PairFailure::Mismatch));
        pairing.apply(PairEvent::Cancel, 4);
        pairing.apply(
            PairEvent::LocalConfirm {
                number: "123456".into(),
            },
            5,
        );
        pairing.apply(
            PairEvent::PeerConfirm {
                number: "123456".into(),
            },
            5,
        );
        pairing.apply(
            PairEvent::Paired {
                device_id: "phone".into(),
            },
            5,
        );
        assert_eq!(pairing, Pairing::Idle);
        offer(&mut pairing);
        pairing.apply(
            PairEvent::Confirm {
                number: "123456".into(),
                computer_name: "Computer".into(),
            },
            1,
        );
        assert!(matches!(
            pairing,
            Pairing::Confirming {
                local_confirmed: false,
                peer_confirmed: false,
                ..
            }
        ));
    }
}

#[test]
fn pairing_rejects_empty_control_and_oversized_computer_names() {
    for computer_name in [
        String::new(),
        "  ".into(),
        "Computer\nPhone".into(),
        "a".repeat(129),
    ] {
        let mut pairing = Pairing::Idle;
        offer(&mut pairing);
        pairing.apply(
            PairEvent::Confirm {
                number: "123456".into(),
                computer_name,
            },
            1,
        );
        assert_eq!(pairing, Pairing::Failed(PairFailure::InvalidName));
    }
    let mut pairing = Pairing::Idle;
    offer(&mut pairing);
    pairing.apply(
        PairEvent::Confirm {
            number: "000000".into(),
            computer_name: "a".repeat(128),
        },
        1,
    );
    assert!(matches!(pairing, Pairing::Confirming { .. }));
}

#[test]
fn the_fallback_notice_names_the_device() {
    assert_eq!(
        fallback_notice("Phone"),
        "Using this computer. Phone did not answer."
    );
}

#[test]
fn the_fallback_notice_only_replaces_the_working_and_result_message() {
    use crate::flow_bar::BarState::*;
    for state in [Transcribing, Result] {
        assert_eq!(working_notice(state, Some("notice")), Some("notice".into()));
        assert_eq!(working_notice(state, None), None);
        assert_eq!(working_notice(state, Some("")), None);
    }
    for state in [Idle, Listening, Undo, Error, Blocked, Cancelled] {
        assert_eq!(working_notice(state, Some("notice")), None);
    }
}
