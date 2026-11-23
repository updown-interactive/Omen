#!/usr/bin/env python3
"""Launch the native GPUI package on a disposable simulator and capture its screen."""
import json
import os
import re
from pathlib import Path
import subprocess
import time
ROOT=Path(__file__).resolve().parents[1]
def command(*args):return subprocess.check_output(args,cwd=ROOT,text=True).strip()
def cleanup(*arguments):
    try:subprocess.run(arguments,cwd=ROOT,check=False,timeout=20)
    except subprocess.TimeoutExpired:print('Simulator cleanup timed out: '+str(arguments),flush=True)
def main():
    bundle=ROOT/'target/packages/ios/OmenApp.app'
    if not bundle.exists():raise RuntimeError('Build the native app with scripts/package.py ios first')
    config=json.loads((ROOT/'omen.app.json').read_text())
    available=json.loads(command('xcrun','simctl','list','devices','available','-j'))['devices']
    candidates=[(runtime,device) for runtime,devices in available.items() for device in devices if 'iOS' in runtime and 'iPhone' in device.get('deviceTypeIdentifier','')]
    runtime,template=sorted(candidates,key=lambda item:item[0],reverse=True)[0]
    device=command('xcrun','simctl','create','Omen Native GPUI Test',template['deviceTypeIdentifier'],runtime)
    try:
        subprocess.run(['xcrun','simctl','boot',device],check=True,cwd=ROOT)
        subprocess.run(['xcrun','simctl','bootstatus',device,'-b'],check=True,cwd=ROOT,timeout=180)
        subprocess.run(['xcrun','simctl','install',device,str(bundle)],check=True,cwd=ROOT,timeout=90)
        environment=os.environ.copy();environment['SIMCTL_CHILD_OMEN_NATIVE_TRACE']='1'
        output=ROOT/'target/ios-native-launch.log'
        output.write_text('');(ROOT/'target/ios-native-stdout.log').write_text('')
        launched=subprocess.run(['xcrun','simctl','launch','--terminate-running-process',
            '--stdout='+str(ROOT/'target/ios-native-stdout.log'), '--stderr='+str(output),
            device,config['identifier']],cwd=ROOT,capture_output=True,text=True,env=environment,check=True,timeout=90)
        match=re.search(re.escape(config['identifier'])+r': (\d+)',launched.stdout)
        if not match:raise RuntimeError('Native launch did not report a process ID: '+launched.stdout)
        app_pid=int(match.group(1))
        time.sleep(10)
        os.kill(app_pid,0)
        subprocess.run(['xcrun','simctl','io',device,'screenshot',str(ROOT/'target/ios-native-screen.png')],check=True,cwd=ROOT)
        print('Captured target/ios-native-screen.png; review pixels and input on a qualified device.')
    finally:
        cleanup('xcrun','simctl','terminate',device,config['identifier'])
        cleanup('xcrun','simctl','shutdown',device)
        cleanup('xcrun','simctl','delete',device)
if __name__=='__main__':main()
