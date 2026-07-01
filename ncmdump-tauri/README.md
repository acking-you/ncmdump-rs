# ncmdump-tauri

Simple Tauri frontend for the `ncmdump-rs` workspace.

## What It Does

- Login to NetEase and Bilibili inside the embedded webview
- Capture and persist session cookies
- Search NetEase tracks in the app
- Search Bilibili videos in the app
- Download NetEase tracks
- Download Bilibili raw best-audio streams

## Current Download Behavior

- NetEase downloads the track directly from the resolved track URL
- Bilibili does **not** use ffmpeg in this app
- Bilibili saves the raw best stream as `.flac`, `.m4a`, or `.m4s`

## Storage

Desktop:
- App work dir uses Tauri app data storage

Android:
- App work dir uses the external files dir
- Typical path:
  `/sdcard/Android/data/com.ncmdump.tauri/files/`

Session files:
- NetEase:
  `.../ncmdump/session.json`
- Bilibili:
  `.../ncmdump/bilibili_session.json`

Downloads:
- Saved directly into the app work dir
- The app shows the full saved path after each download

## How To Use

1. Start the app.
2. Choose `NetEase` or `Bilibili`.
3. Tap `Open ... Login`.
4. Finish login in the same webview.
5. Tap `Capture ...` to save the session.
6. Use the search section to find tracks or videos.
7. Tap `Download` on a result, or use the manual download inputs.

## Build

Frontend build:

```sh
npm run build
```

Rust check:

```sh
cargo check --manifest-path src-tauri/Cargo.toml
```

Android build:

```sh
./build-android.sh --target aarch64
```
