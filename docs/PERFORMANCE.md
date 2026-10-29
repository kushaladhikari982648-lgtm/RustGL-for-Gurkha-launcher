# Renderer performance work

What was changed in the vendored renderer (`rustgl/`), why it matters on the
weakest hardware this plugin targets (PowerVR GE8300 / Cortex-A53 class,
OpenGL ES 3.x, very little memory bandwidth) as well as on a Snapdragon 8 Gen 5
class device, and - just as important - what is **not** claimed.

Nothing in this document is a measured number. No benchmark was run on a phone
to produce it; see [What is not claimed](#what-is-not-claimed).

## What changed

All six changes are inside the renderer's own GL entry points, so they apply to
every launcher the plugin is installed in.

### 1. Per-call lock contention (`crates/gl-compat/src/lib.rs`)

Every GL call the game makes goes through `driver_fn_cached`, which resolves the
driver's function pointer once and caches it. The cache was behind a `Mutex`, so
*every* forwarded call - every draw, uniform, bind, buffer upload - took that
lock even on a cache hit. Minecraft's chunk builder (Sodium, and the vanilla
chunk uploader) issues GL calls from worker threads while the render thread is
also submitting, so all of them queued behind one lock.

The cache is now behind an `RwLock`: the hit path - essentially every call after
startup - takes a shared read lock and only the first resolution of a name takes
the write lock. On a 2-core Cortex-A53 this is the difference between "the
renderer's own bookkeeping" and "the renderer is a bottleneck"; on an 8-core
flagship it stops the shim from serialising threads that the GPU could otherwise
feed in parallel.

### 2. Shader translation is memoized (`crates/gl-compat/src/lib.rs`)

`glShaderSource` rewrites desktop GLSL into GLSL ES before handing the source to
the driver. That work was repeated on every call, and the game repeats it a lot:
each render type is a separate program, and a resource reload, F3+T, a dimension
switch, a world rejoin or a shader-pack option change recompiles all of them
from identical strings. Shader packs make it worse - an Iris/OptiFine pass is
tens of kilobytes of GLSL.

Translation results are now cached, keyed by the source's fingerprint plus the
two inputs that change the output (the ES target and the driver's texture-buffer
support). Bounded, because the low end has 2 GB of RAM for the whole system: at
most 128 entries and 2 MB of text, oldest evicted first, and nothing over
512 KB is cached at all.

### 3. Shader compile and link no longer force a stall (`crates/gl-compat/src/lib.rs`)

`glCompileShader` and `glLinkProgram` used to ask for the result immediately
(`GL_COMPILE_STATUS` / `GL_LINK_STATUS`). That query is precisely the operation
that waits for the driver to finish, and Minecraft compiles and links its
programs in a loop - so the shim was converting a background compile into one
stall per shader, which is most of the hitch when a shader pack loads.

On drivers that advertise `GL_KHR_parallel_shader_compile` (Adreno and Mali do;
PowerVR generally does not) the status is now read lazily, from the
`glGetShaderiv` / `glGetProgramiv` calls the game makes anyway before it uses a
shader. Compiles can then overlap instead of queueing. Nothing is lost: a
failure is still reported once, with the driver's info log and the translated
source - just from the status read, which is the first place the result is
known. On drivers without the extension nothing changes at all: they compile
synchronously, so the query was never the cost.

### 4. Base-vertex emulation stopped allocating per draw (`crates/gl-compat/src/gl/v3_3.rs`)

Drivers without the GL 3.2 base-vertex entry points (some ES 3.0/3.1 DDKs) get
`glDrawElementsBaseVertex` emulated by re-basing the index data on the CPU. That
path allocated, zeroed and freed a staging buffer **per draw**, and chunk
rendering is one draw per section. The buffer now comes from a small thread-local
pool (two buffers, nothing over 1 MB is kept), so the steady state is no
allocation at all. It is thread-local on purpose: chunk building runs on worker
threads, so recycling needs no locking.

Known cost that remains on that path: the element buffer is still mapped
(`glMapBufferRange`) to read the indices back on every emulated draw. Removing
that needs a CPU-side mirror of the index data with a correctness argument about
when the GPU-side copy changes, which is a bigger change than this round.

### 5. The current program is tracked, not queried (`crates/gl-compat/src/lib.rs`, `fixed_draw.rs`)

