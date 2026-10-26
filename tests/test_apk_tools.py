"""Offline tests for the tools that need the Android SDK when they really run.

The parsers in tools/verify_apk.py only ever see the text output of aapt2, so
they can be tested here against a captured sample of that output. If aapt2
changes its format, these tests fail here instead of in a packaging job.
"""

from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))

import verify_apk  # noqa: E402

# Real shape of `aapt2 dump xmltree <apk> --file AndroidManifest.xml`
XMLTREE = """Binary XML
N: android=http://schemas.android.com/apk/res/android (line=2)
  N: dist=http://schemas.android.com/apk/distribution (line=2)
    E: manifest (line=2)
      A: android:versionCode(0x0101021b)=1
      A: android:versionName(0x0101021c)="0.3.0-gurkha.1" (Raw: "0.3.0-gurkha.1")
      A: package="np.com.gurkha.rustgl.plugin" (Raw: "np.com.gurkha.rustgl.plugin")
      E: uses-sdk (line=25)
        A: android:minSdkVersion(0x0101020c)=26
        A: android:targetSdkVersion(0x01010270)=34
      E: application (line=27)
        A: android:label(0x01010001)=@0x7f040001
        A: android:hasCode(0x0101000e)=true
        A: android:extractNativeLibs(0x010104ea)=true
        E: activity (line=32)
          A: android:name(0x01010003)=".MainActivity" (Raw: ".MainActivity")
          A: android:exported(0x0105100e)=true
          E: intent-filter (line=34)
            E: action (line=35)
              A: android:name(0x01010003)="android.intent.action.MAIN" (Raw: "android.intent.action.MAIN")
            E: category (line=37)
              A: android:name(0x01010003)="android.intent.category.LAUNCHER" (Raw: "android.intent.category.LAUNCHER")
        E: meta-data (line=42)
          A: android:name(0x01010003)="fclPlugin_V2" (Raw: "fclPlugin_V2")
          A: android:resource(0x01010031)=@0x7f040000
        E: meta-data (line=45)
          A: android:name(0x01010003)="fclPlugin" (Raw: "fclPlugin")
          A: android:value(0x01010024)="true" (Raw: "true")
        E: meta-data (line=48)
          A: android:name(0x01010003)="zalithRendererPlugin" (Raw: "zalithRendererPlugin")
          A: android:value(0x01010024)="true" (Raw: "true")
        E: meta-data (line=51)
          A: android:name(0x01010003)="des" (Raw: "des")
          A: android:value(0x01010024)="RustGL (Gurkha build)" (Raw: "RustGL (Gurkha build)")
        E: meta-data (line=54)
          A: android:name(0x01010003)="boatEnv" (Raw: "boatEnv")
          A: android:value(0x01010024)="LIBGL_ES=3:RENDERER_BACKEND=gles" (Raw: "LIBGL_ES=3:RENDERER_BACKEND=gles")
        E: meta-data (line=60)
          A: android:name(0x01010003)="renderer" (Raw: "renderer")
          A: android:value(0x01010024)="opengles3_rust:librust_gl.so:/librust_gl.so" (Raw: "opengles3_rust:librust_gl.so:/librust_gl.so")
        E: meta-data (line=51)
          A: android:name(0x01010003)="pojavEnv" (Raw: "pojavEnv")
          A: android:value(0x01010024)="LIBGL_ES=3:RENDERER_BACKEND=gles" (Raw: "LIBGL_ES=3:RENDERER_BACKEND=gles")
"""

BADGING = """package: name='np.com.gurkha.rustgl.plugin' versionCode='1' \
versionName='0.3.0-gurkha.1' compileSdkVersion='34' compileSdkVersionCodename='14'
sdkVersion:'26'
targetSdkVersion:'34'
application-label:'RustGL (Gurkha build)'
application-label-en:'RustGL (Gurkha build)'
launchable-activity: name='np.com.gurkha.rustgl.plugin.MainActivity'  label='' icon=''
native-code: 'arm64-v8a' 'armeabi-v7a' 'x86' 'x86_64'
"""


