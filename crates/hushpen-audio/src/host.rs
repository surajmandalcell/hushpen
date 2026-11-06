//! Keep the Linux audio server connection alive across device polls and players.

use std::sync::Arc;

pub(crate) fn default_host() -> Arc<cpal::Host> {
    #[cfg(target_os = "linux")]
    {
        // CPAL's PulseAudio reactor can outlive its last client handle. Creating a host on
        // every device poll exhausts the server's client limit. All audio users share one.
        static HOST: std::sync::OnceLock<Arc<cpal::Host>> = std::sync::OnceLock::new();
        Arc::clone(HOST.get_or_init(|| Arc::new(cpal::default_host())))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Arc::new(cpal::default_host())
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::process::Command;
    use std::sync::mpsc;
    use std::time::Duration;

    fn client_count() -> usize {
        let output = Command::new("timeout")
            .args(["5", "pactl", "--format=json", "list", "clients"])
            .output()
            .expect("pactl must be installed");
        assert!(output.status.success(), "pactl must reach the real server");
        let clients: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let name = format!("cpal-pulseaudio-{}", std::process::id());
        clients
            .as_array()
            .unwrap()
            .iter()
            .filter(|client| {
                client["properties"]["application.name"].as_str() == Some(name.as_str())
            })
            .count()
    }

    #[test]
    #[ignore = "needs a real PulseAudio server, pactl, timeout, and a microphone"]
    fn repeated_real_input_lists_keep_one_pulse_connection() {
        // Keep an independent client alive so the process-name filter is checked even if
        // short-lived hosts are correctly released before pactl takes its snapshot.
        let pulse = cpal::host_from_id(cpal::HostId::PulseAudio)
            .expect("the real PulseAudio host must open");
        let baseline = client_count();
        assert!(baseline > 0, "pactl must see the retained PulseAudio host");
        for attempt in 1..=100 {
            let (send, receive) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = send.send(crate::list_inputs());
            });
            let inputs = receive
                .recv_timeout(Duration::from_secs(5))
                .expect("microphone enumeration must finish")
                .expect("microphones must be listed");
            assert!(!inputs.is_empty(), "the real microphone must be found");
            std::thread::sleep(Duration::from_millis(20));
            let clients = client_count();
            assert!(
                clients <= baseline + 1,
                "enumeration {attempt} leaked PulseAudio clients: baseline={baseline}, current={clients}"
            );
        }
        drop(pulse);
    }
}
