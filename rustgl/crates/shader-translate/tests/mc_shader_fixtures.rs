//! Translation regressions pinned to the shader dialects real Minecraft versions deliver.
//!
//! Each fixture is the source the game hands to `glShaderSource` after its own `#moj_import`
//! preprocessing (includes inlined, `#version` lines of the includes removed), written out in
//! the same shape as the vanilla files these came from:
//!
//! * 1.21.10: `#version 330`, std140 uniform *blocks* (`DynamicTransforms`, `Projection`,
//!   `Fog`, `LightmapInfo`), UBO instance names (`} lightmapInfo;`), `1.0f` suffixes.
//! * 1.21.7:  the same blocks under `#version 150`.
//! * 1.20.1:  plain `uniform mat4 ModelViewMat;` era.
//! * 1.12.2:  `#version 120` post shaders: `texture2D`, `gl_FragColor`, and the local
//!   variable `sample` (reserved in GLSL ES, legal on desktop).
//!
//! These fixtures guard translation behavior; they do not reproduce the complete game or
//! establish the cause of a device-specific blank screen. The GLES smoke test also compiles
//! these fixtures on a real driver rather than relying only on text assertions.

use shader_translate::translate;

fn assert_ok(src: &str) -> String {
    match translate(src) {
        Ok(t) => t,
        Err(e) => panic!("translate() rejected a vanilla shader: {e}\n--- source ---\n{src}"),
    }
}

/// The 1.21.10 terrain vertex shader (fog.glsl + dynamictransforms.glsl + projection.glsl
/// inlined the way the game's preprocessor delivers them).
const TERRAIN_VSH_1_21_10: &str = r#"#version 330

layout(std140) uniform Fog {
    vec4 FogColor;
    float FogEnvironmentalStart;
    float FogEnvironmentalEnd;
    float FogRenderDistanceStart;
    float FogRenderDistanceEnd;
    float FogSkyEnd;
    float FogCloudsEnd;
};

float fog_spherical_distance(vec3 pos) {
    return length(pos);
}

float fog_cylindrical_distance(vec3 pos) {
    float distXZ = length(pos.xz);
    float distY = abs(pos.y);
    return max(distXZ, distY);
}

layout(std140) uniform DynamicTransforms {
    mat4 ModelViewMat;
    vec4 ColorModulator;
    vec3 ModelOffset;
    mat4 TextureMat;
    float LineWidth;
};

layout(std140) uniform Projection {
    mat4 ProjMat;
};

in vec3 Position;
in vec4 Color;
in vec2 UV0;
in ivec2 UV2;
in vec3 Normal;

uniform sampler2D Sampler2;

out float sphericalVertexDistance;
out float cylindricalVertexDistance;
out vec4 vertexColor;
out vec2 texCoord0;

vec4 minecraft_sample_lightmap(sampler2D lightMap, ivec2 uv) {
    return texture(lightMap, clamp((uv / 256.0) + 0.5 / 16.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0)));
}

void main() {
    vec3 pos = Position + ModelOffset;
    gl_Position = ProjMat * ModelViewMat * vec4(pos, 1.0);

    sphericalVertexDistance = fog_spherical_distance(pos);
    cylindricalVertexDistance = fog_cylindrical_distance(pos);
    vertexColor = Color * minecraft_sample_lightmap(Sampler2, UV2);
    texCoord0 = UV0;
}
"#;

/// The 1.21.10 terrain fragment shader (fog + dynamictransforms inlined), with the
/// alpha-cutout variant's `#define` the game injects for cutout render types.
const TERRAIN_FSH_1_21_10: &str = r#"#version 330

layout(std140) uniform Fog {
    vec4 FogColor;
    float FogEnvironmentalStart;
    float FogEnvironmentalEnd;
    float FogRenderDistanceStart;
    float FogRenderDistanceEnd;
    float FogSkyEnd;
    float FogCloudsEnd;
};

float linear_fog_value(float vertexDistance, float fogStart, float fogEnd) {
    if (vertexDistance <= fogStart) {
        return 0.0;
    } else if (vertexDistance >= fogEnd) {
        return 1.0;
    }

    return (vertexDistance - fogStart) / (fogEnd - fogStart);
}