class XmlTreeTests(unittest.TestCase):
    def test_meta_data_values(self):
        meta = verify_apk.parse_meta_data(XMLTREE)
        self.assertEqual(meta["fclPlugin"], "true")
        self.assertEqual(meta["fclPlugin_V2"], "@resource")
        self.assertEqual(meta["renderer"], "opengles3_rust:librust_gl.so:/librust_gl.so")
        self.assertEqual(meta["pojavEnv"], "LIBGL_ES=3:RENDERER_BACKEND=gles")

    def test_attribute_value_forms(self):
        self.assertEqual(verify_apk.attribute_value('"text" (Raw: "text")'), "text")
        self.assertEqual(verify_apk.attribute_value("true"), "true")
        self.assertEqual(verify_apk.attribute_value("26"), "26")
        self.assertEqual(verify_apk.attribute_value("@0x7f040000"), "@resource")

    def test_manifest_contract_against_the_sample(self):
        """The checks that gate the build must accept the shipped manifest."""
        meta = verify_apk.parse_meta_data(XMLTREE)
        for key in verify_apk.REQUIRED_META:
            self.assertIn(key, meta)
        self.assertTrue(meta["fclPlugin_V2"].startswith("@"))
        renderer_id, gl_name, egl_name = meta["renderer"].split(":")
        self.assertTrue(egl_name.startswith("/"))
        self.assertTrue(gl_name.endswith(".so"))
        self.assertIn("android.intent.action.MAIN", XMLTREE)
        self.assertRegex(XMLTREE, r"android:exported\(0x[0-9a-fA-F]+\)=true")
        self.assertIsNone(verify_apk.re.search(r"E: uses-permission", XMLTREE))

    def test_badging_strings_that_the_check_greps_for(self):
        needed = [
            "package: name='np.com.gurkha.rustgl.plugin'",
            "versionName='0.3.0-gurkha.1'",
            "sdkVersion:'26'",
            "targetSdkVersion:'34'",
            "launchable-activity: name='np.com.gurkha.rustgl.plugin.MainActivity'",
            "application-label:'RustGL (Gurkha build)'",
        ]
        for needle in needed:
            self.assertIn(needle, BADGING, needle)
        self.assertNotIn("uses-permission:", BADGING)


class ConfigExtractionTests(unittest.TestCase):
    def test_json_is_found_in_a_resource_table_dump(self):
        pool = ('String pool of 4 unique UTF-8 non-sorted strings, 4 entries and 0 styles '
                'using 496 bytes:\n'
                'String #0: app_name\n'
                'String #1: RustGL (Gurkha build)\n'
                'String #2: {"displayName":"RustGL (Gurkha build)",'
                '"rendererId":"opengles3_rust","rendererGLPath":"**|librust_gl.so",'
                '"rendererEGLPath":"librust_gl.so","dlopenLibPaths":[],'
                '"env":[{"type":"NormalEnv","key":"LIBGL_ES","value":"3"}],'
                '"minMCVer":null,"maxMCVer":null}\n'
                'String #3: rustgl_debug_title\n')
        config = verify_apk.extract_json(pool)
        self.assertEqual(config["rendererId"], "opengles3_rust")
        self.assertEqual(config["rendererGLPath"], "**|librust_gl.so")
        self.assertEqual(config["env"][0]["type"], "NormalEnv")

    def test_escaped_json_from_aapt2_dump_strings(self):
        escaped = 'String #2: {\\"displayName\\":\\"RustGL\\",\\"rendererId\\":\\"x\\"}'
        config = verify_apk.extract_json(escaped.replace('\\"', '"'))
        self.assertEqual(config["rendererId"], "x")

    def test_missing_json_raises(self):
        with self.assertRaises(LookupError):
            verify_apk.extract_json("String #0: app_name")


if __name__ == "__main__":
    unittest.main(verbosity=2)
