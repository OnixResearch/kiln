{ pkgs, package }:
let
  serviceUid = 41001;
  serviceGid = 41001;
  port = 9443;
  profile =
    pkgs.runCommand "kiln-web-deployment-profile.json" { nativeBuildInputs = [ pkgs.nickel ]; }
      ''
        cat > profile.ncl <<'NICKEL'
        let base = import "${../config}/fixtures/web-systemd-inputs.ncl" in
        let project = std.array.first base.projects in
        (import "${../config}/web-systemd.ncl") (base & {
          service_id | force = "vm-deployment",
          listen_address | force = "127.0.0.1:${toString port}",
          origin | force = "https://localhost:${toString port}",
          projects | force = [project & {
            repository_path | force = "/srv/kiln-source",
            workflow | force = project.workflow & {
              socket_path | force = "/run/kiln-workflow/lattice.sock",
              allowed_user_ids | force = [${toString serviceUid}],
            },
          }],
        })
        NICKEL
        nickel export --format json profile.ncl > "$out"
      '';
in
pkgs.testers.runNixOSTest {
  name = "kiln-web-service-deployment";
  nodes.machine = {
    virtualisation.memorySize = 1024;
    users.groups.kiln-web-service.gid = serviceGid;
    users.users.kiln-web-service = {
      isSystemUser = true;
      uid = serviceUid;
      group = "kiln-web-service";
      home = "/var/lib/kiln-web-service";
    };
    systemd.packages = [ package ];
    environment.systemPackages = [
      pkgs.curl
      pkgs.openssl
      pkgs.python3
      pkgs.util-linux
    ];
    environment.etc."kiln-web-service/profile.json".source = profile;
    systemd.services.kiln-web-fixture = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig.Type = "oneshot";
      serviceConfig.RemainAfterExit = true;
      path = [
        pkgs.coreutils
        pkgs.git
        pkgs.openssl
        pkgs.python3
      ];
      script = ''
        install -d -m0700 /var/lib/kiln-web-service-credentials /var/lib/kiln-web-test
        install -d -m0755 /run/kiln-workflow /srv/kiln-source
        cd /srv/kiln-source
        git init -b main
        git -c user.name=Fixture -c user.email=fixture@example.invalid commit --allow-empty -m fixture
        chown -R kiln-web-service:kiln-web-service /srv/kiln-source
        openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj '/CN=Isolated deployment CA' \
          -addext basicConstraints=critical,CA:TRUE \
          -keyout /var/lib/kiln-web-test/ca-key.pem -out /var/lib/kiln-web-test/ca.pem
        openssl req -new -newkey rsa:2048 -nodes -subj '/CN=localhost' \
          -keyout /var/lib/kiln-web-service-credentials/tls-private-key.pem \
          -out /var/lib/kiln-web-test/server.csr
        printf '%s\n' 'basicConstraints=critical,CA:FALSE' 'extendedKeyUsage=serverAuth' \
          'subjectAltName=DNS:localhost' > /var/lib/kiln-web-test/server.ext
        openssl x509 -req -in /var/lib/kiln-web-test/server.csr \
          -CA /var/lib/kiln-web-test/ca.pem -CAkey /var/lib/kiln-web-test/ca-key.pem \
          -CAcreateserial -days 1 -extfile /var/lib/kiln-web-test/server.ext \
          -out /var/lib/kiln-web-service-credentials/tls-certificate.pem
        python3 -c 'from pathlib import Path; p=Path("/var/lib/kiln-web-service-credentials"); (p/"operator-token").write_text("a"*64); (p/"worker-token").write_text("b"*64)'
        chmod 0600 /var/lib/kiln-web-service-credentials/* /var/lib/kiln-web-test/ca-key.pem
      '';
    };
  };
  testScript = ''
    import json
    import shlex

    start_all()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("kiln-web-fixture.service")
    origin = "https://localhost:${toString port}"
    curl = "curl --silent --show-error --fail --cacert /var/lib/kiln-web-test/ca.pem "
    operator = "a" * 64
    worker = "b" * 64
    rotated = "c" * 64

    def api(path, token, data=None):
        command = curl + "-H " + shlex.quote("Authorization: Bearer " + token)
        if data is not None:
            command += " -H " + shlex.quote("Origin: " + origin)
            command += " -H 'Content-Type: application/json' --data " + shlex.quote(json.dumps(data))
        return json.loads(machine.succeed(command + " " + shlex.quote(origin + path)))

    with subtest("packaged unit reaches genuine notify readiness with private credentials"):
        machine.succeed("systemctl start kiln-web-service.service")
        machine.wait_for_unit("kiln-web-service.service")
        assert json.loads(machine.succeed(curl + origin + "/healthz")) == {"status": "ok"}
        pid = machine.succeed("systemctl show --value -p MainPID kiln-web-service.service").strip()
        assert machine.succeed("ps -o uid= -p " + pid).strip() == "${toString serviceUid}"
        assert machine.succeed("stat -c '%u:%a' /var/lib/kiln-web-service").strip() == "${toString serviceUid}:700"
        destination = "/run/credentials/kiln-web-service.service/operator-token"
        assert machine.succeed("nsenter -t " + pid + " -m -- stat -c '%u:%a' " + destination).strip() == "0:440"
        machine.fail(curl + origin + "/api/projects")
        machine.fail(curl + "--tls-max 1.2 " + origin + "/healthz")
        api("/api/projects", operator, {"project_id": "fixture", "request_id": "vm-enroll"})
        projects_before = api("/api/projects", operator)
        assert [project["id"] for project in projects_before["enrolled"]] == ["fixture"]

    with subtest("supervisor restarts without losing durable enrollment"):
        machine.succeed("kill -KILL " + pid)
        machine.wait_until_succeeds("test $(systemctl show --value -p MainPID kiln-web-service.service) != " + pid + " && " + curl + origin + "/healthz")
        machine.wait_for_unit("kiln-web-service.service")
        assert api("/api/projects", operator) == projects_before

    with subtest("controlled credential rotation rejects the original authority"):
        machine.succeed("python3 -c 'from pathlib import Path; Path(\"/var/lib/kiln-web-service-credentials/operator-token\").write_text(\"c\"*64)'")
        machine.succeed("systemctl restart kiln-web-service.service")
        machine.wait_for_unit("kiln-web-service.service")
        machine.fail(curl + "-H " + shlex.quote("Authorization: Bearer " + operator) + " " + origin + "/api/projects")
        assert api("/api/projects", rotated) == projects_before

    with subtest("SIGTERM drains and missing credentials cannot become ready"):
        pid = machine.succeed("systemctl show --value -p MainPID kiln-web-service.service").strip()
        machine.succeed("systemctl stop kiln-web-service.service")
        machine.succeed("test ! -d /proc/" + pid)
        machine.succeed("rm /var/lib/kiln-web-service-credentials/worker-token")
        machine.fail("systemctl start kiln-web-service.service")
        machine.succeed("systemctl stop kiln-web-service.service")

    with subtest("preflight admits readonly secrets without modifying canonical state"):
        machine.succeed("python3 - <<'PY'\nimport json,os,shutil\nfrom pathlib import Path\np=Path('/var/lib/kiln-web-preflight'); p.mkdir(mode=0o700)\n(p/'state').mkdir(mode=0o700)\nsource=Path('/var/lib/kiln-web-service-credentials')\nfor dst,src in [('operator-token','operator-token'),('tls-key','tls-private-key.pem'),('tls-certificate','tls-certificate.pem')]:\n shutil.copyfile(source/src,p/dst); (p/dst).chmod(0o400)\n(p/'worker-token').write_text('b'*64); (p/'worker-token').chmod(0o400)\ncfg=json.loads(Path('/etc/kiln-web-service/profile.json').read_text()); cfg['state_directory']=str(p/'state'); cfg['operator_token_file']=str(p/'operator-token'); cfg['worker_token_file']=str(p/'worker-token'); cfg['tls']['private_key_path']=str(p/'tls-key'); cfg['tls']['certificate_path']=str(p/'tls-certificate')\n(p/'profile.json').write_text(json.dumps(cfg));\nfor root,dirs,files in os.walk(p):\n os.chown(root,${toString serviceUid},${toString serviceGid})\n for f in files: os.chown(Path(root)/f,${toString serviceUid},${toString serviceGid})\nPY")
        preflight = "runuser -u kiln-web-service -- ${package}/bin/kiln-web-service check --profile /var/lib/kiln-web-preflight/profile.json"
        machine.succeed(preflight)
        assert machine.succeed("find /var/lib/kiln-web-preflight/state -mindepth 1 -print").strip() == ""
        machine.succeed("chmod 0644 /var/lib/kiln-web-preflight/operator-token")
        machine.fail(preflight)
        machine.succeed("chown root:root /var/lib/kiln-web-preflight/operator-token; chmod 0400 /var/lib/kiln-web-preflight/operator-token; ${pkgs.acl}/bin/setfacl -m u:${toString serviceUid}:r,g::--- /var/lib/kiln-web-preflight/operator-token")
        machine.fail(preflight)
        machine.succeed("${pkgs.acl}/bin/setfacl -b /var/lib/kiln-web-preflight/operator-token; chown kiln-web-service:kiln-web-service /var/lib/kiln-web-preflight/operator-token")
        machine.succeed("chmod 0400 /var/lib/kiln-web-preflight/operator-token; mv /var/lib/kiln-web-preflight/operator-token /var/lib/kiln-web-preflight/original-token; ln -s original-token /var/lib/kiln-web-preflight/operator-token")
        machine.fail(preflight)
  '';
}
