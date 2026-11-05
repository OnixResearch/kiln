//! Native guest denial probe: no networking library, fork, exec or subprocess harness.
use std::ffi::{c_char, c_int, c_long, c_void};
unsafe extern "C" {
    fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    fn connect(fd: c_int, address: *const c_void, length: u32) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn syscall(number: c_long, ...) -> c_long;
}
const AF_INET: u16 = 2;
const AF_VSOCK: u16 = 40;
const SOCK_STREAM: c_int = 1;
const SYS_FORK: c_long = 57;
const SYS_EXECVE: c_long = 59;
#[repr(C)]
struct Inet {
    family: u16,
    port: u16,
    address: [u8; 4],
    padding: [u8; 8],
}
#[repr(C)]
struct Vsock {
    family: u16,
    reserved: u16,
    port: u32,
    cid: u32,
    padding: [u8; 4],
}
fn probe(domain: u16, address: *const c_void, length: u32, label: &str) {
    // Connecting with a refused socket also demonstrates that no usable channel
    // was created. If socket unexpectedly works, connect is still attempted and
    // the probe fails even if the particular remote endpoint refuses it.
    let fd = unsafe { socket(domain.into(), SOCK_STREAM, 0) };
    let connected = unsafe { connect(fd, address, length) };
    println!("{label}: socket={fd} connect={connected}");
    if fd >= 0 {
        unsafe {
            close(fd);
        }
    }
    assert!(
        fd < 0 && connected < 0,
        "network syscall unexpectedly admitted: {label}"
    );
}
fn main() {
    assert_eq!(
        std::env::args().count(),
        1,
        "unexpected VMM or loader argument forwarded"
    );
    let inet = Inet {
        family: AF_INET,
        port: 443_u16.to_be(),
        address: [1, 1, 1, 1],
        padding: [0; 8],
    };
    probe(
        AF_INET,
        (&inet as *const Inet).cast(),
        std::mem::size_of::<Inet>() as u32,
        "AF_INET",
    );
    for cid in [1, 2] {
        let vsock = Vsock {
            family: AF_VSOCK,
            reserved: 0,
            port: 10240,
            cid,
            padding: [0; 4],
        };
        probe(
            AF_VSOCK,
            (&vsock as *const Vsock).cast(),
            std::mem::size_of::<Vsock>() as u32,
            &format!("AF_VSOCK CID {cid}"),
        );
    }
    let fork = unsafe { syscall(SYS_FORK) };
    println!("fork={fork}");
    assert!(fork < 0, "fork unexpectedly admitted");
    let path = b"/test\0";
    let argv = [path.as_ptr().cast::<c_char>(), std::ptr::null()];
    let env: [*const c_char; 1] = [std::ptr::null()];
    let exec = unsafe { syscall(SYS_EXECVE, path.as_ptr(), argv.as_ptr(), env.as_ptr()) };
    println!("execve={exec}");
    assert!(exec < 0, "exec unexpectedly admitted");
    println!("native network and process denial probe passed");
}
