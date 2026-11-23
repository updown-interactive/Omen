#!/usr/bin/env python3
"""Build local Omen platform artifacts. Signing is explicit; no upload occurs by default."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = ROOT
TARGET_DIR = ROOT / 'target'
CONFIG_PATH = None
CURRENT_CONFIG = {}

def run(arguments, env=None):
    arguments = [str(value) for value in arguments]
    if arguments[0] == 'cargo' and not (PROJECT_ROOT / 'Cargo.lock').exists():
        arguments = [value for value in arguments if value != '--locked']
    try:
        subprocess.run(arguments, cwd=PROJECT_ROOT, env=env, check=True)
    except subprocess.CalledProcessError:
        if arguments[0] == 'cargo' and '--locked' in arguments:
            unlocked = [value for value in arguments if value != '--locked']
            subprocess.run(unlocked, cwd=PROJECT_ROOT, env=env, check=True)
        else:
            raise

def configuration():
    config = json.loads((CONFIG_PATH or PROJECT_ROOT / 'omen.app.json').read_text())
    if not re.fullmatch(r'[A-Za-z0-9_-]+', config['package']):
        raise ValueError('Invalid Cargo package name')
    if not re.fullmatch(r'[A-Za-z0-9.-]+', config['identifier']):
        raise ValueError('Invalid application identifier')
    if not re.fullmatch(r'[A-Za-z0-9_-]+', config.get('native_package', config['package'])):
        raise ValueError('Invalid native Cargo package name')
    targets = config.get('targets', ['desktop', 'android', 'ios', 'web'])
    if not isinstance(targets, list) or not targets or any(target not in ['desktop', 'android', 'ios', 'web'] for target in targets) or len(set(targets)) != len(targets):
        raise ValueError('targets must be a nonempty list of unique Omen platform names')
    return config

def cargo_env():
    env = os.environ.copy()
    env['CARGO_TARGET_DIR'] = str(TARGET_DIR)
    window = CURRENT_CONFIG.get('window', {})
    env['OMEN_WINDOW_TITLE'] = window.get('title', CURRENT_CONFIG.get('name', 'Omen'))
    env['OMEN_WINDOW_WIDTH'] = str(window.get('width', 800))
    env['OMEN_WINDOW_HEIGHT'] = str(window.get('height', 520))
    env['OMEN_APP_ID'] = CURRENT_CONFIG.get('identifier', 'dev.omen.app')
    env['OMEN_APP_NAME'] = CURRENT_CONFIG.get('name', 'Omen')
    if sys.platform == 'darwin':
        # Some rustup distributions give rust-lld an incomplete libLLVM rpath.
        sysroot = subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip()
        env['DYLD_FALLBACK_LIBRARY_PATH'] = str(Path(sysroot) / 'lib') + ':' + env.get('DYLD_FALLBACK_LIBRARY_PATH', '')
    return env

def copy_assets(destination):
    source = PROJECT_ROOT / 'assets'
    if destination.resolve().is_relative_to(source.resolve()):
        raise ValueError('Package output cannot be inside the source assets directory')
    if destination.exists(): shutil.rmtree(destination)
    if source.is_dir(): shutil.copytree(source, destination)

def copy_icon(source, destination):
    if source is None:
        destination.unlink(missing_ok=True)
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

def embed_windows_icon(executable, icon):
    import ctypes
    from ctypes import wintypes
    import struct

    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    begin = kernel.BeginUpdateResourceW
    begin.argtypes = [wintypes.LPCWSTR, wintypes.BOOL]
    begin.restype = wintypes.HANDLE
    update = kernel.UpdateResourceW
    update.argtypes = [wintypes.HANDLE, ctypes.c_void_p, ctypes.c_void_p,
                       wintypes.WORD, ctypes.c_void_p, wintypes.DWORD]
    update.restype = wintypes.BOOL
    end = kernel.EndUpdateResourceW
    end.argtypes = [wintypes.HANDLE, wintypes.BOOL]
    end.restype = wintypes.BOOL

    data = icon.read_bytes()
    count, = struct.unpack_from('<H', data, 4)
    group = bytearray(data[:6])
    handle = begin(str(executable), False)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())

    def write_resource(kind, identifier, payload):
        buffer = ctypes.create_string_buffer(payload)
        if not update(handle, kind, identifier, 0, buffer, len(payload)):
            raise ctypes.WinError(ctypes.get_last_error())

    try:
        for index in range(count):
            entry = 6 + index * 16
            length, offset = struct.unpack_from('<II', data, entry + 8)
            identifier = index + 1
            write_resource(3, identifier, data[offset:offset + length]) # RT_ICON
            group.extend(data[entry:entry + 12])
            group.extend(struct.pack('<H', identifier))
        write_resource(14, 1, bytes(group)) # RT_GROUP_ICON
    except BaseException:
        end(handle, True)
        raise
    if not end(handle, False):
        raise ctypes.WinError(ctypes.get_last_error())

def compile_ios_icon(icons, bundle, device):
    with tempfile.TemporaryDirectory() as directory:
        directory = Path(directory)
        partial_info = directory / 'icon.plist'
        resources = directory / 'resources'
        resources.mkdir()
        platform = 'iphoneos' if device else 'iphonesimulator'
        run(['xcrun', '--sdk', platform, 'actool',
             '--compile', resources, '--output-partial-info-plist', partial_info,
             '--platform', platform, '--minimum-deployment-target', '15.0',
             '--target-device', 'iphone', '--target-device', 'ipad',
             '--app-icon', 'AppIcon', icons / 'Icons.xcassets'])
        for resource in resources.iterdir():
            shutil.copy2(resource, bundle / resource.name)
        return plistlib.loads(partial_info.read_bytes())

def web(config, out, icons=None):
    run(['cargo', 'build', '-p', config['package'], '--bin', config['package'], '--release', '--target', 'wasm32-unknown-unknown', '--no-default-features', '--features', 'web', '--locked'], cargo_env())
    out.mkdir(parents=True, exist_ok=True)
    shutil.copytree(ROOT / 'platforms/web/ui', out / 'ui', dirs_exist_ok=True)
    shutil.copy2(ROOT / 'platforms/web/host.mjs', out / 'host.mjs')
    binary = TARGET_DIR / 'wasm32-unknown-unknown/release' / (config['package'] + '.wasm')
    shutil.copy2(binary, out / 'app.wasm')
    (out / 'app.json').write_text(json.dumps(config, indent=2))
    page = '''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; media-src blob:; object-src 'none'; frame-src 'none'; base-uri 'none'">
<title>Omen</title><link rel="stylesheet" href="./ui/omen.css"></head><body><main id="omen-root" aria-label="Omen application"></main><pre id="omen-error" hidden role="alert"></pre><script type="module" src="./boot.mjs"></script></body></html>'''
    if icons:
        page = page.replace('<title>Omen</title>',
                            '<title>Omen</title><link rel="icon" href="./favicon.png">'
                            '<link rel="apple-touch-icon" href="./apple-touch-icon.png">')
    (out / 'index.html').write_text(page)
    for name in ['favicon.png', 'apple-touch-icon.png']:
        copy_icon(icons / name if icons else None, out / name)
    shutil.copy2(ROOT / 'platforms/web/ui/boot.mjs', out / 'boot.mjs')
    copy_assets(out / 'assets')
    files = sorted(path for path in out.rglob('*') if path.is_file() and path.name not in ['manifest.json', 'omen-artifact.json'])
    manifest = {str(path.relative_to(out)): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2))
    artifact(out, 'web', out, config)
    print(f'Built web application: {out}')

def android(config, out, release, icons=None):
    if release:
        for key in ['OMEN_ANDROID_KEYSTORE', 'OMEN_ANDROID_STORE_PASSWORD', 'OMEN_ANDROID_KEY_ALIAS', 'OMEN_ANDROID_KEY_PASSWORD']:
            if not os.environ.get(key):
                raise ValueError(f'Release signing requires {key}')
    sdk = Path(os.environ.get('ANDROID_HOME', os.environ.get('ANDROID_SDK_ROOT', '')))
    ndk = Path(os.environ.get('ANDROID_NDK_HOME', str(sdk / 'ndk/28.2.13676358')))
    host = 'darwin-x86_64' if sys.platform == 'darwin' else 'windows-x86_64' if sys.platform == 'win32' else 'linux-x86_64'
    compiler = ndk / 'toolchains/llvm/prebuilt' / host / ('bin/aarch64-linux-android26-clang.cmd' if sys.platform == 'win32' else 'bin/aarch64-linux-android26-clang')
    if not compiler.is_file():
        raise ValueError('Set ANDROID_HOME/ANDROID_NDK_HOME to an installed NDK')
    env = cargo_env(); env['CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER'] = str(compiler)
    env['CC_aarch64_linux_android'] = str(compiler)
    run(['cargo', 'build', '-p', config.get('native_package', config['package']), '--lib', '--target', 'aarch64-linux-android', '--profile', ('release' if release else 'mobile'), '--no-default-features', '--features', 'native', '--locked'], env)
    android_root = PROJECT_ROOT / '.omen/android'
    shutil.copytree(ROOT / 'platforms/android-app', android_root, dirs_exist_ok=True,
                    ignore=shutil.ignore_patterns('build', '.gradle', '.cxx', 'jniLibs', 'assets', 'local.properties'))
    gradle_config = android_root / 'build.gradle.kts'
    settings = gradle_config.read_text()
    settings = re.sub(r'applicationId = "[^"]+"', 'applicationId = ' + json.dumps(config['identifier']), settings)
    settings = re.sub(r'versionName = "[^"]+"', 'versionName = ' + json.dumps(config['version']), settings)
    gradle_config.write_text(settings)
    import xml.etree.ElementTree as ET
    ns = 'http://schemas.android.com/apk/res/android'; ET.register_namespace('android', ns)
    manifest = android_root / 'src/main/AndroidManifest.xml'; document = ET.parse(manifest)
    application = document.getroot().find('application')
    application.set('{'+ns+'}label', config['name'])
    if icons:
        application.set('{'+ns+'}icon', '@mipmap/omen_app_icon')
    for density in ['mdpi', 'hdpi', 'xhdpi', 'xxhdpi', 'xxxhdpi']:
        icon_path = Path(f'mipmap-{density}/omen_app_icon.png')
        copy_icon(icons / icon_path if icons else None, android_root / 'src/main/res' / icon_path)
    document.write(manifest, encoding='utf-8', xml_declaration=True)
    libraries = android_root / 'src/main/jniLibs/arm64-v8a'; libraries.mkdir(parents=True, exist_ok=True)
    shutil.copy2(TARGET_DIR / 'aarch64-linux-android' / ('release' if release else 'mobile') / 'libomen_app.so', libraries / 'libomen_app.so')
    # The superseded WebView bundle must not ship in a native application.
    assets = android_root / 'src/main/assets/omen'
    if assets.exists(): shutil.rmtree(assets)
    copy_assets(android_root / 'src/main/assets')
    gradle = os.environ.get('GRADLE', 'gradle')
    run([gradle, '--no-daemon', '-p', android_root, ':assembleRelease' if release else ':assembleDebug'])
    mode = 'release' if release else 'debug'
    source = android_root / f'build/outputs/apk/{mode}/android-app-{mode}.apk'
    if not source.is_file():
        source = android_root / f'build/outputs/apk/{mode}/app-{mode}.apk'
    out.mkdir(parents=True, exist_ok=True); shutil.copy2(source, out / 'Omen.apk')
    artifact(out, 'android', out / 'Omen.apk', config, activity='dev.omen.app.OmenActivity')
    print(f'Built native GPUI Android ARM64 application: {out / "Omen.apk"}')

def desktop(config, out, sign, release=False, icons=None):
    run(['cargo', 'build', '-p', config['package'], '--bin', config['package'], '--no-default-features', '--features', 'native', '--locked'] + (['--release'] if release else []), cargo_env())
    out.mkdir(parents=True, exist_ok=True)
    binary = TARGET_DIR / ('release' if release else 'debug') / (config['package'] + ('.exe' if sys.platform == 'win32' else ''))
    if sys.platform == 'darwin':
        description = subprocess.check_output(['file', '-b', binary], text=True).strip()
        if not os.access(binary, os.X_OK) or 'Mach-O' not in description or 'executable' not in description:
            raise ValueError(f'Cargo produced an invalid macOS executable: {binary} ({description}). '
                             'Check available disk space and the linker or stripping output before rebuilding.')
        bundle = out / 'Omen.app'; contents = bundle / 'Contents'; (contents / 'MacOS').mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, contents / 'MacOS/Omen')
        info = {'CFBundleExecutable': 'Omen', 'CFBundleIdentifier': config['identifier'], 'CFBundleName': config['name'], 'CFBundleShortVersionString': config['version'], 'CFBundleVersion': '1', 'CFBundlePackageType': 'APPL', 'NSHighResolutionCapable': True}
        if icons:
            info['CFBundleIconFile'] = 'AppIcon.icns'
        copy_icon(icons / 'AppIcon.icns' if icons else None, contents / 'Resources/AppIcon.icns')
        (contents / 'Info.plist').write_bytes(plistlib.dumps(info))
        copy_assets(contents / 'Resources/assets')
        identity = os.environ.get('OMEN_MAC_SIGN_IDENTITY') if sign else '-'
        if not identity:
            raise ValueError('Signing requires OMEN_MAC_SIGN_IDENTITY')
        run(['codesign', '--force', '--sign', identity, bundle])
        run(['codesign', '--verify', '--strict', bundle])
    elif sys.platform == 'win32':
        destination = out / 'Omen.exe'; shutil.copy2(binary, destination)
        if icons:
            embed_windows_icon(destination, icons / 'AppIcon.ico')
        copy_assets(out / 'assets')
        if sign:
            thumbprint = os.environ.get('OMEN_WINDOWS_CERT_THUMBPRINT')
            if not thumbprint:
                raise ValueError('Signing requires OMEN_WINDOWS_CERT_THUMBPRINT in the certificate store')
            run(['signtool', 'sign', '/sha1', thumbprint, '/fd', 'SHA256', destination])
            run(['signtool', 'verify', '/pa', destination])
    else:
        if sign: raise ValueError('Linux signing is not configured; sign the generated archive separately')
        staging = out / 'linux'; (staging / 'bin').mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, staging / 'bin' / config['package'])
        icon_name = config['identifier']
        entry = f'[Desktop Entry]\nType=Application\nName={config["name"]}\nExec={config["package"]}\nTerminal=false\nCategories=Development;\n'
        if icons:
            entry += f'Icon={icon_name}\n'
        copy_icon(icons / 'app-icon.png' if icons else None,
                  staging / 'share/icons/hicolor/256x256/apps' / f'{icon_name}.png')
        (staging / 'Omen.desktop').write_text(entry)
        copy_assets(staging / 'assets')
        with tarfile.open(out / 'Omen-linux.tar.gz', 'w:gz') as archive:
            archive.add(staging, arcname='Omen')
    executable = (out / 'Omen.app/Contents/MacOS/Omen') if sys.platform == 'darwin' else (out / 'Omen.exe') if sys.platform == 'win32' else (out / 'linux/bin' / config['package'])
    artifact(out, 'desktop', executable, config)
    print(f'Built desktop artifact: {out}')

def ios(config, out, device, sign, release=False, icons=None):
    if sys.platform != 'darwin':
        raise ValueError('iOS builds require macOS and Xcode')
    if device:
        for key in ['OMEN_APPLE_TEAM', 'OMEN_IOS_SIGN_IDENTITY', 'OMEN_IOS_PROVISIONING_PROFILE']:
            if not sign or not os.environ.get(key):
                raise ValueError(f'Device packaging requires --sign and {key}')
    target = 'aarch64-apple-ios' if device else ('aarch64-apple-ios-sim' if os.uname().machine == 'arm64' else 'x86_64-apple-ios')
    env = cargo_env(); env['IPHONEOS_DEPLOYMENT_TARGET'] = '15.0'
    run(['cargo', 'build', '-p', config.get('native_package', config['package']), '--bin', config.get('native_package', config['package']), '--profile', ('release' if release else 'mobile'), '--no-default-features', '--features', 'native', '--target', target, '--locked'], env)
    bundle = out / 'OmenApp.app'; bundle.mkdir(parents=True, exist_ok=True)
    # GPUI owns UIApplicationMain and the UIKit/Metal view; no WKWebView host.
    for stale in ['Assets', 'Frameworks', '_CodeSignature']:
        if (bundle / stale).exists(): shutil.rmtree(bundle / stale)
    shutil.copy2(TARGET_DIR / target / ('release' if release else 'mobile') / config.get('native_package', config['package']), bundle / 'OmenApp')
    info = {'CFBundleExecutable':'OmenApp', 'CFBundleIdentifier':config['identifier'],
            'CFBundleName':config['name'], 'CFBundlePackageType':'APPL',
            'CFBundleShortVersionString':config['version'], 'CFBundleVersion':'1',
            'MinimumOSVersion':'15.0', 'LSRequiresIPhoneOS':True, 'UIDeviceFamily':[1,2],
            'CFBundleInfoDictionaryVersion':'6.0', 'CFBundleDevelopmentRegion':'en', 'UIRequiresFullScreen':True,
            'UIApplicationSceneManifest':{'UIApplicationSupportsMultipleScenes':False, 'UISceneConfigurations':{'UIWindowSceneSessionRoleApplication':[{'UISceneConfigurationName':'Default Configuration','UISceneDelegateClassName':'GPUIIosSceneDelegate'}]}},
            'UILaunchScreen':{}, 'UISupportedInterfaceOrientations':['UIInterfaceOrientationPortrait','UIInterfaceOrientationLandscapeLeft','UIInterfaceOrientationLandscapeRight']}
    (bundle / 'Assets.car').unlink(missing_ok=True)
    for stale_icon in bundle.glob('AppIcon*.png'):
        stale_icon.unlink()
    if icons:
        info.update(compile_ios_icon(icons, bundle, device))
    (bundle / 'Info.plist').write_bytes(plistlib.dumps(info))
    copy_assets(bundle / 'assets')
    identity = os.environ.get('OMEN_IOS_SIGN_IDENTITY') if sign else '-'
    if not identity: raise ValueError('Signing requires OMEN_IOS_SIGN_IDENTITY')
    arguments = ['codesign','--force','--sign',identity]
    if device:
        import fnmatch
        profile = Path(os.environ['OMEN_IOS_PROVISIONING_PROFILE']).expanduser()
        payload = subprocess.check_output(['security','cms','-D','-i',str(profile)])
        entitlements = plistlib.loads(payload)['Entitlements']
        app_id = os.environ['OMEN_APPLE_TEAM'] + '.' + config['identifier']
        if not fnmatch.fnmatchcase(app_id, entitlements.get('application-identifier','')):
            raise ValueError('Provisioning profile does not match the application identifier and team')
        entitlements['application-identifier'] = app_id
        entitlement_file = out / 'entitlements.plist'; entitlement_file.write_bytes(plistlib.dumps(entitlements))
        shutil.copy2(profile, bundle / 'embedded.mobileprovision')
        arguments += ['--entitlements', entitlement_file]
    run(arguments + [bundle]); run(['codesign','--verify','--strict',bundle])
    artifact(out, 'ios', bundle, config)
    print(f'Built native GPUI iOS application: {bundle}')

def artifact(out, platform, path, config, **extra):
    (out / 'omen-artifact.json').write_text(json.dumps({'platform':platform, 'path':str(path.resolve()), 'bundle_id':config['identifier'], **extra}, indent=2))

def main():
    global PROJECT_ROOT, TARGET_DIR, CONFIG_PATH, CURRENT_CONFIG
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('platform', choices=['web', 'desktop', 'android', 'ios'])
    parser.add_argument('--out', type=Path)
    parser.add_argument('--project-root', type=Path, default=ROOT)
    parser.add_argument('--config', type=Path)
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('--icons', type=Path, help='Generated icon resources from the Omen CLI')
    parser.add_argument('--release', action='store_true', help='Build optimized release output (Android requires signing credentials)')
    parser.add_argument('--sign', action='store_true', help='Use configured signing identity; does not publish')
    parser.add_argument('--device', action='store_true', help='Build iOS device artifact rather than simulator')
    args = parser.parse_args()
    PROJECT_ROOT = args.project_root.resolve()
    TARGET_DIR = args.target_dir.resolve() if args.target_dir else PROJECT_ROOT / 'target'
    CONFIG_PATH = args.config.resolve() if args.config else None
    config = configuration(); CURRENT_CONFIG = config
    if args.platform not in config.get('targets', ['desktop', 'android', 'ios', 'web']):
        raise ValueError(f"Target '{args.platform}' is not enabled for this project")
    out = args.out.resolve() if args.out else TARGET_DIR / 'packages' / args.platform
    icons = args.icons.resolve() if args.icons else None
    if args.platform == 'web': web(config, out, icons)
    elif args.platform == 'android': android(config, out, args.release, icons)
    elif args.platform == 'ios': ios(config, out, args.device, args.sign, args.release, icons)
    else: desktop(config, out, args.sign, args.release, icons)

if __name__ == '__main__':
    try: main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'Packaging failed: {error}', file=sys.stderr); sys.exit(1)
