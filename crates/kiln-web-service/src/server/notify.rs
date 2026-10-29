use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::path::Path;

// r[impl kiln.web.deploy.runtime]
pub(super) fn ready() -> Result<(), String> {
    match std::env::var_os("NOTIFY_SOCKET") {
        Some(path) => send_ready(&path),
        None => Ok(()),
    }
}

fn send_ready(path: &OsStr) -> Result<(), String> {
    let bytes = path.as_bytes();
    let address = match bytes.first() {
        Some(b'/') => SocketAddr::from_pathname(Path::new(path)),
        #[cfg(target_os = "linux")]
        Some(b'@') if bytes.len() > 1 => {
            use std::os::linux::net::SocketAddrExt;
            SocketAddr::from_abstract_name(bytes.get(1..).unwrap_or_default())
        }
        _ => return Err("supervisor notification socket is invalid".into()),
    }
    .map_err(|_| "supervisor notification socket is invalid")?;
    let socket = UnixDatagram::unbound().map_err(|_| "supervisor notification unavailable")?;
    socket
        .set_nonblocking(true)
        .map_err(|_| "supervisor notification unavailable")?;
    socket
        .connect_addr(&address)
        .map_err(|_| "supervisor notification failed")?;
    if socket
        .send(b"READY=1")
        .map_err(|_| "supervisor notification failed")?
        != 7
    {
        return Err("supervisor notification incomplete".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify kiln.web.deploy.runtime.scenario.ready]
    #[test]
    fn readiness_reaches_supervisor_and_missing_supervisor_fails_closed() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("notify.sock");
        let receiver = UnixDatagram::bind(&path).unwrap_or_else(|_| unreachable!());
        receiver
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .unwrap_or_else(|_| unreachable!());
        send_ready(path.as_os_str()).unwrap_or_else(|_| unreachable!());
        let mut bytes = [0; 32];
        let count = receiver.recv(&mut bytes).unwrap_or_else(|_| unreachable!());
        assert_eq!(bytes.get(..count), Some(b"READY=1".as_slice()));
        assert!(send_ready(root.path().join("missing.sock").as_os_str()).is_err());
        assert!(send_ready(OsStr::new("relative.sock")).is_err());
        assert!(send_ready(OsStr::new("")).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn readiness_supports_systemd_abstract_socket_names() {
        use std::os::linux::net::SocketAddrExt;
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let mut name = b"kiln-notify-".to_vec();
        name.extend_from_slice(root.path().as_os_str().as_bytes());
        let address = SocketAddr::from_abstract_name(&name).unwrap_or_else(|_| unreachable!());
        let receiver = UnixDatagram::bind_addr(&address).unwrap_or_else(|_| unreachable!());
        receiver
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .unwrap_or_else(|_| unreachable!());
        let mut path = vec![b'@'];
        path.extend(name);
        send_ready(OsStr::from_bytes(&path)).unwrap_or_else(|_| unreachable!());
        let mut bytes = [0; 32];
        let count = receiver.recv(&mut bytes).unwrap_or_else(|_| unreachable!());
        assert_eq!(bytes.get(..count), Some(b"READY=1".as_slice()));
    }
}