float total_fog_value(float sphericalVertexDistance, float cylindricalVertexDistance, float environmentalStart, float environmantalEnd, float renderDistanceStart, float renderDistanceEnd) {
    return max(linear_fog_value(sphericalVertexDistance, environmentalStart, environmantalEnd), linear_fog_value(cylindricalVertexDistance, renderDistanceStart, renderDistanceEnd));
}

vec4 apply_fog(vec4 inColor, float sphericalVertexDistance, float cylindricalVertexDistance, float environmentalStart, float environmantalEnd, float renderDistanceStart, float renderDistanceEnd, vec4 fogColor) {
    float fogValue = total_fog_value(sphericalVertexDistance, cylindricalVertexDistance, environmentalStart, environmantalEnd, renderDistanceStart, renderDistanceEnd);
    return vec4(mix(inColor.rgb, fogColor.rgb, fogValue * fogColor.a), inColor.a);
}

layout(std140) uniform DynamicTransforms {
    mat4 ModelViewMat;
    vec4 ColorModulator;
    vec3 ModelOffset;
    mat4 TextureMat;
    float LineWidth;
};

uniform sampler2D Sampler0;

in float sphericalVertexDistance;
in float cylindricalVertexDistance;
in vec4 vertexColor;
in vec2 texCoord0;

out vec4 fragColor;

void main() {
    vec4 color = texture(Sampler0, texCoord0) * vertexColor * ColorModulator;
#ifdef ALPHA_CUTOUT
    if (color.a < ALPHA_CUTOUT) {
        discard;
    }
#endif
    fragColor = apply_fog(color, sphericalVertexDistance, cylindricalVertexDistance, FogEnvironmentalStart, FogEnvironmentalEnd, FogRenderDistanceStart, FogRenderDistanceEnd, FogColor);
}
"#;

/// The 1.21.10 lightmap fragment shader: a std140 block with an *instance name*, member
/// access through it, `1.0f` float suffixes, and a local `notGamma` shadowing a function.
const LIGHTMAP_FSH_1_21_10: &str = r#"#version 330

layout(std140) uniform LightmapInfo {
    float AmbientLightFactor;
    float SkyFactor;
    float BlockFactor;
    float NightVisionFactor;
    float DarknessScale;
    float DarkenWorldFactor;
    float BrightnessFactor;
    vec3 SkyLightColor;
    vec3 AmbientColor;
} lightmapInfo;

in vec2 texCoord;

out vec4 fragColor;

float get_brightness(float level) {
    return level / (4.0 - 3.0 * level);
}

vec3 notGamma(vec3 color) {
    float maxComponent = max(max(color.x, color.y), color.z);
    float maxInverted = 1.0f - maxComponent;
    float maxScaled = 1.0f - maxInverted * maxInverted * maxInverted * maxInverted;
    return color * (maxScaled / maxComponent);
}

void main() {
    float block_brightness = get_brightness(floor(texCoord.x * 16) / 15) * lightmapInfo.BlockFactor;
    float sky_brightness = get_brightness(floor(texCoord.y * 16) / 15) * lightmapInfo.SkyFactor;

    vec3 color = vec3(
        block_brightness,
        block_brightness * ((block_brightness * 0.6 + 0.4) * 0.6 + 0.4),
        block_brightness * (block_brightness * block_brightness * 0.6 + 0.4)
    );

    color = mix(color, lightmapInfo.AmbientColor, lightmapInfo.AmbientLightFactor);

    color += lightmapInfo.SkyLightColor * sky_brightness;
    color = mix(color, vec3(0.75), 0.04);

    if (lightmapInfo.AmbientLightFactor == 0.0f) {
        vec3 darkened_color = color * vec3(0.7, 0.6, 0.6);
        color = mix(color, darkened_color, lightmapInfo.DarkenWorldFactor);
    }

    if (lightmapInfo.NightVisionFactor > 0.0) {
        float max_component = max(color.r, max(color.g, color.b));
        if (max_component < 1.0) {
            vec3 bright_color = color / max_component;
            color = mix(color, bright_color, lightmapInfo.NightVisionFactor);
        }
    }

    if (lightmapInfo.AmbientLightFactor == 0.0f) {
        color = color - vec3(lightmapInfo.DarknessScale);
    }

    color = clamp(color, 0.0, 1.0);

    vec3 notGamma = notGamma(color);
    color = mix(color, notGamma, lightmapInfo.BrightnessFactor);
    color = mix(color, vec3(0.75), 0.04);

    fragColor = vec4(color, 1.0);
}
"#;

