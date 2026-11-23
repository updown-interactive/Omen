#!/usr/bin/env python3
"""Create a disposable headless ARM64 emulator and run the Android integration suite."""
import os
from pathlib import Path
import subprocess
import time
ROOT=Path(__file__).resolve().parents[1]
def main():
    sdk=Path(os.environ['ANDROID_HOME']);env=os.environ.copy()
    env['ANDROID_AVD_HOME']=str(ROOT/'target/test-avd');env['ANDROID_USER_HOME']=str(ROOT/'target/test-android-home')
    Path(env['ANDROID_AVD_HOME']).mkdir(parents=True,exist_ok=True);Path(env['ANDROID_USER_HOME']).mkdir(parents=True,exist_ok=True)
    manager=sdk/'cmdline-tools/latest/bin/avdmanager';name='OmenPhase7';serial='emulator-5580';adb=sdk/'platform-tools/adb'
    def run(args,**kwargs):return subprocess.run([str(a) for a in args],env=env,cwd=ROOT,check=True,**kwargs)
    # The AVD and its data are entirely inside the ignored build directory.
    run([manager,'create','avd','--force','--name',name,'--path',ROOT/'target/test-avd/OmenPhase7.avd','--package','system-images;android-36;google_apis_playstore;arm64-v8a','--device','pixel_7'],input='no\n',text=True)
    log=(ROOT/'target/android-emulator.log').open('w')
    emulator=subprocess.Popen([str(sdk/'emulator/emulator'),'-avd',name,'-port','5580','-no-window','-no-audio','-no-boot-anim','-no-snapshot','-gpu','swiftshader_indirect'],env=env,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
    try:
        deadline=time.monotonic()+180
        while time.monotonic()<deadline:
            if emulator.poll() is not None:raise RuntimeError('Emulator stopped; inspect target/android-emulator.log')
            status=subprocess.run([str(adb),'-s',serial,'shell','getprop','sys.boot_completed'],capture_output=True,text=True,env=env)
            if status.stdout.strip()=='1':break
            time.sleep(2)
        else:raise RuntimeError('Emulator boot timed out')
        testenv=env.copy();testenv['ANDROID_SERIAL']=serial
        if os.environ.get('OMEN_TEST_SKIP_INSTRUMENTATION') != '1':
            subprocess.run([env.get('GRADLE','gradle'),'--no-daemon','-p','platforms/android-app',':connectedDebugAndroidTest'],cwd=ROOT,env=testenv,check=True)
        # AGP uninstalls instrumented packages after the test. Install the standalone
        # application again for an independent native launch and display capture.
        run([adb,'-s',serial,'install','-r',ROOT/'target/packages/android/Omen.apk'])
        run([adb,'-s',serial,'shell','am','start','-n','dev.omen.platformdemo/dev.omen.app.OmenActivity'])
        deadline=time.monotonic()+90
        presented=False
        while time.monotonic()<deadline:
            pid=subprocess.run([str(adb),'-s',serial,'shell','pidof','dev.omen.platformdemo'],capture_output=True,text=True,env=env).stdout.strip()
            if pid:
                text=subprocess.run([str(adb),'-s',serial,'logcat','-d','--pid='+pid,'-s','gpui_android:I'],capture_output=True,text=True,env=env).stdout
                if 'GPUI Vulkan frame presented' in text:presented=True;break
            time.sleep(1)
        if not presented:raise RuntimeError('Standalone native app did not present a Vulkan frame')
        with (ROOT/'target/android-native-screen.png').open('wb') as image:
            subprocess.run([str(adb),'-s',serial,'exec-out','screencap','-p'],stdout=image,env=env,check=True,timeout=30)
        if not (ROOT/'target/android-native-screen.png').read_bytes().startswith(b'\x89PNG\r\n\x1a\n'):
            raise RuntimeError('Android screen capture failed')
        if os.environ.get('OMEN_TEST_KEEP_EMULATOR') == '1':
            release=ROOT/'target/android-test-release'
            release.unlink(missing_ok=True)
            print('Native screen ready; retaining owned emulator for up to 180 seconds',flush=True)
            deadline=time.monotonic()+180
            while time.monotonic()<deadline and not release.exists():time.sleep(1)
    finally:
        try:
            with (ROOT/'target/android-native-logcat.txt').open('w') as output:
                subprocess.run([str(adb),'-s',serial,'logcat','-d'],stdout=output,env=env,check=False,timeout=15)
        except subprocess.TimeoutExpired:pass
        subprocess.run([str(adb),'-s',serial,'emu','kill'],env=env,check=False)
        try:emulator.wait(timeout=15)
        except subprocess.TimeoutExpired:emulator.terminate();emulator.wait(timeout=15)
        log.close()
if __name__=='__main__':main()
