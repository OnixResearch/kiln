use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    assert_eq!(argv.len(), 2, "unexpected VMM or loader argument forwarded");
    assert_eq!(argv[0], "/test");
    assert!(matches!(
        argv[1].as_str(),
        "pass" | "fail" | "binary" | "loud" | "fixture"
    ));
    println!("static Rust native test argv={argv:?}");
    if argv[1] == "binary" {
        std::io::stdout()
            .write_all(b"\xff\n")
            .expect("binary stdout");
    } else if argv[1] == "loud" {
        std::io::stdout()
            .write_all(&[b'x'; 4096])
            .expect("bounded overflow probe");
        println!();
    } else if argv[1] == "fixture" {
        let path = "/readonly.txt";
        assert_eq!(
            std::fs::read(path).expect("declared fixture"),
            b"immutable fixture\n"
        );
        assert!(std::fs::OpenOptions::new().write(true).open(path).is_err());
        assert_eq!(
            std::fs::read(path).expect("unchanged fixture"),
            b"immutable fixture\n"
        );
        println!("declared fixture read succeeds; write denied; bytes unchanged");
    }
    if argv[1] == "fail" {
        ExitCode::from(7)
    } else {
        ExitCode::SUCCESS
    }
}