/// The 1.21.7 position_color vertex shader: same UBO shape under `#version 150`.
const POSITION_COLOR_VSH_1_21_7: &str = r#"#version 150

layout(std140) uniform DynamicTransforms {
    mat4 ModelViewMat;
    vec4 ColorModulator;
    vec3 ModelOffset;
    mat4 TextureMat;
    float LineWidth;
};

layout(std140) uniform Projection {
    mat4 ProjMat;
};

in vec3 Position;
in vec4 Color;

out vec4 vertexColor;

void main() {
    gl_Position = ProjMat * ModelViewMat * vec4(Position, 1.0);

    vertexColor = Color;
}
"#;

/// The 1.20.1-era shape: plain uniforms, no blocks.
const POSITION_COLOR_VSH_1_20_1: &str = r"#version 150

uniform mat4 ModelViewMat;
uniform mat4 ProjMat;
uniform int FogShape;
uniform vec4 FogColor;
uniform float FogStart;
uniform float FogEnd;

in vec3 Position;
in vec4 Color;

out vec4 vertexColor;
out float vertexDistance;

float linear_fog_value(float vertexDistance, float fogStart, float fogEnd) {
    if (vertexDistance <= fogStart) {
        return 0.0;
    } else if (vertexDistance >= fogEnd) {
        return 1.0;
    }
    return (vertexDistance - fogStart) / (fogEnd - fogStart);
}

float fog_distance(mat4 modelViewMat, vec3 pos, int shape) {
    if (shape == 0) {
        return length((modelViewMat * vec4(pos, 1.0)).xyz);
    }
    return length((modelViewMat * vec4(pos, 1.0)).xz);
}

void main() {
    gl_Position = ProjMat * ModelViewMat * vec4(Position, 1.0);
    vertexColor = Color;
    vertexDistance = fog_distance(ModelViewMat, Position, FogShape);
}
";

/// Minecraft 1.12.2's blur.fsh (shaders/program), verbatim: `#version 120`, `texture2D`,
/// `gl_FragColor`, and a local named `sample`.
const BLUR_FSH_1_12_2: &str = r"#version 120

uniform sampler2D DiffuseSampler;

varying vec2 texCoord;
varying vec2 oneTexel;

uniform vec2 InSize;

uniform vec2 BlurDir;
uniform float Radius;

void main() {
    vec4 blurred = vec4(0.0);
    float totalStrength = 0.0;
    float totalAlpha = 0.0;
    float totalSamples = 0.0;
    for(float r = -Radius; r <= Radius; r += 1.0) {
        vec4 sample = texture2D(DiffuseSampler, texCoord + oneTexel * r * BlurDir);

        totalAlpha = totalAlpha + sample.a;
        totalSamples = totalSamples + 1.0;

        float strength = 1.0 - abs(r / Radius);
        totalStrength = totalStrength + strength;
        blurred = blurred + sample;
    }
    gl_FragColor = vec4(blurred.rgb / (Radius * 2.0 + 1.0), totalAlpha);
}
";

