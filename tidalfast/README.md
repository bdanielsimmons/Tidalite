# tidalfast

Tidal, native and fast. A small Rust desktop app (egui, no browser engine) in the spirit of Spotifast:
dark UI, album art everywhere, Home/Library/Search, playlist sidebar, queue panel, shuffle/repeat,
seek + volume, lossless FLAC playback, and gapless-ish next-track prefetching.

Needs an active Tidal subscription. Unofficial: it uses the same endpoints the Tidal apps and the
open-source `tidalapi` library use. No API keys to obtain. Login is the normal "approve in your browser" flow.

## Get the .exe (no Rust needed on your machine)

1. Create a new private repo on GitHub and upload everything in this folder
   (keep the `.github/workflows/build.yml` path exactly; if your upload drops hidden folders, create that file in the GitHub web UI and paste in `build.yml.copy`).
2. Open the repo's **Actions** tab -> the `build` run -> wait ~5-10 min.
3. Download the `tidalfast-windows` artifact (a zip containing `tidalfast.exe`). Run it.

This sidesteps the pip/crates SSL problems you get on a locked-down work machine, because GitHub does the
download and compile, not your PC.

## Build it yourself instead

```
winget install Rustlang.Rustup
cargo build --release
target\release\tidalfast.exe
```

If cargo hits the same SSL/revocation error as pip did (corporate TLS inspection), try:
`set CARGO_HTTP_CHECK_REVOKE=false` and re-run, or ask IT for the proxy CA and set `CARGO_HTTP_CAINFO`.
The app itself uses Windows' own certificate store (schannel), so it trusts your corporate root cert.

## Files

- `src/api.rs`: Tidal login (device code), token refresh, pages, search, stream URLs
- `src/player.rs`: audio thread (rodio + symphonia: FLAC/AAC decode)
- `src/main.rs`: the UI
- Session is stored in `%APPDATA%\tidalfast\session.json`

## If Tidal rotates its public client id

Create `%APPDATA%\tidalfast\credentials.json`:
`{"client_id":"...","client_secret":"..."}` (current values live in the `tidalapi` repo's `session.py`).

## Known limits

- Hi-Res / DASH streams aren't supported yet (falls back to FLAC lossless, then 320k AAC).
- No Tidal Connect, no media keys yet, no lyrics.
- Home page layout comes from Tidal's undocumented page API; if it changes, the app falls back to your Library.
