#!/usr/bin/env python3
"""Bounded aspen1-only operator maintenance; private journal, reversible changes."""
import argparse, fcntl, grp, json, os, pathlib, pwd, re, shutil, signal, subprocess, sys, time
ROOT = pathlib.Path('/var/lib/kiln-qualification/aspen1-20261106-cf6e793b-r5')
EXPECTED_HOST_KERNEL = '/nix/store/sby4x25d1wl7nn915c62jps8nr65sncj-linux-7.3/bzImage'
USER = 'kilnqual20261106'
LEASE = 'kiln-qualification-recovery-20261106-r5'
GROUPS = ['kilnqual20261106-sender', 'kilnqual20261106-receiver']
LEASE_SECONDS = 1800  # Bounded maintenance; expiry kills the dedicated account before restoration.
ESSENTIAL = {'accounts-daemon.service','avahi-daemon.service','dbus-broker.service','dbus.service','dhcpcd.service','NetworkManager.service','systemd-networkd.service','systemd-resolved.service','wpa_supplicant.service','sshd.service','tailscaled.service','tailscalesd.service','systemd-journald.service','systemd-logind.service','systemd-udevd.service','systemd-oomd.service','systemd-timesyncd.service','nscd.service','polkit.service','serial-getty@ttyS0.service','user@0.service'}
def run(argv, check=True, timeout=300):
    p = subprocess.run(argv, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
    if check and p.returncode: raise RuntimeError(f'{argv[0]} failed ({p.returncode}): {p.stderr.strip()}')
    return p

def read(p): return pathlib.Path(p).read_text().strip()
def save(j):
    p=ROOT/'journal.json'; tmp=p.with_suffix('.new')
    with open(tmp,'w') as f: json.dump(j,f,indent=2); f.write('\n'); f.flush(); os.fsync(f.fileno())
    os.chmod(tmp,0o600); os.replace(tmp,p)
    fd=os.open(ROOT,os.O_RDONLY|os.O_DIRECTORY); os.fsync(fd); os.close(fd)
def load(): return json.loads((ROOT/'journal.json').read_text())
def units(kind, state):
    p=run(['systemctl','list-units','--type='+kind,'--state='+state,'--plain','--no-legend','--no-pager'])
    return [line.split()[0] for line in p.stdout.splitlines() if line.strip()]
def mounted():
    return any(line.split()[4]=='/sys/fs/resctrl' and ' - resctrl ' in line for line in pathlib.Path('/proc/self/mountinfo').read_text().splitlines())
def children(unit):
    value=run(['systemctl','show',unit,'-p','MainPID','--value']).stdout.strip()
    if not value or value=='0': return []
    p=pathlib.Path(f'/proc/{value}/task/{value}/children')
    return p.read_text().split() if p.exists() else []
def arm_lease(j):
    run(['systemd-run','--unit='+LEASE,'--on-active='+str(LEASE_SECONDS)+'s','--timer-property=AccuracySec=1s',sys.executable,str(ROOT/'host-maintenance.py'),'restore'])
    j['lease_seconds']=LEASE_SECONDS; j['lease_started_at']=time.time(); save(j)
def freeze_intake_and_drain(j):
    # Freeze admission parents, not their running build children. Inspect the
    # native master DB while scheduling is paused; never cancel a remote build.
    intake=['nix-grpc-daemon.service','buildbot-master.service','buildbot-worker.service','nix-daemon.service']
    for unit in intake:
        pid=run(['systemctl','show',unit,'-p','MainPID','--value']).stdout.strip()
        if pid and pid!='0':
            if unit not in j.setdefault('frozen',[]): j['frozen'].append(unit); save(j)
            run(['systemctl','kill','--kill-whom=main','--signal=STOP',unit])
    q=run(['runuser','-u','postgres','--','env','PGOPTIONS=-c default_transaction_read_only=on','psql','-d','buildbot','-Atq','-c','SELECT COUNT(*) FROM builds WHERE complete_at IS NULL;'])
    if q.stderr.strip(): j['preexisting_database_warning']=q.stderr.strip(); save(j)
    if int(q.stdout.strip()) or children('buildbot-worker.service') or children('nix-daemon.service'):
        for unit in j.get('frozen',[]): run(['systemctl','kill','--kill-whom=main','--signal=CONT',unit],check=False)
        j['frozen']=[]; save(j)
        raise RuntimeError('in-flight CI/build work: admission resumed without cancelling it; retry drain only after work completes')
    active=units('socket','active')+units('service','running,activating')
    for unit in ['nix-daemon.socket','nix-grpc-daemon.service','buildbot-master.service','buildbot-worker.service','nix-daemon.service']:
        if unit in active:
            if unit not in j['stopped']: j['stopped'].append(unit); save(j)
            run(['systemctl','stop',unit])
    j['frozen']=[]; save(j)
def snapshot():
    if (ROOT/'journal.json').exists(): raise RuntimeError('refusing to overwrite an existing maintenance journal')
    try: pwd.getpwnam(USER); raise RuntimeError('dedicated account already exists; ownership not established')
    except KeyError: pass
    try: grp.getgrnam(USER); raise RuntimeError('dedicated primary group already exists; ownership not established')
    except KeyError: pass
    if mounted(): raise RuntimeError('preexisting resctrl mount needs a separately reviewed allocation plan')
    grub=pathlib.Path('/boot/grub/grub.cfg').read_text()
    first=re.search(r'^\s*linux\s+([^\n]+)',grub,re.M)
    if not first or 'linux-7.3-bzImage' not in first.group(1): raise RuntimeError('default GRUB kernel is not the reviewed staged 7.3')
    env=pathlib.Path('/boot/grub/grubenv')
    if env.exists() and any(line.startswith(b'next_entry=') and line.split(b'=',1)[1].strip() for line in env.read_bytes().splitlines()): raise RuntimeError('preexisting GRUB one-shot override; do not overwrite operator intent')
    j={'schema':'kiln.operator.aspen1-maintenance.v1','target':'aspen1','authorized':'User selected Authorize maintenance now','created_at':time.time(),'phase':'prepared','smt_original':read('/sys/devices/system/cpu/smt/control'),'ksm_original':read('/sys/kernel/mm/ksm/run'),'preboot_services':units('service','running'),'stopped':[],'account_owned':False,'resctrl_owned':False,'root_schemata_original':None,'groups_owned':[],'smt_changed':False,'restore_errors':[]}
    save(j); arm_lease(j); return {'phase':j['phase'],'worker_children':children('buildbot-worker.service'),'nix_children':children('nix-daemon.service')}
def drain_before_reboot():
    j=load()
    if j['phase']!='prepared': raise RuntimeError('wrong maintenance phase')
    freeze_intake_and_drain(j)
    j['phase']='reboot-ready'; save(j); return {'phase':j['phase']}
def reboot():
    j=load()
    if j['phase']!='reboot-ready': raise RuntimeError('host is not drained for reboot')
    j['phase']='reboot-requested'; save(j)
    run(['systemctl','reboot','--no-block'])
    return {'phase':j['phase']}
def account():
    j=load()
    if j['phase'] not in ['prepared','reboot-requested']: raise RuntimeError('host not staged for provisioning')
    if str(pathlib.Path('/run/booted-system/kernel').resolve())!=EXPECTED_HOST_KERNEL: raise RuntimeError('booted kernel differs from the exact authorized immutable cohort')
    if read('/sys/kernel/mm/ksm/run')!='0': raise RuntimeError('KSM must remain off; no waiver or implicit KSM change')
    try: pwd.getpwnam(USER); raise RuntimeError('unexpected existing dedicated account')
    except KeyError: pass
    j['account_owned']=True; save(j)  # Reserve ownership before the external effect.
    run(['useradd','--system','--user-group','--no-create-home','--home-dir',str(ROOT/'home'),'--shell',shutil.which('nologin') or '/run/current-system/sw/bin/nologin','--groups','kvm',USER])
    u=pwd.getpwnam(USER); j['uid']=u.pw_uid; j['gid']=u.pw_gid; save(j)
    for name in ['home','runtime']:
        p=ROOT/name; p.mkdir(mode=0o700); os.chown(p,u.pw_uid,u.pw_gid)
    run(['loginctl','enable-linger',USER]); run(['systemctl','start',f'user@{u.pw_uid}.service'])
    if j['phase']=='reboot-requested': arm_lease(j)
    j['phase']='account-ready'; save(j)
    return {'phase':j['phase'],'uid':u.pw_uid,'lease_seconds':LEASE_SECONDS}
def quiesce():
    j=load()
    if j['phase']!='account-ready': raise RuntimeError('lease/account must be established first')
    freeze_intake_and_drain(j)
    keep=ESSENTIAL|{f'user@{j["uid"]}.service',LEASE+'.service'}
    before=units('service','running,activating,reloading')
    activation=[u for u in units('timer','active') if not u.startswith(LEASE)]+[u for u in units('socket','active') if not u.startswith(('dbus','sshd','systemd-','avahi','nscd',LEASE))]
    new=[u for u in activation if u not in j['stopped']]; j['stopped']+=new; save(j)
    if activation: run(['systemctl','stop',*activation])
    services=[u for u in dict.fromkeys(j['preboot_services']+before+units('service','running,activating,reloading')) if u not in keep]
    # Stop application ingress/compute before stateful backing services.
    backing=[u for u in services if u.startswith(('postgresql','rustfs','redis','nix-daemon'))]
    front=[u for u in services if u not in backing]
    for batch in [front,backing]:
        new=[u for u in batch if u not in j['stopped']]; j['stopped']+=new; save(j)
        if batch: run(['systemctl','stop',*batch])
    remaining=[u for u in units('service','running,activating,reloading') if u not in keep]
    if remaining: raise RuntimeError('unexpected live service after quiescence: '+','.join(remaining))
    j['smt_changed']=True; save(j); pathlib.Path('/sys/devices/system/cpu/smt/control').write_text('off\n')
    if read('/sys/devices/system/cpu/smt/active')!='0': raise RuntimeError('SMT did not disable')
    j['phase']='quiesced'; save(j); return {'phase':j['phase'],'smt_active':False,'remaining_services':units('service','running')}
def cat():
    j=load()
    if j['phase']!='quiesced': raise RuntimeError('host must be quiesced first')
    if mounted(): raise RuntimeError('unexpected resctrl mount appeared')
    j['resctrl_owned']=True; save(j)
    run(['mount','-t','resctrl','resctrl','/sys/fs/resctrl'])
    base=pathlib.Path('/sys/fs/resctrl'); original=read(base/'schemata'); j['root_schemata_original']=original; save(j)
    cbm=int(read(base/'info/L3/cbm_mask'),16); minimum=int(read(base/'info/L3/min_cbm_bits')); share=int(read(base/'info/L3/shareable_bits'),16)
    bits=cbm.bit_length(); reserve=max(minimum,share.bit_length()); width=(bits-reserve)//3
    if cbm!=(1<<bits)-1 or width<minimum: raise RuntimeError('cannot form two exclusive valid contiguous masks while retaining root/shareable capacity')
    receiver=((1<<width)-1)<<(bits-width); sender=((1<<width)-1)<<(bits-2*width); root=cbm^(sender|receiver)
    if root.bit_count()<minimum or (sender|receiver)&share or sender&receiver or root&(sender|receiver): raise RuntimeError('CAT mask invariant failed')
    rows=[s.lstrip() for s in original.splitlines() if s.lstrip().startswith('L3:')]
    if len(rows)!=1: raise RuntimeError('requires unified L3 CAT')
    domains=[x.split('=')[0] for x in rows[0][3:].split(';') if x]
    j['cat_plan']={'cbm_mask':format(cbm,'x'),'min_cbm_bits':minimum,'shareable_bits':format(share,'x'),'root':format(root,'x'),'sender':format(sender,'x'),'receiver':format(receiver,'x'),'domains':domains}; save(j)
    def schema(mask): return 'L3:'+ ';'.join(d+'='+format(mask,'x') for d in domains)+'\n'
    (base/'schemata').write_text(schema(root))
    for name,mask in zip(GROUPS,[sender,receiver]):
        p=base/name
        if p.exists(): raise RuntimeError('unexpected existing group: '+name)
        j['groups_owned'].append(name); save(j); p.mkdir()
        (p/'schemata').write_text(schema(mask)); (p/'mode').write_text('exclusive\n')
        os.chown(p/'tasks',j['uid'],j['gid']); os.chmod(p/'tasks',0o600)
        if read(p/'mode')!='exclusive' or read(p/'cpus_list'): raise RuntimeError('exclusive group or CPU-default invariant failed')
    j['phase']='ready'; save(j); return {'phase':j['phase'],'cat_plan':j['cat_plan'],'groups':GROUPS}
def restore():
    j=load(); errors=[]
    if j['phase']=='restored': return {'phase':'restored','errors':j.get('restore_errors',[])}
    j['phase']='restoring'; save(j)
    def attempt(label,fn):
        try: fn()
        except Exception as e: errors.append(label+': '+str(e))
    def incomplete():
        j['phase']='restore-incomplete'; j['restore_errors']=errors; save(j)
        return {'phase':j['phase'],'errors':errors}
    for unit in j.get('frozen',[]):
        run(['systemctl','kill','--kill-whom=main','--signal=CONT',unit],check=False)
    j['frozen']=[]; save(j)
    uid=j.get('uid')
    if j['account_owned'] and not j.get('account_teardown_complete'):
        try: actual=pwd.getpwnam(USER)
        except KeyError: actual=None
        if actual:
            if uid is not None and actual.pw_uid!=uid: raise RuntimeError('dedicated account identity changed; do not signal it')
            uid=actual.pw_uid
            if uid==0: raise RuntimeError('refusing root UID as dedicated account')
            attempt('disable owned linger',lambda:run(['loginctl','disable-linger',USER]))
            attempt('mask owned user manager',lambda:run(['systemctl','mask','--runtime',f'user@{uid}.service']))
            run(['loginctl','terminate-user',USER],check=False)
            run(['pkill','-TERM','-u',str(uid)],check=False); time.sleep(1)
            run(['pkill','-KILL','-u',str(uid)],check=False); time.sleep(1)
            survivors=[]
            for p in pathlib.Path('/proc').iterdir():
                if p.name.isdigit():
                    try:
                        if p.stat().st_uid==uid: survivors.append(int(p.name))
                    except FileNotFoundError: pass
            if survivors: errors.append('dedicated processes still alive: '+str(survivors))
            if errors: return incomplete()
            for p in [ROOT/'runtime',ROOT/'home']:
                if p.exists(): os.chown(p,0,0); os.chmod(p,0o700)
            attempt('remove owned account',lambda:run(['userdel',USER]))
            try: pwd.getpwnam(USER); errors.append('dedicated account still exists')
            except KeyError: pass
            if errors: return incomplete()
        j['account_teardown_complete']=True; save(j)  # Never signal a subsequently reused UID.
    if j.get('gid') is not None:
        try: owned_group=grp.getgrnam(USER)
        except KeyError: owned_group=None
        if owned_group:
            if owned_group.gr_gid!=j['gid'] or owned_group.gr_mem or any(p.pw_gid==owned_group.gr_gid for p in pwd.getpwall()):
                errors.append('owned primary group identity or membership changed'); return incomplete()
            attempt('remove owned primary group',lambda:run(['groupdel',USER]))
            if errors: return incomplete()
    for name in reversed(j['groups_owned']):
        p=pathlib.Path('/sys/fs/resctrl')/name
        if p.exists(): attempt('remove '+name,lambda p=p:p.rmdir())
    if errors: return incomplete()
    if j['resctrl_owned'] and mounted():
        foreign=[p.name for p in pathlib.Path('/sys/fs/resctrl').iterdir() if p.is_dir() and p.name not in ['info','mon_data','mon_groups']]
        if foreign: errors.append('new foreign CAT groups; do not overwrite/unmount them: '+str(foreign)); return incomplete()
    if j['root_schemata_original'] is not None and mounted(): attempt('restore root schemata',lambda:pathlib.Path('/sys/fs/resctrl/schemata').write_text(j['root_schemata_original']+'\n'))
    if j['resctrl_owned'] and mounted(): attempt('unmount owned resctrl',lambda:run(['umount','/sys/fs/resctrl']))
    if errors: return incomplete()
    if j['smt_changed']:
        attempt('restore SMT',lambda:pathlib.Path('/sys/devices/system/cpu/smt/control').write_text(j['smt_original']+'\n'))
        if read('/sys/devices/system/cpu/smt/control')!=j['smt_original']: errors.append('SMT restoration did not take effect')
    if errors: return incomplete()
    if uid is not None: attempt('unmask deleted account instance',lambda:run(['systemctl','unmask','--runtime',f'user@{uid}.service']))
    if errors: return incomplete()
    # Only after required account/process/CAT/SMT teardown succeeds may trusted
    # services return. Reverse stop order starts databases before applications.
    # Docker-backed frontends need the daemon ready before their bounded retries.
    if 'docker.service' in j['stopped']: attempt('restart docker.service',lambda:run(['systemctl','start','docker.service'],timeout=180))
    for unit in reversed(j['stopped']): attempt('restart '+unit,lambda unit=unit:run(['systemctl','start',unit],timeout=180))
    if errors: return incomplete()
    run(['systemctl','stop',LEASE+'.timer'],check=False)
    j['restored_at']=time.time(); j['restore_errors']=[]; j['phase']='restored'; save(j)
    return {'phase':'restored','errors':[],'smt_control':read('/sys/devices/system/cpu/smt/control'),'booted_kernel':run(['uname','-r']).stdout.strip()}
def main():
    if os.geteuid()!=0 or os.uname().nodename!='aspen1': raise RuntimeError('aspen1 root only')
    a=argparse.ArgumentParser(); a.add_argument('action',choices=['snapshot','drain-before-reboot','reboot','account','quiesce','cat','restore','status']); action=a.parse_args().action
    if action=='status': result=load()
    else:
        fd=os.open(ROOT/'restore.lock',os.O_CREAT|os.O_RDWR,0o600)
        with os.fdopen(fd,'a') as lock:
            fcntl.flock(lock,fcntl.LOCK_EX)
            if action in ['drain-before-reboot','reboot','account','quiesce','cat']:
                j=load()
                if time.time()>=j['lease_started_at']+j['lease_seconds']: raise RuntimeError('maintenance lease expired; no new mutation admitted')
            result={'snapshot':snapshot,'drain-before-reboot':drain_before_reboot,'reboot':reboot,'account':account,'quiesce':quiesce,'cat':cat,'restore':restore}[action]()
    print(json.dumps(result,indent=2))
    if action=='restore' and result['phase']!='restored': sys.exit(2)
if __name__=='__main__':
    try: main()
    except Exception as e: print(f'{type(e).__name__}: {e}',file=sys.stderr); sys.exit(1)