#[test]
fn terrain_vsh_1_21_10_keeps_std140_blocks_and_members() {
    let out = assert_ok(TERRAIN_VSH_1_21_10);
    assert!(out.starts_with("#version 300 es"), "{out}");
    // Every uniform block and its members must survive verbatim: the game binds them with
    // glGetUniformBlockIndex("DynamicTransforms")/("Projection")/("Fog"), and the shader
    // reads the members directly. Losing either zeroes the transforms -> black screen.
    for block in ["DynamicTransforms", "Projection", "Fog"] {
        assert!(
            out.contains(&format!("layout(std140) uniform {block}")),
            "block {block} lost:\n{out}"
        );
    }
    for member in [
        "mat4 ModelViewMat;",
        "vec4 ColorModulator;",
        "vec3 ModelOffset;",
        "mat4 TextureMat;",
        "float LineWidth;",
        "mat4 ProjMat;",
        "vec4 FogColor;",
        "float FogEnvironmentalStart;",
    ] {
        assert!(out.contains(member), "member {member} lost:\n{out}");
    }
    assert!(out.contains("gl_Position = ProjMat * ModelViewMat"), "{out}");
    // The integer attribute and its lightmap call must survive, with the ivec2/float
    // division made explicit for ES (no implicit conversions).
    assert!(out.contains("in ivec2 UV2;"), "{out}");
    assert!(out.contains("minecraft_sample_lightmap"), "{out}");
    // No leftover preprocessing, no stub-replacing errors.
    assert!(!out.contains("#moj_"), "{out}");
}

#[test]
fn terrain_fsh_1_21_10_keeps_frag_out_and_fog_call() {
    let out = assert_ok(TERRAIN_FSH_1_21_10);
    assert!(out.starts_with("#version 300 es"), "{out}");
    // The shader declares its own `out vec4 fragColor` -- it must NOT be rewritten onto the
    // legacy rust_FragColor path (there is no gl_FragColor here).
    assert!(out.contains("out vec4 fragColor;"), "{out}");
    assert!(out.contains("fragColor = apply_fog("), "{out}");
    assert!(!out.contains("rust_FragColor"), "{out}");
    assert!(out.contains("layout(std140) uniform Fog"), "{out}");
    assert!(out.contains("layout(std140) uniform DynamicTransforms"), "{out}");
    assert!(out.contains("FogEnvironmentalStart"), "{out}");
    assert!(out.contains("FogRenderDistanceEnd"), "{out}");
    // The #ifdef body must survive for the alpha-cutout variant.
    assert!(out.contains("ALPHA_CUTOUT"), "{out}");
}

#[test]
fn lightmap_fsh_1_21_10_keeps_instance_named_block_and_shadowing() {
    let out = assert_ok(LIGHTMAP_FSH_1_21_10);
    assert!(out.starts_with("#version 300 es"), "{out}");
    // Instance-named block and the member access through it.
    assert!(out.contains("} lightmapInfo;"), "{out}");
    assert!(out.contains("lightmapInfo.BlockFactor"), "{out}");
    assert!(out.contains("lightmapInfo.AmbientLightFactor"), "{out}");
    // The local that shadows its own helper must survive untouched.
    assert!(out.contains("vec3 notGamma = notGamma(color);"), "{out}");
    // Desktop float suffixes must be stripped for ES.
    assert!(!out.contains("1.0f"), "{out}");
    assert!(out.contains("fragColor = vec4(color, 1.0);"), "{out}");
}

#[test]
fn position_color_1_21_7_and_1_20_1_translate() {
    let out = assert_ok(POSITION_COLOR_VSH_1_21_7);
    assert!(out.contains("layout(std140) uniform DynamicTransforms"), "{out}");
    assert!(out.contains("layout(std140) uniform Projection"), "{out}");
    assert!(out.contains("gl_Position = ProjMat * ModelViewMat"), "{out}");

    let out = assert_ok(POSITION_COLOR_VSH_1_20_1);
    // Plain uniforms must stay plain declarations -- macroize must not fold them.
    assert!(out.contains("uniform mat4 ModelViewMat;"), "{out}");
    assert!(out.contains("uniform mat4 ProjMat;"), "{out}");
    assert!(!out.contains("#define ModelViewMat"), "{out}");
    assert!(out.contains("float fog_distance(mat4 modelViewMat, vec3 pos, int shape)"), "{out}");
}

