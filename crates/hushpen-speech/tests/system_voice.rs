//! Speaks with the real system voice and writes the result as a WAV. It needs
//! AVSpeechSynthesizer on macOS or espeak-ng on Linux, so it runs only when asked:
//! `cargo test -p hushpen-speech --test system_voice -- --ignored`. Set `HUSHPEN_SPEECH_WAV`
//! to a path to keep the WAV.

use hushpen_audio::{Player, write_wav};
use hushpen_speech::{
    SAMPLE_RATE, SpeechEngine, SpeechEvent, TextFormat, prepare, synthesize, system_engine,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let asked = std::env::args().any(|arg| arg == "--ignored" || arg == "--include-ignored");
    if !asked {
        println!("system_voice: ignored, pass --ignored to run it");
        return;
    }
    let test = thread::spawn(a_short_phrase_becomes_a_wav);
    run_main_loop_until(&test);
    if test.join().is_err() {
        std::process::exit(1);
    }
    println!("system_voice: ok");
}

/// The system voice delivers its audio on the main queue, as it does under the app's run loop.
#[cfg(target_os = "macos")]
fn run_main_loop_until(test: &thread::JoinHandle<()>) {
    #![allow(unsafe_code)]
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop};

    let run_loop = NSRunLoop::mainRunLoop();
    while !test.is_finished() {
        // SAFETY: the main run loop runs on the main thread, in the default mode.
        unsafe {
            run_loop.runMode_beforeDate(
                NSDefaultRunLoopMode,
                &NSDate::dateWithTimeIntervalSinceNow(0.05),
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn run_main_loop_until(test: &thread::JoinHandle<()>) {
    while !test.is_finished() {
        thread::sleep(Duration::from_millis(50));
    }
}

fn a_short_phrase_becomes_a_wav() {
    let engine: Arc<dyn SpeechEngine> = Arc::from(system_engine());
    let voices = engine.voices().expect("the system voice is available");
    assert!(!voices.is_empty());
    let voice = voices
        .iter()
        .filter(|voice| voice.language.to_ascii_lowercase().starts_with("en"))
        .max_by_key(|voice| voice.quality)
        .map(|voice| voice.id.clone());

    let text = prepare(
        "# Hello\n\nHushpen reads **this** aloud. Then it stops.",
        TextFormat::Markdown,
    )
    .expect("the text has words");
    let synthesis = synthesize(Arc::clone(&engine), text, voice, 1.25);
    let mut samples = Vec::new();
    loop {
        match synthesis.events().recv_timeout(Duration::from_secs(60)) {
            Ok(SpeechEvent::Audio { samples: chunk, .. }) => samples.extend(chunk),
            Ok(SpeechEvent::Done) => break,
            other => panic!("unexpected end: {other:?}"),
        }
    }
    let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
    assert!((1.0..20.0).contains(&seconds), "{seconds} s of speech");
    assert!(
        samples.iter().any(|sample| sample.abs() > 0.05),
        "the speech is silent"
    );

    let dir = tempfile::tempdir().expect("a temp folder");
    let path = dir.path().join("hello.wav");
    write_wav(&path, SAMPLE_RATE, &samples).expect("the WAV is written");
    let player = Player::open(&path).expect("the WAV reads back");
    assert_eq!(
        player.duration_ms(),
        samples.len() as u64 * 1000 / u64::from(SAMPLE_RATE)
    );
    if let Ok(keep) = std::env::var("HUSHPEN_SPEECH_WAV") {
        std::fs::copy(&path, keep).expect("the WAV is copied");
    }
}
