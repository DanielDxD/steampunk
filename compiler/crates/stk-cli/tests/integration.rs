use std::path::PathBuf;
use std::process::Command;

fn steampunk_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_steampunk").into()
}

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../examples")
}

#[test]
fn run_hello() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("hello.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello World"), "stdout={stdout}");
}

#[test]
fn run_math() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("math.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("soma=42"), "stdout={stdout}");
}

#[test]
fn build_and_execute_math() {
    let out = std::env::temp_dir().join(format!("stk-math-{}", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(steampunk_bin())
        .args(["build"])
        .arg(examples_dir().join("math.stk"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&out).output().expect("exec binary");
    assert!(run.status.success());
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("soma=42"), "stdout={stdout}");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn run_greet_string_param() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("greet.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Hello, Daniel") && stdout.contains("Hello, John"),
        "stdout={stdout}"
    );
}

#[test]
fn run_control() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("control.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok"), "stdout={stdout}");
    assert!(stdout.contains("i=0"), "stdout={stdout}");
    assert!(stdout.contains("i=1"), "stdout={stdout}");
    assert!(stdout.contains("i=2"), "stdout={stdout}");
    assert!(stdout.contains("um"), "stdout={stdout}");
    assert!(stdout.contains("outro"), "stdout={stdout}");
    assert!(!stdout.contains("fail"), "stdout={stdout}");
}