#[test]
fn blur_fsh_1_12_2_maps_frag_color_and_reserved_sample() {
    let out = assert_ok(BLUR_FSH_1_12_2);
    assert!(out.starts_with("#version 300 es"), "{out}");
    // gl_FragColor-era fragment shader: the output declaration is injected and every write
    // retargeted; missing either is an ES compile error.
    assert!(
        out.contains("layout(location = 0) out vec4 rust_FragColor;"),
        "{out}"
    );
    assert!(out.contains("rust_FragColor = vec4(blurred.rgb"), "{out}");
    assert!(!out.contains("gl_FragColor"), "{out}");
    // texture2D -> texture, varyings -> in.
    assert!(out.contains("texture(DiffuseSampler,"), "{out}");
    assert!(!out.contains("texture2D("), "{out}");
    assert!(out.contains("in vec2 texCoord;"), "{out}");
    assert!(!out.contains("varying "), "{out}");
    // `sample` is reserved in GLSL ES 3.00 and must be renamed consistently.
    assert!(!out.contains("vec4 sample"), "{out}");
    assert!(out.contains("rs_id_sample"), "{out}");
}

#[test]
fn ubo_explicit_binding_is_stripped_but_std140_kept() {
    let src = r#"#version 330
layout(std140, binding = 0) uniform DynamicTransforms {
    mat4 ModelViewMat;
};
in vec3 Position;
void main() {
    gl_Position = ModelViewMat * vec4(Position, 1.0);
}
"#;
    let out = assert_ok(src);
    assert!(out.contains("layout(std140) uniform DynamicTransforms"), "{out}");
    assert!(!out.contains("binding"), "{out}");
    assert!(!out.contains("layout()"), "{out}");
    assert!(!out.contains(", )"), "{out}");
}

#[test]
fn geometry_tess_and_compute_are_rejected_not_stubbed_silently() {
    assert!(translate("#version 330\nvoid main(){ EmitVertex(); }\n").is_err());
    assert!(translate("#version 330\nlayout(local_size_x=1) in;\nvoid main(){}\n").is_err());
}

#[test]
fn mojang_directives_are_dropped_and_version_collapsed() {
    // The game sometimes leaves stray directive lines and per-include #version lines.
    let src = "#version 330\n#version 150\n#moj_import <minecraft:fog.glsl>\n#moj_pack x\nin vec3 Position;\nvoid main(){ gl_Position = vec4(Position, 1.0); }\n";
    let out = assert_ok(src);
    assert_eq!(out.matches("#version").count(), 1, "{out}");
    assert!(!out.contains("#moj_"), "{out}");
}

// Reduced modern cloud/fog dialects. Not full game shaders or a gameplay claim.
const MC_MODERN_VSH_BUFFER: &str = r#"#version 330
uniform isamplerBuffer CloudFaces;
flat out int face;
void main() {
    face = texelFetch(CloudFaces, gl_VertexID / 6).r;
    gl_Position = vec4(float(face), 0.0, 0.0, 1.0);
}
"#;

const MC_MODERN_FSH_FOG_CALL: &str = r#"#version 330
in float vertexDistance;
uniform float FogCloudsEnd;
out vec4 fragColor;
float linear_fog_value(float vertexDistance, float fogStart, float fogEnd) {
    if (vertexDistance <= fogStart) return 0.0;
    if (vertexDistance >= fogEnd) return 1.0;
    return (vertexDistance - fogStart) / (fogEnd - fogStart);
}
void main() {
    fragColor = vec4(linear_fog_value(vertexDistance, 0, FogCloudsEnd));
}
"#;

#[test]
fn buffer_samplers_follow_driver_capabilities() {
    use shader_translate::{translate_with_texture_buffers, TextureBufferSupport::*};
    assert!(translate_with_texture_buffers(MC_MODERN_VSH_BUFFER, None).is_err());
    for (cap, version, extension) in [
        (Core32, "320", ""), (Ext31, "310", "GL_EXT_texture_buffer"),
        (Oes31, "310", "GL_OES_texture_buffer"),
    ] {
        let text = translate_with_texture_buffers(MC_MODERN_VSH_BUFFER, cap).unwrap();
        assert!(text.starts_with(&format!("#version {version} es\n")), "{text}");
        let partner = translate_with_texture_buffers("#version 330\nvoid main(){ gl_Position = vec4(0.0); }", cap).unwrap();
        assert!(partner.starts_with(&format!("#version {version} es\n")), "stages must link: {partner}");
        assert!(text.contains("precision highp isamplerBuffer;"), "{text}");
        assert!(text.contains("texelFetch(CloudFaces, gl_VertexID / 6)"), "{text}");
        if !extension.is_empty() {
            let directive = format!("#extension {extension} : require");
            assert!(text.find(&directive).unwrap() < text.find("precision").unwrap());
        }
    }
}

