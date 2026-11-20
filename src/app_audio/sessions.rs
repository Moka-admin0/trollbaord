use super::AudioApp;
use crate::win32;
use std::collections::HashMap;
use wasapi::{DeviceEnumerator, Direction, SessionState};

const DISCORD_IS_BLOCKED: &str = "That's Discord itself. Sending it would echo your friends back to them.";

pub fn list_audio_apps(display_names: &mut HashMap<String, String>) -> Vec<AudioApp> {
    let own_pid = std::process::id();
    let own_exe = win32::process_image_path(own_pid).map(|path| exe_name(&path));
    let mut apps: Vec<AudioApp> = Vec::new();

    for (pid, peak) in active_sessions() {
        if pid == own_pid {
            continue;
        }
        let Some(path) = win32::process_image_path(pid) else {
            continue;
        };
        let exe = exe_name(&path);
        if own_exe.as_ref() == Some(&exe) {
            continue;
        }

        if let Some(app) = apps.iter_mut().find(|app| app.exe == exe) {
            app.peak = app.peak.max(peak);
            if !app.pids.contains(&pid) {
                app.pids.push(pid);
            }
            continue;
        }

        let name = display_names.entry(path).or_insert_with_key(|path| display_name(path, &exe)).clone();
        let blocked_reason = exe.starts_with("discord").then_some(DISCORD_IS_BLOCKED);
        apps.push(AudioApp { exe, name, pids: vec![pid], peak, blocked_reason, sending: false, error: None });
    }

    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

fn active_sessions() -> Vec<(u32, f32)> {
    let mut sessions = Vec::new();
    let Ok(enumerator) = DeviceEnumerator::new() else {
        return sessions;
    };
    let Ok(devices) = enumerator.get_device_collection(Direction::Render) else {
        return sessions;
    };

    for device in devices.into_iter().flatten() {
        let Ok(device_sessions) =
            device.get_iaudiosessionmanager().and_then(|manager| manager.get_audiosessionenumerator())
        else {
            continue;
        };
        for index in 0..device_sessions.get_count().unwrap_or(0) {
            let Ok(session) = device_sessions.get_session(index) else {
                continue;
            };
            let Ok(pid) = session.get_process_id() else {
                continue;
            };
            let expired = session.get_state().is_ok_and(|state| state == SessionState::Expired);
            if pid == 0 || expired {
                continue;
            }
            let peak = session.get_audiometerinformation().and_then(|meter| meter.get_peak_value()).unwrap_or(0.0);
            sessions.push((pid, peak));
        }
    }
    sessions
}

fn display_name(path: &str, exe: &str) -> String {
    win32::file_description(path).unwrap_or_else(|| exe.trim_end_matches(".exe").to_owned())
}

fn exe_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_audio::capture::{CAPTURE_RATE, capture_process};
    use crate::audio::{AtomicF32, Feed, FeedReader, peak};
    use std::f32::consts::TAU;
    use std::process::{Child, Command};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
    use std::thread;
    use std::time::{Duration, Instant};

    const TONE_AMPLITUDE: f32 = 0.002;

    fn play_quiet_tone_in_powershell(wav: &std::path::Path) -> Child {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: CAPTURE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(wav, spec).unwrap();
        for i in 0..CAPTURE_RATE * 4 {
            let sample = TONE_AMPLITUDE * (i as f32 * 440.0 * TAU / CAPTURE_RATE as f32).sin();
            writer.write_sample((sample * f32::from(i16::MAX)) as i16).unwrap();
        }
        writer.finalize().unwrap();

        let script = format!("(New-Object Media.SoundPlayer '{}').PlaySync()", wav.display());
        Command::new("powershell").args(["-NoProfile", "-Command", &script]).spawn().unwrap()
    }

    fn wait_for_app(pid: u32) -> AudioApp {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(app) = list_audio_apps(&mut HashMap::new()).into_iter().find(|app| app.pids.contains(&pid)) {
                return app;
            }
            assert!(Instant::now() < deadline, "PowerShell never showed up in the app list");
            thread::sleep(Duration::from_millis(100));
        }
    }

    #[test]
    #[ignore = "plays a nearly silent tone on the default playback device"]
    fn captures_another_process() {
        let wav = std::env::temp_dir().join(format!("trollboard_quiet_{}.wav", std::process::id()));
        let mut player = play_quiet_tone_in_powershell(&wav);
        let pid = player.id();
        let _ = wasapi::initialize_mta();

        let app = wait_for_app(pid);
        assert_eq!(app.exe, "powershell.exe");

        let feed = Arc::new(Feed::new(CAPTURE_RATE));
        let stop = Arc::new(AtomicBool::new(false));
        let capture = thread::spawn({
            let (feed, stop) = (feed.clone(), stop.clone());
            move || capture_process(pid, &feed, &stop).map_err(|error| error.to_string())
        });

        let mut reader = FeedReader::new(feed, CAPTURE_RATE, Arc::new(AtomicF32::new(1.0)));
        let mut buffer = vec![0.0; 960];
        let mut loudest = 0.0_f32;
        let until = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < until {
            thread::sleep(Duration::from_millis(20));
            buffer.fill(0.0);
            reader.mix_into(&mut buffer);
            loudest = loudest.max(peak(&buffer));
        }

        stop.store(true, Relaxed);
        let result = capture.join().unwrap();
        let _ = player.kill();
        let _ = player.wait();
        let _ = std::fs::remove_file(&wav);

        result.unwrap();
        println!("captured peak {loudest:.5} from a {TONE_AMPLITUDE} tone");
        assert!(loudest > TONE_AMPLITUDE / 10.0);
    }
}
