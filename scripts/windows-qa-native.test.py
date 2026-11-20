"""Local tests for native notice and NSIS list checks. No program is executed."""
import hashlib
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch
import zipfile

mod = runpy.run_path(str(Path(__file__).with_name("windows-qa-native.py")))
list_nsis = mod["list_nsis"]
get_sdk_license = mod["get_sdk_license"]
verify_bitmap = mod["verify_bitmap"]
SDK = mod["SDK"]
NSIS_LIST = """Type = Nsis
Method = Deflate
----------
Path = $PLUGINSDIR\\nsis_tauri_utils.dll
Path = $PLUGINSDIR\\System.dll
Path = $PLUGINSDIR\\nsDialogs.dll
Path = $PLUGINSDIR\\NSISdl.dll
Path = $INSTDIR\\rosenfu-desktop.exe
Path = $INSTDIR\\uninstall.exe
"""


class NativeQa(unittest.TestCase):
    def test_deflate_and_plugins_windows_newlines(self):
        app, plugins, extras = list_nsis(NSIS_LIST.replace("\n","\r\n"))
        self.assertEqual(app, "$INSTDIR\\rosenfu-desktop.exe")
        self.assertEqual(len(plugins), 4)
        self.assertEqual(extras, {})

    def test_rejects_unreviewed_codec_and_components(self):
        for text, error in (
            (NSIS_LIST.replace("Deflate", "LZMA:23"), "Deflate"),
            (NSIS_LIST + "Path = vcruntime140.dll\n", "native DLL"),
            (NSIS_LIST + "Path = $PLUGINSDIR\\System.DLL\n", "Case-colliding"),
            (NSIS_LIST + "Path = nested\\$PLUGINSDIR\\System.dll\n", "native DLL"),
            (NSIS_LIST + "Path = nested\\$INSTDIR\\rosenfu-desktop.exe\n", "native EXE"),
            (NSIS_LIST + "Path = nested\\uninstall.exe\n", "native EXE"),
            (NSIS_LIST + "Path = uninstall.exe\n", "Duplicate uninstaller"),
            (NSIS_LIST + "Path = unreviewed.exe.\n", "Unsafe"),
            (NSIS_LIST + "Path = data\\fake-license.txt\n", "Unreviewed installer member"),
            (NSIS_LIST + "Path = $PLUGINSDIR\\unknown.bmp\n", "Unreviewed installer member"),
            (NSIS_LIST + "Path = $PLUGINSDIR\\..\\rogue.exe\n", "Unsafe"),
            (NSIS_LIST.replace("Path = $PLUGINSDIR\\NSISdl.dll\n", ""), "required plugin"),
        ):
            with self.subTest(error=error), self.assertRaisesRegex(ValueError, error):
                list_nsis(text)

    def test_known_bitmap_requires_actual_bmp_data(self):
        _, _, extras = list_nsis(NSIS_LIST + "Path = $PLUGINSDIR\\modern-wizard.bmp\n")
        self.assertIn("$pluginsdir/modern-wizard.bmp", extras)
        bmp = bytearray(64)
        bmp[:2] = b"BM"
        bmp[2:6] = (64).to_bytes(4, "little")
        bmp[10:14] = (54).to_bytes(4, "little")
        verify_bitmap(bmp)
        bmp[:2] = b"MZ"
        with self.assertRaisesRegex(ValueError, "Non-BMP"):
            verify_bitmap(bmp)

    def test_sdk_notice_integrity_identity_and_pinned_loader_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            cargo_loader = directory / "WebView2LoaderStatic.lib"
            cargo_loader.write_bytes(b"test loader")
            sdk = directory / "WebView2.nupkg"
            expected = hashlib.sha256(cargo_loader.read_bytes()).hexdigest()

            notice = "Fixture: Microsoft Corporation Redistribution binary form disclaimer"
            notice_hash = hashlib.sha256(notice.encode()).hexdigest()

            def sdk_bytes(version=SDK, loader=b"test loader", notice_override=notice):
                with zipfile.ZipFile(sdk, "w") as archive:
                    archive.writestr("Microsoft.Web.WebView2.nuspec", f'''<package xmlns="https://schemas.microsoft.com/packaging/2013/05/nuspec.xsd"><metadata><id>Microsoft.Web.WebView2</id><version>{version}</version><license type="file">LICENSE.txt</license></metadata></package>''')
                    archive.writestr("build/native/x64/WebView2LoaderStatic.lib", loader)
                    archive.writestr("LICENSE.txt", notice_override)

            with patch.dict(get_sdk_license.__globals__, {"LOADER_SHA256": expected, "SDK_LICENSE_TEXT_SHA256": notice_hash}):
                sdk_bytes()
                self.assertTrue(get_sdk_license(sdk, cargo_loader).startswith(b"Fixture:"))
                sdk_bytes(notice_override=notice + " substituted terms")
                with self.assertRaisesRegex(ValueError, "official license"):
                    get_sdk_license(sdk, cargo_loader)
                sdk_bytes(loader=b"wrong loader")
                with self.assertRaisesRegex(ValueError, "bytes differ"):
                    get_sdk_license(sdk, cargo_loader)
                sdk_bytes(version="0.0.0")
                with self.assertRaisesRegex(ValueError, "identity"):
                    get_sdk_license(sdk, cargo_loader)


if __name__ == "__main__":
    unittest.main()