The fixed-function draw path asked the driver for `GL_CURRENT_PROGRAM` with
`glGetIntegerv` on every `glDrawArrays` - twice for a fixed-function draw: once
to see whether the game had a shader bound, once to put it back. On a driver
where a query means a round trip into the DDK, that is paid per draw, and chunk
rendering is thousands of draws per frame.

`glUseProgram` and `glDeleteProgram` now keep the value in the shim (one atomic
integer), and the draw path reads it. No query, and the save/restore pair around
the internal blit program is gone too: the two entry points that could change it
are the only ones that update the value.

### 6. `GL_QUADS` no longer allocates per draw (`crates/gl-compat/src/fixed_draw.rs`)

ES has no `GL_QUADS`, so a quad draw was expanded into triangle indices on the
CPU - and the index vector was allocated and uploaded **per draw**. Minecraft
1.12.2 and older draw chunks, the GUI and the debug overlay as quads, so this
was the common case on exactly the version range the weakest devices are stuck
on. The index buffer is now cached and only grows; a second quad draw of the
same or smaller size uploads nothing, and a larger one reuses the first run's
indices rather than rebuilding them from scratch.

## Scaling from a GE8300 to a Snapdragon 8 Gen 5

One binary, no device list. The renderer already discovers what the driver can
do at context creation (`crates/gl-compat/src/gles3/mod.rs`) and picks paths
accordingly:

| Capability | Stronger driver takes | Weaker driver falls back to |
|---|---|---|
| `glDrawElementsBaseVertex` present | the driver's entry point | CPU re-base (item 4 above) |
| `GL_KHR_parallel_shader_compile` | overlapping compiles (item 3) | synchronous compile, unchanged |
| ES 3.2 / `GL_EXT_texture_buffer` / `GL_OES_texture_buffer` | texture-buffer shaders | the shader is translated for what the driver has, or rejected with a logged reason instead of rendering blank |
| ES 3.1+ compute, indirect draw, texture view | passthrough | not advertised, so the game never picks the path |

The changes above are deliberately on the paths that cost the *most* on
weak hardware rather than the paths that only matter at the top end: a
Cortex-A53 has no spare core for allocator churn and shader retranslation, while
a flagship's problem is thread serialisation - which item 1 addresses.

## Shader packs

This is the change that makes shader packs usable rather than merely compiling.

**The problem.** OptiFine 1.12.2 and older (and every pack written for it) keeps
drawing with `glVertexPointer` / `glColorPointer` / `glTexCoordPointer` and reads
the fixed-function builtins in its GLSL: `gl_Vertex`, `gl_MultiTexCoord0`,
`gl_ModelViewProjectionMatrix`, `gl_TextureMatrix`, `ftransform()`. GLSL ES has
none of them. Previously the translator replaced the matrices with identity
matrices and left the vertex arrays undeclared: the shader either failed to
compile or compiled and then collapsed the entire world into one place.

**What it does now.**

* `crates/shader-translate/src/lib.rs` rewrites those builtins onto `rust_gl_*`
  attributes and uniforms and declares only the ones a shader actually
  references, using the locations a desktop compatibility profile would use
  (vertex 0, normal 2, colour 3, `gl_MultiTexCoordN` at 8+N), so the numbers
  packs hardcode still line up.
* `crates/gl-compat/src/fixed_draw.rs` resolves those names once per program
  (cached until the program is re-linked) and, when the game draws its
  fixed-function arrays with its own program bound, uploads the client arrays it
  already tracks and pushes the matrix stacks it already tracks. A program with
  none of those names is untouched, so modern Iris and Sodium shaders take
  exactly the path they took before.
* `gl_NormalMatrix` gets the upper-left 3x3 of the modelview matrix. That is
  exact for the rotations and uniform scales Minecraft issues; a full inverse
  transpose per draw is not worth its cost at the bottom of the range.
* `crates/gl-compat/src/gles3/mod.rs` advertises the GLSL-only and format-only
  extensions packs probe before they enable a code path - `GL_ARB_shader_texture_lod`
  (OptiFine turns advertised extensions into the `MC_GL_*` macros its shaders test
  with `#ifdef`), `GL_EXT_shader_texture_lod`, `GL_ARB_depth_texture`,
  `GL_ARB_shadow`, `GL_ARB_texture_float`, `GL_ARB_framebuffer_object`,
  `GL_EXT_texture_array`, and the colour-buffer-float pair - all of which ES 3.x
  genuinely backs. Nothing is advertised that the renderer cannot serve.