#[test]
fn fog_call_uses_the_declared_float_parameter() {
    let text = shader_translate::translate(MC_MODERN_FSH_FOG_CALL).unwrap();
    assert!(text.contains("linear_fog_value(vertexDistance, float(0), FogCloudsEnd)"), "{text}");
}

/// Iris / OptiFine `gbuffers_terrain.vsh`: GLSL 330 with the attribute names a shader pack
/// injects (`mc_Entity`, `at_tangent`) and no uniform blocks. This is the dialect most shader
/// packs still ship, and the one that decides whether a pack works at all.
const GBUFFERS_TERRAIN_VSH_IRIS: &str = r#"#version 330

in vec3 Position;
in vec4 Color;
in vec2 UV0;
in ivec2 UV2;
in vec3 Normal;
in vec4 at_tangent;
in vec3 mc_Entity;

uniform mat4 modelViewMatrix;
uniform mat4 projectionMatrix;
uniform vec3 cameraPosition;
uniform float frameTimeCounter;

out vec4 texcoord;
out vec4 lmcoord;
out vec4 color;
out vec3 normal;
out vec3 viewPos;
out float entityId;

void main() {
    vec4 viewPosition = modelViewMatrix * vec4(Position, 1.0);
    gl_Position = projectionMatrix * viewPosition;

    texcoord = vec4(UV0, 0.0, 0.0);
    lmcoord = vec4(vec2(UV2) / 255.0, 0.0, 1.0);
    color = Color;
    normal = normalize((modelViewMatrix * vec4(Normal, 0.0)).xyz);
    viewPos = viewPosition.xyz;
    entityId = mc_Entity.x;
}
"#;

/// Iris / OptiFine `gbuffers_terrain.fsh`: alpha test, entity colour tint and a hand-written
/// fragment output - everything here must survive verbatim or the pack renders black.
const GBUFFERS_TERRAIN_FSH_IRIS: &str = r#"#version 330

uniform sampler2D gtexture;
uniform sampler2D lightmap;
uniform vec4 entityColor;
uniform float alphaTestRef;

in vec4 texcoord;
in vec4 lmcoord;
in vec4 color;
in vec3 normal;
in float entityId;

out vec4 fragColor;

void main() {
    vec4 albedo = texture(gtexture, texcoord.st) * color;
    albedo.rgb = mix(albedo.rgb, entityColor.rgb, entityColor.a);
    if (albedo.a < alphaTestRef) {
        discard;
    }
    vec3 light = texture(lightmap, lmcoord.st).rgb;
    fragColor = vec4(albedo.rgb * light, albedo.a);
}
"#;

/// Iris / OptiFine `composite.fsh`: a full-screen deferred pass that reads a colour
/// attachment through `gl_FragCoord` - the pattern every post-processing stage uses.
const COMPOSITE_FSH_IRIS: &str = r#"#version 330

uniform sampler2D colortex0;
uniform sampler2D colortex1;
uniform float viewWidth;
uniform float viewHeight;
uniform float frameTimeCounter;

in vec2 texCoord;

out vec4 fragColor;

void main() {
    vec2 coord = gl_FragCoord.xy / vec2(viewWidth, viewHeight);
    vec3 color = texture(colortex0, coord).rgb;
    float depth = texture(colortex1, coord).r;
    float pulse = 0.5 + 0.5 * sin(frameTimeCounter);
    fragColor = vec4(color * mix(1.0, pulse, 0.25), depth);
}
"#;

/// Sodium-style chunk vertex shader: GLSL 330, one std140 block for the section offset that
/// the mod updates per section, and plain uniforms for the matrices.
const SODIUM_CHUNK_VSH_0_6: &str = r#"#version 330

layout(std140) uniform ChunkOffset {
    vec4 u_ChunkOffset;
};

uniform mat4 u_ModelViewProjectionMatrix;

in vec3 aPosition;
in vec4 aColor;
in vec2 aTexCoord;
in vec2 aLightCoord;

out vec4 v_Color;
out vec2 v_TexCoord;
out vec2 v_LightCoord;

