"""Verify native components and stage corresponding notices (no installer execution)."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import threading
import xml.etree.ElementTree as ET
import zipfile

SDK = "1.0.3800.47"
SDK_URL = f"https://api.nuget.org/v3-flatcontainer/microsoft.web.webview2/{SDK}/microsoft.web.webview2.{SDK}.nupkg"
# NuGet version-specific official license display, UTF-8 after collapsing whitespace.
SDK_LICENSE_TEXT_SHA256 = "009bc47dc0881d95f92e240040400d3d32c4b291e3097dc4063c8c3df1efe279"
LOADER_SHA256 = "89c6b872783b8f6c3cedbff618adb42082d455c615453ae10cfc753f1e8f25d8"
UTIL_SHA1 = "75197fee3c6a814fe035788d1c34ead39349b860"
NSIS_ARCHIVE_SHA1 = "db14f8af2ea346b786c6ae19343f61aaf5e09b8e"
ALLOWED_PLUGINS = {"nsisdl.dll", "startmenu.dll", "system.dll", "nsdialogs.dll", "nsis_tauri_utils.dll"}
REQUIRED_PLUGINS = ALLOWED_PLUGINS - {"startmenu.dll"}
RESOURCE_LIMITS = {
    "$pluginsdir/modern-wizard.bmp": 1_048_576,
    "$pluginsdir/modern-header.bmp": 1_048_576,
    # Some 7-Zip builds synthesize a textual listing member instead of hiding it.
    "[nsis].nsi": 1_048_576,
}
NOTICES = {
    "NSIS-3.13-COPYING.txt": "6cc665347be5c84b76bf0fd6f85fbb0c33261e89b3408aa3e3e55dbe95e12d89",
    "NSIS-Tauri-Utils-0.5.3-APACHE.txt": "0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594",
    "NSIS-Tauri-Utils-0.5.3-MIT.txt": "1c1020fa10a6bf318717e82c911bcc54ebdfb9bb280460ae332bcb2f82f57fbe",
    "Tauri-CLI-2.12.4-APACHE.txt": "0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594",
    "Tauri-CLI-2.12.4-MIT.txt": "9dd42ea92cff2ede5cd477cbfcce051b2d0115c0ac7f368ee88cb545055dff1d",
}


def sha(path, name="sha256"):
    with path.open("rb") as f:
        return hashlib.file_digest(f, name).hexdigest()


def list_nsis(text):
    header, sep, payload = text.replace("\r\n", "\n").partition("\n----------\n")
    if not sep or not re.search(r"^Type = Nsis$", header, re.M | re.I) or not re.search(
        r"^Method = Deflate(?::[^\n]*)?$", header, re.M
    ):
        raise ValueError("Expected an inspectable Deflate/NSIS archive")
    paths = re.findall(r"^Path = (.+)$", payload, re.M)
    if not paths:
        raise ValueError("NSIS payload list empty")
    names = {}
    for original in paths:
        plain = original.replace("\\", "/")
        path = PurePosixPath(plain)
        if path.is_absolute() or any(
            not re.fullmatch(r"[A-Za-z0-9$_. ()\[\]-]+", part)
            or part[-1:] in (" ", ".")
            or part[:1] == " "
            or part.split(".", 1)[0].casefold() in {"con", "prn", "aux", "nul", *(f"{x}{i}" for x in ("com", "lpt") for i in range(1,10))}
            for part in plain.split("/")
        ):
            raise ValueError(f"Unsafe NSIS member: {original}")
        key = str(path).casefold()
        if key in names:
            raise ValueError(f"Case-colliding NSIS member: {original}")
        names[key] = original
    plugins = {}
    mains = []
    uninstallers = []
    resources = {}
    for name, original in names.items():
        filename = PurePosixPath(name).name
        if filename.endswith(".dll"):
            if str(PurePosixPath(name).parent) != "$pluginsdir" or filename not in ALLOWED_PLUGINS:
                raise ValueError(f"Unreviewed native DLL: {original}")
            if filename in plugins:
                raise ValueError(f"Duplicate plugin: {original}")
            plugins[filename] = original
        elif filename.endswith(".exe"):
            if str(PurePosixPath(name).parent) not in {".", "$instdir"}:
                raise ValueError(f"Unreviewed native EXE: {original}")
            if filename == "uninstall.exe":
                uninstallers.append(original)
            elif filename in {"rosenfu-desktop.exe", "rosenfu alpha 0.2 test.exe"}:
                mains.append(original)
            else:
                raise ValueError(f"Unreviewed native EXE: {original}")
        elif name in RESOURCE_LIMITS:
            resources[name] = original
        else:
            raise ValueError(f"Unreviewed installer member: {original}")
    if len(uninstallers) > 1:
        raise ValueError("Duplicate uninstaller in native payload")
    if len(mains) != 1 or REQUIRED_PLUGINS - plugins.keys():
        raise ValueError("NSIS app or required plugin absent")
    return mains[0], plugins, resources


def get_sdk_license(package, cargo_loader):
    if package.stat().st_size > 25 * 1024 * 1024:
        raise ValueError("Unexpected SDK package size")
    if sha(cargo_loader) != LOADER_SHA256:
        raise ValueError("Pinned Cargo WebView2 SDK loader has changed")
    with zipfile.ZipFile(package) as archive:
        if len(archive.infolist()) > 2_000:
            raise ValueError("SDK package unexpectedly large")
        names = {name.casefold(): name for name in archive.namelist()}
        if len(names) != len(archive.namelist()):
            raise ValueError("Case-colliding SDK members")
        spec, = [name for name in names if name.endswith(".nuspec") and "/" not in name]
        if archive.getinfo(names[spec]).file_size > 131_072:
            raise ValueError("SDK nuspec unexpectedly large")
        root = ET.fromstring(archive.read(names[spec]))
        namespace = root.tag.partition("}")[0] + "}" if root.tag.startswith("{") else ""
        prop = lambda name: root.find(f"{namespace}metadata/{namespace}{name}")
        if prop("id").text != "Microsoft.Web.WebView2" or prop("version").text != SDK:
            raise ValueError("Wrong Microsoft SDK package identity")
        license_node = prop("license")
        if license_node is None or license_node.get("type") != "file":
            raise ValueError("Missing package-bundled license")
        license_name = PurePosixPath(license_node.text.strip())
        if len(license_name.parts) != 1 or not re.fullmatch(r"(?i)licen[cs]e(?:\.[a-z0-9]+)?", str(license_name)):
            raise ValueError("Unexpected SDK license path")
        loader, = [name for name in names if re.search(r"(?:^|/)x64/webview2loaderstatic\.lib$", name)]
        loader_info = archive.getinfo(names[loader])
        license_info = archive.getinfo(names[str(license_name).casefold()])
        if loader_info.file_size > 5 * 1024 * 1024 or license_info.file_size > 64 * 1024:
            raise ValueError("Unexpected SDK member size")
        if hashlib.sha256(archive.read(loader_info)).hexdigest() != LOADER_SHA256:
            raise ValueError("Microsoft SDK and linked Cargo loader bytes differ")
        notice = archive.read(license_info)
        normalized = " ".join(notice.decode("utf-8-sig").split()).encode()
        if hashlib.sha256(normalized).hexdigest() != SDK_LICENSE_TEXT_SHA256:
            raise ValueError("SDK license text differs from the pinned official license")
        return notice


def capture_listing(seven_zip, args):
    result = subprocess.run([str(seven_zip), *map(str,args)], check=True, capture_output=True, timeout=60)
    return result.stdout.decode("utf-8", "strict")


def capture_member(seven_zip, installer, name, maximum):
    """Read bounded bytes from exact 7-Zip member output, never run or extract to a path."""
    args = [str(seven_zip), "x", "-so", "-spd", "-bd", "-bse2", "-bsp0", "--", str(installer), name]
    with subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL) as process:
        watchdog = threading.Timer(45, process.kill)
        watchdog.daemon = True
        watchdog.start()
        try:
            data = process.stdout.read(maximum + 1)
            if len(data) > maximum:
                process.kill()
                raise ValueError(f"Installer member exceeds byte bound: {name}")
            if process.wait(timeout=5) != 0:
                raise ValueError(f"Could not inspect installer member: {name}")
            return data
        finally:
            watchdog.cancel()
            if process.poll() is None:
                process.kill()
                process.wait()


def verify_pe(content, name):
    offset = int.from_bytes(content[60:64], "little")
    if not content.startswith(b"MZ") or not 64 <= offset <= len(content) - 4 or content[offset:offset + 4] != b"PE\x00\x00":
        raise ValueError(f"Embedded plugin is not a PE image: {name}")


def verify_bitmap(content):
    # Real small BMP data only; reject PE/different content under an allowed filename.
    if len(content) < 54 or not content.startswith(b"BM") or int.from_bytes(content[2:6], "little") != len(content):
        raise ValueError("Non-BMP data under an NSIS UI resource name")
    offset = int.from_bytes(content[10:14], "little")
    if not 26 <= offset < len(content):
        raise ValueError("Invalid NSIS UI bitmap offset")


def build(options):
    if options.output.exists():
        raise ValueError("Native notice destination must be new")
    lock = json.loads((options.repo / "package-lock.json").read_text(encoding="utf-8"))
    qa = json.loads((options.repo / "src-tauri/tauri.alpha02-qa.conf.json").read_text(encoding="utf-8"))
    if lock["packages"]["node_modules/@tauri-apps/cli"]["version"] != "2.12.4" or qa["bundle"]["windows"]["nsis"]["compression"] != "zlib":
        raise ValueError("Native notice pins require reviewed Tauri CLI 2.12.4 and NSIS zlib")
    if qa["bundle"]["windows"]["webviewInstallMode"]["type"] != "downloadBootstrapper":
        raise ValueError("Only Microsoft runtime download mode is reviewed")
    listing = capture_listing(options.seven_zip, ["l","-slt","-sccUTF-8","--", options.installer])
    main, plugins, resources = list_nsis(listing)
    plugin_hashes = {}
    for filename, path in plugins.items():
        blob = capture_member(options.seven_zip, options.installer, path, 4 * 1024 * 1024)
        verify_pe(blob, filename)
        if filename == "nsis_tauri_utils.dll" and hashlib.sha1(blob).hexdigest() != UTIL_SHA1:
            raise ValueError("Embedded NSIS Tauri Utils DLL does not match the pinned version")
        plugin_hashes[filename] = hashlib.sha256(blob).hexdigest()
    resource_hashes = {}
    for name, path in resources.items():
        blob = capture_member(options.seven_zip, options.installer, path, RESOURCE_LIMITS[name])
        if name == "[nsis].nsi":
            if not blob or blob.startswith(b"MZ") or b"\x00" in blob:
                raise ValueError("Non-text bytes in the NSIS script listing")
            blob.decode("utf-8-sig", "strict")
        else:
            verify_bitmap(blob)
        resource_hashes[name] = hashlib.sha256(blob).hexdigest()
    loader = get_sdk_license(options.sdk_package, options.cargo_loader)
    options.output.mkdir()
    for name, digest in NOTICES.items():
        path = options.repo / "scripts/windows-qa-native" / name
        if sha(path) != digest:
            raise ValueError(f"Native notice source changed: {name}")
        shutil.copyfile(path, options.output / name)
    (options.output / f"Microsoft-WebView2-SDK-{SDK}-LICENSE.txt").write_bytes(loader)
    (options.output / "NATIVE-PROVENANCE.json").write_text(json.dumps({
        "tauri_cli":"2.12.4", "nsis":"3.13", "nsis_expected_input_archive_sha1":NSIS_ARCHIVE_SHA1,
        "nsis_compression":"Deflate/zlib", "installer_sha256":sha(options.installer), "main":main,
        "observed_plugin_sha256":plugin_hashes, "observed_resource_sha256":resource_hashes, "nsis_tauri_utils":"0.5.3", "nsis_tauri_utils_sha1":UTIL_SHA1,
        "nsis_tauri_utils_dependencies":"Pinned tag has semver 1.x and windows-sys 0.61.2; binary release lock not available; review exact linked licenses separately.",
        "webview2_runtime":"Download on demand from Microsoft; neither bootstrapper nor Runtime is bundled.",
        "webview2_loader_sdk":SDK, "webview2_loader_sha256":LOADER_SHA256,
        "sdk_nuget_url":SDK_URL, "sdk_nuget_sha256":sha(options.sdk_package),
        "sdk_license_normalized_sha256":SDK_LICENSE_TEXT_SHA256,
        "license_sources":{
            "nsis":"https://github.com/NSIS-Dev/nsis/blob/a4ba28e60bd9e7c271cd2bfc1558889ce3355653/COPYING",
            "nsis_tauri_utils":"https://github.com/tauri-apps/nsis-tauri-utils/tree/13d9edd27b69310e108d6fbd49f90992f8a05390",
            "tauri":"https://github.com/tauri-apps/tauri/tree/tauri-cli-v2.12.4",
            "sdk":f"https://www.nuget.org/packages/Microsoft.Web.WebView2/{SDK}/License",
        }
    }, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("repo", "installer", "seven-zip", "sdk-package", "cargo-loader", "output"):
        parser.add_argument(f"--{name}",type=Path,required=True)
    build(parser.parse_args())