**What that gets you, and what it does not.** The two families of packs:

| Pack family | Route | Status |
|---|---|---|
| OptiFine 1.7.10-1.12.2 (GLSL 120, `gl_FragData` MRT, `texture2DLod`, `sampler2DShadow`) | fixed-function arrays + the builtins above | compiles on a real driver and the geometry is now fed; **not run on a device** |
| Iris / Sodium 1.16+ (GLSL 330, `texelFetch` on depth attachments, MRT, uniform blocks) | modern attribute/uniform path | compiles; the game feeds these itself, as before |

Unsupported stages and features - geometry, tessellation, compute, image
load/store, SSBOs - are still rejected with a logged reason rather than silently
rendering blank. That is deliberate: a pack that needs one of them will not work
here, and it says so in the log instead of showing you a black screen.


## Shaders actually compile

"Compiles" here means the driver compiled it, not that the shim shrugged:

* `rustgl/crates/shader-translate/tests/mc_shader_fixtures.rs` holds twenty
  real shader dialects: vanilla 1.21.10 terrain (vertex + fragment, std140
  uniform blocks), 1.21.10 lightmap, 1.21.7 and 1.20.1 position/colour,
  1.12.2 blur, Iris/OptiFine `gbuffers_terrain` (vertex + fragment) and
  `composite`, a Sodium-style chunk shader, a 1.12.2 `attribute`/`varying`
  vertex shader and a 1.17 entity fragment shader - plus the capability-matrix
  fixtures for buffer samplers, explicit UBO bindings, `#moj_import` handling
  and geometry/tessellation/compute rejection.
* `rustgl/tests/render_smoke.py` compiles every one of them through the
  renderer's own `glShaderSource` on a real GLES driver (Mesa, software), in
  three configurations: default, with `GL_EXT_buffer_storage` disabled, and
  with the driver claiming ES 3.1. It asserts the compile status is true, and
  it asserts that an unsupported stage (a geometry shader) *fails* instead of
  silently becoming a blank shader.
* CI runs that on every push (`Renderer tests (host, Mesa GLES)`).

## What is not claimed

* **No performance numbers.** Nothing was measured on a phone, so this document
  quotes none, and no comparison against MobileGL or LTW is implied. What is
  described is where CPU time and stalls were removed, not how many frames
  anybody will gain.
* **No device testing.** The changes were verified by the renderer's own tests
  and the host GLES smoke test; they have not been run on a GE8300, an Adreno or
  a Snapdragon 8 Gen 5. In particular, no shader pack has been run end to end on
  a phone: what is verified is that the shaders compile on a real driver and
  that the geometry and matrices the legacy path reads are fed to it.
* **The shader-pack work is one round, not a guarantee.** Packs are programs,
  and there are thousands of them. Six representative ones compile and the
  legacy builtins are fed from the state the shim already tracks; a pack that
  reads something the shim does not track will still be wrong, and a pack that
  needs a stage ES 3.x has no equivalent for will still be rejected.
* **No rendering change on the modern path.** The performance changes are
  throughput and latency changes, and programs that use no legacy builtin take
  exactly the path they took before - the compile-failure logging in particular
  was moved, not removed.
* **Not a driver.** This layer does not replace the system GLES libraries and
  does not switch engines mid-process; it forwards to the driver the launcher
  picked.

## Verifying

```sh
python3 -m unittest discover -s tests          # plugin + source-sync contract tests
python3 tools/check_sync.py                    # rustgl/ vs the source it was synced from
cd rustgl && cargo test -p renderer-core -p gl-compat -p shader-translate
cargo build -p gl-compat && python3 tests/render_smoke.py
```

`tools/check_sync.py` deserves a note: `rustgl/` is the renderer source from
[`Gurkha Launcher`](https://github.com/kushaladhikari982648-lgtm/Gurkha-launcher)
plus the local changes listed in `renderer-source.json`. The check substitutes
the recorded upstream blob for each documented change and recomputes the tree
hash, so it proves every *other* file is byte-identical to upstream and that no
file was quietly added or removed.
