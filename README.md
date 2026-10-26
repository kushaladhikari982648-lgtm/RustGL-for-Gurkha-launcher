# RustGL for Gurkha Launcher

The **RustGL** renderer - a desktop-OpenGL-to-OpenGL-ES compatibility layer
written in Rust - and the packager that turns it into a **universal renderer
plugin APK** for Minecraft: Java Edition launchers on Android.

![RustGL screenshot (original desktop demo)](res/screenshot.png)

## What is in this repository

| Path | What it is |
|---|---|
| `rustgl/` | The renderer: a Rust workspace whose `gl-compat` crate builds `librust_gl.so`. Synced unmodified from [`Gurkha Launcher`](https://github.com/kushaladhikari982648-lgtm/Gurkha-launcher) (`rustgl/`, branch `arena/a8e9723f-gurkha-launcher`, commit `1726782a`). `python3 tools/check_sync.py` proves the copy byte for byte. |
| `android-plugin/` | The plugin: `AndroidManifest.xml` (RendererPlugin v2 + v1 meta-data), the v2 JSON config, and the single information `Activity`. |
| `tools/` | `package_apk.py` (aapt2 + javac + d8 + zipalign + apksigner, no Gradle), `verify_apk.py` (post-build contract checks), `collect_native.py`, `check_sync.py`. |
| `tests/` | Contract tests for the manifest, the v2 config and the packaging helpers - plain Python, no SDK. |
| `src/`, `res/` | The original desktop RustGL demo (SDL2 + OpenGL) this repository started as. Kept for reference; it is not part of the Android plugin. |
| `docs/UNIVERSAL_APK.md` | Install, per-launcher notes, environment variables, limitations, signing and rebuild instructions. |

## The universal APK

`rustgl-universal-<version>.apk` ships `librust_gl.so` for **arm64-v8a,
armeabi-v7a, x86 and x86_64** in one package and declares the plugin contracts
used by the Pojav-family launchers, so the same file can be installed and
selected as a renderer in:

* **Zalith Launcher 2** and **Zalith Launcher 1 / Gurkha Launcher**
* **FCL (Fold Craft Launcher)**
* **Pojav-family forks** that kept the FCL plugin reader (Mojo Launcher and
  others)
* launchers with no plugin loader at all, through the per-ABI ZIP or the
  local-import ZIP published next to it

Both FCL and Zalith Launcher 2 discover plugins with
`queryIntentActivities(Intent.ACTION_MAIN)`, so the APK also contains a tiny
exported `Activity` that explains the plugin - that is what makes it visible to
them, and the reason this plugin is not a pure native package.

**Download:** <https://github.com/kushaladhikari982648-lgtm/RustGL-for-Gurkha-launcher/releases/latest>
- install `rustgl-universal-<version>.apk` (2.3 MB, four ABIs in one file),
- or `rustgl-local-import.zip` for launchers that import renderer folders,
- or `rustgl-gurkha-<abi>.zip` for a bare `librust_gl.so`.

Install, selection and honest limitations: **[docs/UNIVERSAL_APK.md](docs/UNIVERSAL_APK.md)**.

## Build it yourself

GitHub Actions does all of it (`.github/workflows/android.yml`): host renderer
tests against Mesa software GLES, a `cargo-ndk` build for four ABIs, the APK,
its contract verification, and - on a `v*` tag - the release with checksums.

Locally, with Rust, the Android NDK and the Android SDK:

```sh
cd rustgl
cargo test -p renderer-core -p gl-compat -p shader-translate -- --test-threads=1
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 build --release -p gl-compat
cd ..
python3 tools/collect_native.py --target rustgl/target --output artifacts/rustgl
python3 tools/package_apk.py --sdk "$ANDROID_HOME" --output artifacts/release
python3 tools/verify_apk.py artifacts/release/*.apk --sdk "$ANDROID_HOME"
python3 -m unittest discover -s tests -v
```

Set `APK_KEYSTORE_BASE64`, `APK_KEYSTORE_PASSWORD` and `APK_KEY_ALIAS` to sign
with a persistent key; without them the packager generates a one-build key and
prints a warning (a later build then needs the plugin uninstalled first).

## Updating the renderer

Re-sync `rustgl/` from Gurkha Launcher, then record the new tree hash in
`renderer-source.json` (`python3 tools/check_sync.py --record`) and bump
`versionCode`/`versionName` in `android-plugin/AndroidManifest.xml`.

## Licences

The packaging tooling (`android-plugin/`, `tools/`, `tests/`) is MIT licensed;
see [`NOTICE`](NOTICE). The renderer in `rustgl/` comes from
[`whaltermc/rust-renderer`](https://github.com/whaltermc/rust-renderer) (revision
`eb22559`) via Gurkha Launcher and **upstream publishes no licence file** - until
it does, that code is redistributed as "used with the author's permission" and
is not covered by the MIT grant. Keep `NOTICE` with any copy.
