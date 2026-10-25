use std::process::Command;
use std::fs;
use std::path::Path;

const SHITLANG_DIR: &str = "D:/Athalla/Desktop/ShitLang";
const EXE_PATH: &str = "D:/Athalla/Desktop/ShitLang/target/debug/shitlang.exe";

fn run_shitlang(code: &str) -> String {
    let output = Command::new(EXE_PATH)
        .arg("--eval")
        .arg(code)
        .output()
        .expect("Failed to execute shitlang");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn ensure_binary() {
    if !Path::new(EXE_PATH).exists() {
        Command::new("cargo")
            .args(&["build"])
            .current_dir(SHITLANG_DIR)
            .output()
            .expect("Failed to build shitlang");
    }
}

#[test]
fn test_say() {
    ensure_binary();
    let output = run_shitlang(r#"say "Hello from ShitLang""#);
    assert_eq!(output.trim(), "Hello from ShitLang");
}

#[test]
fn test_rename_with_numbering() {
    ensure_binary();
    let test_dir = format!("{}/test_rename_dir", SHITLANG_DIR);

    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(format!("{}/old1.txt", test_dir), "content1").unwrap();
    fs::write(format!("{}/old2.txt", test_dir), "content2").unwrap();

    let pattern = format!("{}/*.txt", test_dir);
    let rename_code = format!(r#"rename "{}" to new_{{n}}"#, pattern);
    run_shitlang(&rename_code);

    assert!(Path::new(&format!("{}/new_001.txt", test_dir)).exists());
    assert!(Path::new(&format!("{}/new_002.txt", test_dir)).exists());

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_set_and_interpolate() {
    ensure_binary();
    let output = run_shitlang(r#"set name = "World"
say "Hello {name}""#);
    let combined = output.trim().lines().last().unwrap_or("");
    assert_eq!(combined, "Hello World");
}

#[test]
fn test_file_write() {
    ensure_binary();
    let test_file = format!("{}/test_io_file.txt", SHITLANG_DIR);

    let _ = fs::remove_file(&test_file);
    run_shitlang(&format!(r#"write "test content" to "{}""#, test_file));
    assert_eq!(fs::read_to_string(&test_file).unwrap().trim(), "test content");

    let _ = fs::remove_file(&test_file);
}

#[test]
fn test_count() {
    ensure_binary();
    let test_dir = format!("{}/test_count_dir", SHITLANG_DIR);

    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    fs::write(format!("{}/a.txt", test_dir), "a").unwrap();
    fs::write(format!("{}/b.txt", test_dir), "b").unwrap();

    let pattern = format!("{}/*.txt", test_dir);
    let output = run_shitlang(&format!(r#"count "{}""#, pattern));
    assert!(output.contains("2"));

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_if_condition() {
    ensure_binary();
    let output = run_shitlang(r#"if 5 > 3 : say "yes""#);
    assert_eq!(output.trim(), "yes");
}
