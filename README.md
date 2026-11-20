# trollboard

A Windows soundboard that builds itself from your friends' voices.

trollboard keeps the last 30 seconds of what you hear on Discord. When someone says something
worth keeping, press F9 and it becomes a clip. Press Ctrl+Alt+1 to 9 to play it back through
your mic. It can also send another app's sound, like Spotify, a YouTube tab or a game, to your
friends.

<p align="center">
  <img src="https://i.imgur.com/9DCIwM9.png" alt="The trollboard window showing the Clips tab with two saved clips">
</p>

## Features

- Clip the last 2 to 30 seconds of a call with one hotkey. Silence is trimmed and the volume is
  evened out.
- Play clips into your mic with global hotkeys that also work in games.
- Send any app's audio through your mic using Windows' per-app capture.
- Drag in your own `.wav` files, then rename, reorder or delete clips.
- Works with a GoXLR out of the box, or with the free VB-CABLE driver.

## Requirements

- Windows 10 version 2004 or newer
- A virtual microphone: a GoXLR, or [VB-CABLE](https://vb-audio.com/Cable/)
- Rust 1.95 or newer, to build from source

## Building

```powershell
cargo build --release
```

The app is `target\release\trollboard.exe`. Tagged versions are also built by GitHub Actions and
attached to the release.

## Setup

On first launch trollboard looks for a GoXLR or VB-CABLE and fills in Setup for you. If it finds
neither, Setup opens with instructions.

### GoXLR

1. In Discord's Voice & Video settings, set Output to `Chat (GoXLR)` and Input to `Chat Mic (GoXLR)`.
2. In the GoXLR app's Routing tab, make sure Samples to Chat Mic is ticked.
3. In trollboard's Setup, record from Chat, send clips to Sample, and leave Forward my mic off.

Only Discord plays on the Chat channel, so your clips contain your friends and nothing else.

### VB-CABLE

1. Install VB-CABLE and restart your PC.
2. In Discord, set Input Device to `CABLE Output` and leave Output on your headphones.
3. In trollboard's Setup, record from your headphones, send clips to `CABLE Input`, and turn on
   Forward my mic with your real microphone selected so friends still hear you.

## Usage

| Hotkey         | Action                    |
| -------------- | ------------------------- |
| `F9`           | Clip the last few seconds |
| `Ctrl+Alt+1-9` | Play clip 1 to 9          |
| `F8`           | Play a random clip        |
| `F10`          | Stop playback             |

Hotkeys can be changed in Setup, for example `F9`, `ctrl+shift+c` or `numpad5`. If another app
already uses one, trollboard tells you which.

The header doubles as the title bar: drag it to move the window and double-click it to maximize.

Clips are plain WAV files in `%USERPROFILE%\Music\trollboard`. Set the `TROLLBOARD_CLIPS`
environment variable to use another folder. Settings are stored in `%APPDATA%\trollboard`.

### Sending app audio

The Apps tab lists every app that has audio open. Switch one on and your friends hear it through
your mic while you keep hearing it normally. The slider sets how loud apps are compared to your
voice, and your choices are remembered by program name.

Discord itself can't be sent, because your friends would hear themselves. If music cuts out for
your friends, set Discord's Noise Suppression to None.

With a GoXLR, the Sample channel also plays in your headphones, so you hear shared apps twice.
Untick Samples to Headphones in the GoXLR app and turn on Hear clips myself in Setup.

## Troubleshooting

- **Clips sound cut off or muffled:** set Discord's Noise Suppression to None or Standard and turn
  off Echo Cancellation.
- **Friends only hear clips sometimes:** with push-to-talk, Discord only sends audio while the key
  is held. Voice activity works hands-free.
- **A device is missing:** press Refresh devices in Setup after plugging it in. Running
  `trollboard --setup` opens straight into Setup.

## Development

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test -- --ignored
```

The ignored test plays a nearly silent tone from PowerShell and checks that per-app capture picks
it up, so it needs a working playback device.

| Path             | Contents                                                      |
| ---------------- | ------------------------------------------------------------- |
| `src/audio`      | Devices, stream setup, recording buffer, playback, resampling |
| `src/clips`      | The clip library and WAV files                                |
| `src/engine`     | Device routing, the audio graph and soundboard actions        |
| `src/app_audio`  | Finding apps with audio and capturing them per process        |
| `src/ui`         | The window, built with egui                                   |
| `src/hotkeys.rs` | Global hotkeys                                                |
| `src/win32.rs`   | Small Windows API helpers                                     |

## Consent

Only clip people who are happy to be clipped. Recording someone without their consent is illegal
in some places.

## License

[MIT](LICENSE)
