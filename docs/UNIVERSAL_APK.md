# RustGL universal renderer plugin APK

One installable APK (`rustgl-universal-<version>.apk`) carries `librust_gl.so`
for **arm64-v8a, armeabi-v7a, x86 and x86_64** and speaks three distribution
contracts at once, so the same file works as a renderer plugin in the
Pojav-family launchers that have a plugin loader.

| Contract | Meta-data | Launchers that read it |
|---|---|---|
| RendererPlugin **v2** | `fclPlugin_V2` -> string resource with a JSON config | Zalith Launcher 2 (upstream `ZalithLauncher/ZalithLauncher2`), FCL (Fold Craft Launcher) |
| RendererPlugin **v1** | `fclPlugin` / `zalithRendererPlugin`, `renderer`, `des`, `pojavEnv`, `boatEnv` | Zalith Launcher 1 and its forks (Gurkha Launcher, …), FCL's legacy path, Pojav-family forks that kept the FCL v1 reader (Mojo Launcher and others), Boat/MCinaBox-style readers through `boatEnv` |
| Bare native library | `lib/<abi>/librust_gl.so` | any launcher: use the per-ABI ZIP or the local-import ZIP from the release |

Both FCL and Zalith Launcher 2 find plugins with
`PackageManager.queryIntentActivities(Intent.ACTION_MAIN)`, so the APK ships a
small exported information `Activity`. The launcher selection is still made
inside the launcher: the activity only explains the plugin and holds no settings.

## Install

1. Download `rustgl-universal-<version>.apk` from the release.
2. Allow installation from your browser or file manager if Android asks.
3. Install it, then **fully close and reopen the launcher** (plugin lists are
   cached per process).
4. In the Minecraft version settings, pick **RustGL (Gurkha build)** as the
   renderer. Launchers that read the v1 metadata show it as
   *"RustGL (Gurkha build) - OpenGL 4.5 surface on OpenGL ES 3 [v1 plugin API]"*
   - that is the same renderer, through the older plugin API.
5. Start the game. If it crashes or renders nothing, switch the version back to
   your previous renderer; nothing else is lost.

Some launchers list both the v2 and the v1 entry of this same package. Either
one loads the same `librust_gl.so`; the v2 entry additionally lets you turn
`RENDERER_DEBUG`, `RENDERER_TRACE_GL` and `RENDERER_SPOOF_GL` on and off from
the launcher.

Requirements: **Android 8.0 (API 26) or newer** and real **OpenGL ES 3**
support. Android picks the ABI of the *launcher process*, not of the CPU: a
32-bit launcher on a 64-bit phone uses the armeabi-v7a build.

## What the renderer is

RustGL is a compatibility layer, not a driver. Minecraft talks desktop OpenGL to
`librust_gl.so`, which rewrites the shaders to GLSL ES, emulates desktop-only
entry points (fixed-function drawing, display lists, base-vertex draws, buffer
textures, UBO updates, legacy lightmaps) and forwards to the device's OpenGL ES
3 driver. With `RENDERER_SPOOF_GL=1` it reports an OpenGL 4.5 core profile to
the game and mods.

Environment the plugin sets (`pojavEnv` / v2 `env`):

| Variable | Value | Meaning |
|---|---|---|
| `POJAV_RENDERER` | `opengles3_rust` | renderer id (from `rendererId` in v2) |
| `LIBGL_ES` | `3` | create an OpenGL ES 3 context |
| `RENDERER_BACKEND` | `gles` | GLES passthrough backend (the only one that draws) |
| `RENDERER_SPOOF_GL` | `1` | report OpenGL 4.5 core to the game and mods |
| `JAVA_TOOL_OPTIONS` | `-Dorg.lwjgl.util.NoChecks=true` | LWJGL pointer checks off (1.16 fog workaround) |
| `RENDERER_DEBUG` | `0` | set to `1` for draw diagnostics (slow) |
| `RENDERER_TRACE_GL` | `0` | set to `1` to log every GL call (very slow) |

`JAVA_TOOL_OPTIONS` is only in the v2 config: the v1 `pojavEnv` parser in
Zalith truncates values at the second `=`. If you use the v1 entry on Minecraft
1.16 or older, add it to the version's custom environment by hand.

## Limitations (read before reporting a bug)

- **Compatibility is not universal.** No renderer is: Minecraft versions differ
  in GL profile, shader dialect and mod requirements, and GPU drivers differ in
  what they implement. This plugin was built and unit-tested in CI (unit tests,
  shader fixtures, and GLES pixel read-backs against Mesa on the host); it has
  **not** been validated on every launcher, GPU or Minecraft version. Test a
  clean instance without mods or shader packs first.
- **Launchers without a plugin loader** (stock PojavLauncher, and any fork that
  predates the FCL plugin API) cannot use the APK at all. Use
  `rustgl-gurkha-<abi>.zip` (bare `librust_gl.so`) and the launcher's own
  "custom"/library mechanism, or `rustgl-local-import.zip` for launchers that
  import renderer folders (Zalith, Gurkha: *renderer plugins -> import local*).
- The **v1 entry** is fixed-environment: its `pojavEnv` cannot contain `=` in a
  value, so `JAVA_TOOL_OPTIONS` is omitted there.
- Minecraft versions that create their GL context through SDL3 (26.3+) or that
  need Vulkan are outside what RustGL provides.
- If a device reports OpenGL ES 3 but the driver is incomplete, expect missing
  geometry or a black screen; that is a driver limitation, not a launcher bug.

## Signing and updates

The APK is signed with the repository's release key when the
`APK_KEYSTORE_BASE64` secret is configured, and with a freshly generated
one-build key otherwise (the workflow prints a warning in that case). A
differently signed build cannot update an installed plugin: uninstall the
plugin first. Uninstalling the plugin never touches the launcher or Minecraft
data - it only removes the renderer entry. Verify any download against
`SHA256SUMS` in the release.

## Source, licences, rebuild

- Renderer source: `rustgl/` in this repository, synced unmodified from
  [`Gurkha Launcher`](https://github.com/kushaladhikari982648-lgtm/Gurkha-launcher)
  (`rustgl/`, branch `arena/a8e9723f-gurkha-launcher`, commit `1726782a`).
  `python3 tools/check_sync.py` proves the copy is byte-identical.
- Upstream of that code: <https://github.com/whaltermc/rust-renderer>
  (revision `eb22559`). Upstream publishes **no licence file**; until it does,
  treat the renderer code as "used with the author's permission" and keep the
  notices in `NOTICE` with any redistribution. The packaging tooling in
  `tools/`, `android-plugin/` and `tests/` is MIT licensed.
- Rebuild: `.github/workflows/android.yml` (Rust + `cargo-ndk`, NDK
  25.2.9519653, then `python3 tools/package_apk.py`). Locally:
  `cd rustgl && cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 build
  --release -p gl-compat`, then copy the libraries to
  `artifacts/rustgl/<abi>/` and run `python3 tools/package_apk.py --sdk
  $ANDROID_HOME`, followed by `python3 tools/verify_apk.py` on the result.