#[test]
fn build_and_execute_control() {
    let out = std::env::temp_dir().join(format!("stk-control-{}", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(steampunk_bin())
        .args(["build"])
        .arg(examples_dir().join("control.stk"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&out).output().expect("exec binary");
    assert!(run.status.success());
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("ok") && stdout.contains("um") && stdout.contains("outro"));
    let _ = std::fs::remove_file(&out);
}

#[test]
fn rejects_non_bool_if() {
    let dir = std::env::temp_dir().join(format!("stk-ifbad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
fn main() {
    if 1 {
        std.log("x")
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("bool"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_match_without_wildcard() {
    let dir = std::env::temp_dir().join(format!("stk-matchbad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
fn main() {
    match 1 {
        0 => { std.log("zero") }
        1 => { std.log("one") }
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("_") || err.contains("wildcard"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_let() {
    let dir = std::env::temp_dir().join(format!("stk-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
fn main() {
    let x = 1
    std.log("x")
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("let"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_oop() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("oop.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("flying"), "stdout={stdout}");
    assert!(stdout.contains("bird=eagle"), "stdout={stdout}");
    assert!(stdout.contains("drop Named"), "stdout={stdout}");
}

#[test]
fn build_and_execute_oop() {
    let out = std::env::temp_dir().join(format!("stk-oop-{}", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(steampunk_bin())
        .args(["build"])
        .arg(examples_dir().join("oop.stk"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&out).output().expect("exec binary");
    assert!(run.status.success());
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("flying")
            && stdout.contains("bird=eagle")
            && stdout.contains("drop Named"),
        "stdout={stdout}"
    );
    let _ = std::fs::remove_file(&out);
}

#[test]
fn rejects_diamond_inheritance() {
    let dir = std::env::temp_dir().join(format!("stk-diamond-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
class A { pub fn new() A { return self } }
class B :: A { pub fn new() B { return self } }
class C :: A { pub fn new() C { return self } }
class D :: B, C { pub fn new() D { return self } }
fn main() { var d = new D() }
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("diamond") || err.contains("multiple paths"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn param_default_and_field_default() {
    let dir = std::env::temp_dir().join(format!("stk-defaults-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ok.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
class Box {
    pub var n int = 7
    pub fn new() Box { return self }
}
fn show(int x = 3) {
    std.log("x=$1", x)
}
fn main() {
    show()
    var b = new Box()
    std.log("n=$1", b.n)
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("x=3"), "stdout={stdout}");
    assert!(stdout.contains("n=7"), "stdout={stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_member_without_visibility() {
    let dir = std::env::temp_dir().join(format!("stk-vis-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
class Foo {
    var x int
    pub fn new() Foo { self.x = 0; return self }
}
fn main() {
    var f = new Foo()
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        err.contains("pub") || err.contains("priv") || err.contains("prot") || err.contains("visibility"),
        "err={err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_missing_iclass_impl() {
    let dir = std::env::temp_dir().join(format!("stk-iclass-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
iclass Named {
    getName()
}
class Foo : Named {
    pub fn new() Foo { return self }
}
fn main() {
    var f = new Foo()
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("getName") || err.contains("implement"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_modules_arrays_const() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("modules_main.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sum=30"), "stdout={stdout}");
    assert!(stdout.contains("len=3"), "stdout={stdout}");
    assert!(stdout.contains("factor=2"), "stdout={stdout}");
}

#[test]
fn rejects_import_cycle() {
    let dir = std::env::temp_dir().join(format!("stk-cycle-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(
        dir.join("mods/a.stk"),
        r#"
@import ":mods/b"
pub fn a() int { return 1 }
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("mods/b.stk"),
        r#"
@import ":mods/a"
pub fn b() int { return 2 }
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("main.stk"),
        r#"
@import "std"
@import ":mods/a"
fn main() {
    std.log("$1", a())
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(dir.join("main.stk"))
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("cycle") || err.contains("import"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_private_module_fn() {
    let dir = std::env::temp_dir().join(format!("stk-modpriv-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(
        dir.join("mods/util.stk"),
        r#"
fn secret() int { return 1 }
pub fn open() int { return 2 }
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("main.stk"),
        r#"
@import "std"
@import ":mods/util"
fn main() {
    std.log("$1", secret())
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(dir.join("main.stk"))
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("private") || err.contains("secret"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_assign_to_const() {
    let dir = std::env::temp_dir().join(format!("stk-const-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
const N = 1
fn main() {
    N = 2
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("const") || err.contains("assign"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_future_race() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("future_race.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("winner=2"), "stdout={stdout}");
}

#[test]
fn run_buffered_channel() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("buffered.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("done"), "stdout={stdout}");
    assert!(stdout.contains("got 1"), "stdout={stdout}");
    assert!(stdout.contains("got 2"), "stdout={stdout}");
    // Backpressure: second send finishes only after first recv frees a slot.
    let send2_wait = stdout
        .find("P: send 2 (may wait for slot)")
        .expect("missing send2 start");
    let got1 = stdout.find("C: got 1").expect("missing got 1");
    let send2_done = stdout.find("P: sent 2").expect("missing send2 done");
    assert!(
        send2_wait < got1 && got1 < send2_done,
        "expected backpressure ordering; stdout={stdout}"
    );
}

#[test]
fn run_mutex() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("mutex.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("total=4"), "stdout={stdout}");
}

#[test]
fn run_async_block() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("async_block.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("block=7"), "stdout={stdout}");
    assert!(stdout.contains("winner=2"), "stdout={stdout}");
}

#[test]
fn run_await_recv() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("await_recv.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("got=42"), "stdout={stdout}");
}

#[test]
fn run_cpu_submit() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("cpu_submit.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cpu=42"), "stdout={stdout}");
}

#[test]
fn run_closures() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("closures.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stored=42"), "stdout={stdout}");
    assert!(stdout.contains("inline=42"), "stdout={stdout}");
    assert!(stdout.contains("call=42"), "stdout={stdout}");
    assert!(stdout.contains("named=7"), "stdout={stdout}");
}

#[test]
fn run_await_wait() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("await_wait.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("waited"), "stdout={stdout}");
}

#[test]
fn run_result_option() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("result_option.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok=7"), "stdout={stdout}");
    assert!(stdout.contains("fail=non-positive"), "stdout={stdout}");
    assert!(stdout.contains("some=3"), "stdout={stdout}");
    assert!(stdout.contains("empty"), "stdout={stdout}");
}

#[test]
fn run_channel_string() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("channel_string.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("got=hello"), "stdout={stdout}");
    assert!(stdout.contains("got=world"), "stdout={stdout}");
    assert!(stdout.contains("done"), "stdout={stdout}");
}

#[test]
fn run_async_string() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("async_string.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("title=Ada"), "stdout={stdout}");
}

#[test]
fn run_future_join_ready() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("future_join.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ready=100"), "stdout={stdout}");
    assert!(stdout.contains("join=10+32=42"), "stdout={stdout}");
}

#[test]
fn run_async_await_spawn() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("async.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("v=43"), "stdout={stdout}");
    assert!(stdout.contains("spawned"), "stdout={stdout}");
}

#[test]
fn run_channel_waitgroup() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("channel.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sum=12"), "stdout={stdout}");
}

#[test]
fn send_after_close_aborts() {
    let dir = std::env::temp_dir().join(format!("stk-chclose-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
fn main() {
    var ch = std.sync.Channel<int>.new()
    ch.close()
    ch.send(1)
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        err.contains("closed") || err.contains("send"),
        "err={err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_channel_without_std() {
    let dir = std::env::temp_dir().join(format!("stk-chstd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
fn main() {
    var ch = std.sync.Channel<int>.new()
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("std") || err.contains("Channel"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_spawn_concurrent() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("spawn.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // main continues before workers finish — proves spawn is non-blocking.
    let main_idx = stdout
        .find("main: still running")
        .expect("missing main continue line");
    let a_end = stdout.find("A: end").expect("missing A: end");
    let c_end = stdout.find("C: end").expect("missing C: end");
    assert!(
        main_idx < a_end,
        "main should continue before A finishes; stdout={stdout}"
    );
    // C sleeps less than A/B, so it should finish first among workers.
    assert!(
        c_end < a_end,
        "C (50ms) should end before A (200ms); stdout={stdout}"
    );
}

#[test]
fn rejects_await_outside_async() {
    let dir = std::env::temp_dir().join(format!("stk-await-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
async fn answer() int { return 1 }
fn main() {
    var x = await answer()
    std.log("$1", x)
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("await") || err.contains("async"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_priv_access_outside() {
    let dir = std::env::temp_dir().join(format!("stk-priv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
class Foo {
    priv var x int
    pub fn new() Foo {
        self.x = 1
        return self
    }
}
fn main() {
    var f = new Foo()
    std.log("$1", f.x)
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("private") || err.contains("priv"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn assert_float_math_output(stdout: &str) {
    for expected in [
        "area=10",
        "soma=6.5",
        "dif=1.5",
        "div=1.6",
        "metade=1.25",
        "negativo=-1.25",
        "w menor que h",
        "area exata",
    ] {
        assert!(stdout.contains(expected), "missing {expected}; stdout={stdout}");
    }
}

#[test]
fn run_float_math() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("float_math.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_float_math_output(&String::from_utf8_lossy(&output.stdout));
}

#[test]
fn build_and_execute_float_math() {
    let out = std::env::temp_dir().join(format!("stk-float-{}", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(steampunk_bin())
        .args(["build"])
        .arg(examples_dir().join("float_math.stk"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&out).output().expect("exec binary");
    assert!(run.status.success());
    assert_float_math_output(&String::from_utf8_lossy(&run.stdout));
    let _ = std::fs::remove_file(&out);
}

#[test]
fn rejects_mixed_int_float_arithmetic() {
    let dir = std::env::temp_dir().join(format!("stk-mixnum-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
fn main() {
    var x = 1 + 2.5
    std.log("$1", x)
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(err.contains("implicit conversion"), "err={err}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn assert_list_string_output(stdout: &str) {
    for expected in [
        "total=2",
        "primeiro=ada",
        "q0=42 q2=4",
        "saudacao=ola, grace",
        "len=10",
        "slice=grace",
        "contem grace",
        "fromInt=7",
        "parse=123",
        "invalid int: abc",
    ] {
        assert!(stdout.contains(expected), "missing {expected}; stdout={stdout}");
    }
}

#[test]
fn run_list_string() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("list_string.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_list_string_output(&String::from_utf8_lossy(&output.stdout));
}

#[test]
fn build_and_execute_list_string() {
    let out = std::env::temp_dir().join(format!("stk-list-{}", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(steampunk_bin())
        .args(["build"])
        .arg(examples_dir().join("list_string.stk"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&out).output().expect("exec binary");
    assert!(run.status.success());
    assert_list_string_output(&String::from_utf8_lossy(&run.stdout));
    let _ = std::fs::remove_file(&out);
}

#[test]
fn run_fs_io_round_trip() {
    let path = std::env::temp_dir().join(format!("stk-fsio-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("fs_io.stk"))
        .env("STK_FS_PATH", &path)
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("lido=engrenagens a vapor"), "stdout={stdout}");
    assert!(stdout.contains("bytes=19"), "stdout={stdout}");
    assert!(
        stdout.contains("leitura falhou como esperado"),
        "stdout={stdout}"
    );

    let on_disk = std::fs::read_to_string(&path).expect("file written by program");
    assert_eq!(on_disk, "engrenagens a vapor");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn run_cli_env() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("cli_env.stk"))
        .output()
        .expect("run steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("STK_DEMO=engrenagem"), "stdout={stdout}");
    assert!(
        stdout.contains("STK_NAO_DEFINIDA_9137 ausente"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("relogio ok"), "stdout={stdout}");
    // The JIT forwards the host argv, so args() is never empty.
    assert!(stdout.contains("programa="), "stdout={stdout}");
}

#[test]
fn build_and_execute_cli_env() {
    let out = std::env::temp_dir().join(format!("stk-clienv-{}", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(steampunk_bin())
        .args(["build"])
        .arg(examples_dir().join("cli_env.stk"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&out).output().expect("exec binary");
    assert!(run.status.success());
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("argc=1"), "stdout={stdout}");
    assert!(stdout.contains("STK_DEMO=engrenagem"), "stdout={stdout}");
    assert!(stdout.contains("relogio ok"), "stdout={stdout}");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn std_panic_aborts_with_message() {
    let dir = std::env::temp_dir().join(format!("stk-panic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("boom.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
fn main() {
    std.log("antes")
    std.panic("caldeira estourou")
    std.log("depois")
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("caldeira estourou"), "stderr={err}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("depois"), "stdout={stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn std_process_exit_sets_status() {
    let dir = std::env::temp_dir().join(format!("stk-exit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("exit.stk");
    std::fs::write(
        &file,
        r#"
@import "std"
fn main() {
    std.log("saindo")
    std.process.exit(3)
    std.log("inalcancavel")
}
"#,
    )
    .unwrap();

    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&file)
        .output()
        .expect("run steampunk");
    assert_eq!(output.status.code(), Some(3));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("saindo"), "stdout={stdout}");
    assert!(!stdout.contains("inalcancavel"), "stdout={stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_rwlock() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("rwlock.stk"))
        .output()
        .expect("run");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("read=10") && stdout.contains("after=11"), "stdout={stdout}");
}

#[test]
fn run_parallel_map() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("parallel_map.stk"))
        .output()
        .expect("run");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("len=3") && stdout.contains("first=1"), "stdout={stdout}");
}

#[test]
fn run_struct_point() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("struct_point.stk"))
        .output()
        .expect("run");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("point=3,4"), "stdout={stdout}");
}

#[test]
fn run_task_cancel() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("task_cancel.stk"))
        .output()
        .expect("run");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("task ok"), "stdout={stdout}");
}

#[test]
fn run_http_get_err() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("http_get.stk"))
        .output()
        .expect("run");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("http err ok"), "stdout={stdout}");
}

#[test]
fn run_universal_types() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("universal_types.stk"))
        .output()
        .expect("run");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Ada")
            && stdout.contains("id=42")
            && stdout.contains("future=Ada")
            && stdout.contains("opt=Ada")
            && stdout.contains("res=Ada")
            && stdout.contains("ch=Ada")
            && stdout.contains("mu=Ada"),
        "stdout={stdout}"
    );
}

#[test]
fn run_serde_user() {
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(examples_dir().join("serde_user.stk"))
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("user_name")
            && stdout.contains("json ok")
            && stdout.contains("yaml ok")
            && stdout.contains("toml ok")
            && stdout.contains("toon ok")
            && stdout.contains("invalid json err ok")
            && stdout.contains("serde ok")
            && !stdout.contains("secret"),
        "stdout={stdout}"
    );
}

#[test]
fn build_and_execute_serde_user() {
    let out = std::env::temp_dir().join(format!("stk-serde-{}", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(steampunk_bin())
        .args(["build"])
        .arg(examples_dir().join("serde_user.stk"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build steampunk");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&out).output().expect("exec binary");
    assert!(run.status.success(), "stderr={}", String::from_utf8_lossy(&run.stderr));
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("serde ok"), "stdout={stdout}");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn serde_rejects_decorator_on_method() {
    let dir = std::env::temp_dir().join(format!("stk-serde-neg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad.stk");
    std::fs::write(
        &path,
        r#"
@import "std"
struct S {
    pub var x int
    @ignore
    pub fn new() S { return self }
}
fn main() {}
"#,
    )
    .unwrap();
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&path)
        .output()
        .expect("run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("decorators are not allowed on methods"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn serde_rejects_non_serializable_field() {
    let dir = std::env::temp_dir().join(format!("stk-serde-fut-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad.stk");
    std::fs::write(
        &path,
        r#"
@import "std"
struct S {
    pub var f Future<int>
    pub fn new() S { return self }
}
fn main() {
    var s = new S()
    std.log("$1", std.json.encode(s))
}
"#,
    )
    .unwrap();
    let output = Command::new(steampunk_bin())
        .args(["run"])
        .arg(&path)
        .output()
        .expect("run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not serializable") || stderr.contains("serialize"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stkm_unit_parses() {
    let dir = std::env::temp_dir().join(format!("stk-deps-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        dir.join("manager.stkm"),
        r#"
name = "T"
version = "0.1.0"
dependencies
    .use("demo", version = "^0.1.0")
"#,
    )
    .unwrap();
    let output = Command::new(steampunk_bin())
        .args(["deps", "--dir"])
        .arg(&dir)
        .env("STEAMPUNK_HOME", &home)
        .output()
        .expect("deps");
    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let _ = std::fs::remove_dir_all(&dir);
}