void main() {
    vec3 position = aPosition + u_ChunkOffset.xyz;
    gl_Position = u_ModelViewProjectionMatrix * vec4(position, 1.0);
    v_Color = aColor;
    v_TexCoord = aTexCoord;
    v_LightCoord = aLightCoord;
}
"#;

/// Minecraft 1.12.2 `block.vsh`: `#version 120` with `attribute` / `varying`, the oldest
/// dialect the game still ships in 1.7-1.12 modpacks.
const LEGACY_VSH_1_12_2: &str = r#"#version 120

attribute vec3 Position;
attribute vec4 Color;
attribute vec2 UV0;

uniform mat4 ModelViewMat;
uniform mat4 ProjMat;

varying vec4 vertexColor;
varying vec2 texCoord0;

void main() {
    gl_Position = ProjMat * ModelViewMat * vec4(Position, 1.0);
    vertexColor = Color;
    texCoord0 = UV0;
}
"#;

/// 1.17-1.19 `rendertype_entity_cutout.fsh`: `#version 150`, a single sampler, an alpha test
/// and an explicit fragment output - the shape of every entity render type in that era.
const ENTITY_CUTOUT_FSH_1_17: &str = r#"#version 150

uniform sampler2D Sampler0;
uniform vec4 ColorModulator;

in vec2 texCoord0;
in vec4 vertexColor;
in float vertexDistance;

out vec4 fragColor;

void main() {
    vec4 color = texture(Sampler0, texCoord0) * vertexColor * ColorModulator;
    if (color.a < 0.1) {
        discard;
    }
    fragColor = color;
}
"#;

#[test]
fn shader_pack_dialects_translate_and_keep_their_names() {
    let out = assert_ok(GBUFFERS_TERRAIN_VSH_IRIS);
    assert!(out.starts_with("#version 300 es"), "{out}");
    for kept in ["in vec3 mc_Entity;", "in vec4 at_tangent;", "in ivec2 UV2;",
                 "out vec4 texcoord;", "uniform mat4 projectionMatrix;"] {
        assert!(out.contains(kept), "{kept} lost:\n{out}");
    }

    let out = assert_ok(GBUFFERS_TERRAIN_FSH_IRIS);
    assert!(out.starts_with("#version 300 es"), "{out}");
    // A pack declares its own output: the gl_FragColor rewrite must not be forced on it.
    assert!(out.contains("out vec4 fragColor;"), "{out}");
    assert!(!out.contains("rust_FragColor"), "{out}");
    assert!(out.contains("discard;"), "{out}");
    assert!(out.contains("texture(gtexture, texcoord.st)"), "{out}");

    let out = assert_ok(COMPOSITE_FSH_IRIS);
    assert!(out.contains("gl_FragCoord.xy / vec2(viewWidth, viewHeight)"), "{out}");
    assert!(out.contains("texture(colortex0, coord)"), "{out}");
}

#[test]
fn sodium_chunk_shader_keeps_its_uniform_block() {
    let out = assert_ok(SODIUM_CHUNK_VSH_0_6);
    assert!(out.starts_with("#version 300 es"), "{out}");
    assert!(out.contains("layout(std140) uniform ChunkOffset"), "{out}");
    assert!(out.contains("vec4 u_ChunkOffset;"), "{out}");
    assert!(out.contains("u_ModelViewProjectionMatrix * vec4(position, 1.0)"), "{out}");
}

#[test]
fn legacy_attribute_and_varying_are_rewritten() {
    let out = assert_ok(LEGACY_VSH_1_12_2);
    assert!(out.starts_with("#version 300 es"), "{out}");
    assert!(out.contains("in vec3 Position;"), "{out}");
    assert!(out.contains("out vec4 vertexColor;"), "{out}");
    assert!(!out.contains("attribute "), "{out}");
    assert!(!out.contains("varying "), "{out}");

    let out = assert_ok(ENTITY_CUTOUT_FSH_1_17);
    assert!(out.starts_with("#version 300 es"), "{out}");
    assert!(out.contains("in vec2 texCoord0;"), "{out}");
    assert!(out.contains("out vec4 fragColor;"), "{out}");
    assert!(out.contains("discard;"), "{out}");
}
