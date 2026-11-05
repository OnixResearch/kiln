import pathlib,subprocess,json,socket,time,os
out=pathlib.Path('/tmp/kiln-native-preboot-explicit');out.mkdir(exist_ok=True)
allowlist=pathlib.Path('crates/kiln-hosted/src/unikernel-preboot-syscalls.txt').read_text().strip()
probe=subprocess.run(['systemd-run','--user','--wait','--pipe','--collect','--unit=kiln-native-explicit-denial','--property=NoNewPrivileges=yes','--property=SystemCallFilter='+allowlist,'--property=SystemCallErrorNumber=EPERM','/run/current-system/sw/bin/python3','-c','import ctypes; c=ctypes.CDLL(None,use_errno=True);r=c.ptrace(0,0,None,None);print("ptrace_result",r,"errno",ctypes.get_errno());assert r==-1 and ctypes.get_errno()==1'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
(out/'ptrace-denial.log').write_bytes(probe.stdout+probe.stderr);assert probe.returncode==0
guest=json.loads(pathlib.Path('/nix/store/viz9v99ynqphffnf82lyl9r82313kqjg-kiln-unikraft-firecracker-a4c494e80141bd402961c7d96150dd218c79db37/manifest.json').read_text())
image=json.loads(pathlib.Path('/nix/store/xy0b37h2vfg5yspd0ywpq32a16zgc5xw-kiln-unikernel-rust-image/manifest.json').read_text())
for filtered in [False,True]:
 unit='kiln-native-preboot-'+str(filtered).lower();api=out/(unit+'.sock')
 command=['systemd-run','--user','--wait','--pipe','--collect','--unit='+unit,'--property=NoNewPrivileges=yes']
 if filtered:command+=['--property=SystemCallFilter='+allowlist,'--property=SystemCallErrorNumber=EPERM']
 command += [guest['firecracker']['path'],'--api-sock',str(api)]
 p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE);deadline=time.monotonic()+8
 try:
  while not api.exists():
   if time.monotonic()>deadline or p.poll() is not None:raise RuntimeError('API startup failed')
   time.sleep(.01)
  group=subprocess.check_output(['systemctl','--user','show',unit,'--property=ControlGroup','--value'],text=True).strip()
  pids=(pathlib.Path('/sys/fs/cgroup')/group.lstrip('/')/'cgroup.procs').read_text().split();assert len(pids)==1
  pid=pids[0];statuses={task.name:(task/'status').read_text() for task in (pathlib.Path('/proc')/pid/'task').iterdir()}
  modes={tid:int(next(line.split(':')[1] for line in text.splitlines() if line.startswith('Seccomp:'))) for tid,text in statuses.items()}
  (out/(unit+'.status.json')).write_text(json.dumps({'phase':'before-InstanceStart','seccomp_per_thread':modes,'statuses':statuses},indent=2))
  print('filtered',filtered,'prestart_modes',modes)
  if not filtered:
   assert 0 in modes.values();print('unfiltered main refused before application dispatch');continue
  assert set(modes.values())=={2}
  properties=subprocess.check_output(['systemctl','--user','show',unit,'--property=SystemCallFilter','--property=SystemCallErrorNumber'],text=True)
  actual=next(line.split('=',1)[1] for line in properties.splitlines() if line.startswith('SystemCallFilter='));assert actual.split()==allowlist.split()
  assert 'SystemCallErrorNumber=1' in properties
  manager=subprocess.check_output(['systemctl','--user','show','--property=Version','--property=Features'],text=True)
  (out/'observed-policy.json').write_text(json.dumps({'policy':'kiln.native.preboot.v1','properties':properties,'manager':manager,'phase':'before-InstanceStart','strict_thread_modes':modes},indent=2))
  def put(path,body):
   payload=json.dumps(body).encode();s=socket.socket(socket.AF_UNIX);s.settimeout(4);s.connect(str(api));s.sendall(f'PUT {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {len(payload)}\r\n\r\n'.encode()+payload);data=s.recv(4096);s.close();assert b' 204 ' in data or b' 200 ' in data,data
  put('/machine-config',{'vcpu_count':1,'mem_size_mib':256,'smt':False})
  put('/boot-source',{'kernel_image_path':guest['kernel']['path'],'initrd_path':image['image']['path'],'boot_args':'elfloader /test pass KILN_ARGV_END'})
  put('/actions',{'action_type':'InstanceStart'})
  stdout,stderr=p.communicate(timeout=10);(out/(unit+'.serial.log')).write_bytes(stdout);(out/(unit+'.vmm.log')).write_bytes(stderr)
  assert p.returncode==0 and b'"exit_code":0' in stdout,stdout+stderr
  print('strict preboot filtered real native Rust pass observed',p.returncode)
 finally:
  subprocess.run(['systemctl','--user','stop',unit],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  if p.poll() is None:p.kill();p.communicate()
