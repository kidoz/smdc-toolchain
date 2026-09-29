//! End-to-end emulator tests: compile C programs to ROMs, run them in
//! BlastEm (headless via SDL's offscreen driver), and assert on values the
//! program wrote to work RAM.
//!
//! Test programs call `set_result(index, value)` to publish 16-bit results
//! at RESULT_BASE and `test_done()` when finished. The harness sets a
//! debugger breakpoint on `test_done` (address taken from the `.sym` map),
//! lets the ROM run, then reads the result words back through BlastEm's
//! debugger (`p/x [$addr]`).
//!
//! Requires `blastem` and `script` (util-linux, provides the pty BlastEm's
//! debugger needs) on PATH; each test skips itself when either is missing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const RESULT_BASE: u32 = 0x00FF0100;

const PRELUDE: &str = "void test_done(void) { for (;;) {} }\n\
void set_result(int index, int value) {\n\
    volatile unsigned short *base = (volatile unsigned short *)0x00FF0100;\n\
    base[index] = (unsigned short)value;\n\
}\n";

fn emulator_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        // PATH lookup only: `blastem -v` prints the version but never exits
        // (0.6.3-pre), so probing by running it would hang.
        let on_path = |cmd: &str| {
            std::env::var_os("PATH")
                .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(cmd).is_file()))
        };
        on_path("blastem") && on_path("script")
    })
}

macro_rules! require_emulator {
    () => {
        if !emulator_available() {
            eprintln!("skipping: blastem (or script) not on PATH");
            return;
        }
    };
}

fn compile_rom(dir: &Path, body: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let c_path = dir.join("test.c");
    let rom_path = dir.join("test.bin");
    std::fs::write(&c_path, format!("{PRELUDE}\n{body}")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_smdc"))
        .arg(&c_path)
        .args(["-t", "rom", "-g", "-o"])
        .arg(&rom_path)
        .output()
        .expect("failed to run smdc");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    rom_path
}

/// Look up a label address in the WLADX-format `.sym` map (`00:000304 name`).
fn symbol_address(sym_path: &Path, name: &str) -> u32 {
    let text = std::fs::read_to_string(sym_path).unwrap();
    for line in text.lines() {
        if let Some((addr, sym)) = line.trim().split_once(' ')
            && sym == name
        {
            let addr = addr.split_once(':').map_or(addr, |(_, a)| a);
            return u32::from_str_radix(addr, 16).unwrap();
        }
    }
    panic!("symbol {name} not found in {}", sym_path.display());
}

/// Run the ROM until `test_done` and read `count` result words from RAM.
fn run_rom(rom: &Path, count: usize) -> Vec<u16> {
    let done = symbol_address(&rom.with_extension("sym"), "test_done");

    use std::fmt::Write as _;
    let mut commands = format!("b 0x{done:X}\nc\n");
    for i in 0..count {
        let _ = writeln!(commands, "p/x [${:X}]", RESULT_BASE + 2 * i as u32);
    }
    commands.push_str("q\n");

    // BlastEm's x86-64 JIT occasionally fails to place its code buffer
    // within reach of its own binary (address-space randomization) and
    // hangs; that is independent of the ROM, so retry those launches
    const JIT_PLACEMENT_FAILURE: &str = "out of range for a 32-bit displacement";
    let mut stdout = launch_blastem(rom, &commands);
    for _ in 0..2 {
        if !stdout.contains(JIT_PLACEMENT_FAILURE) {
            break;
        }
        stdout = launch_blastem(rom, &commands);
    }

    assert!(
        stdout.contains("Breakpoint 0 hit"),
        "ROM never reached test_done; blastem output:\n{stdout}"
    );

    (0..count)
        .map(|i| {
            let needle = format!("[${:X}]: ", RESULT_BASE + 2 * i as u32);
            let value = stdout
                .lines()
                .find_map(|line| {
                    let (_, rest) = line.split_once(&needle)?;
                    Some(rest.trim())
                })
                .unwrap_or_else(|| panic!("no value for {needle} in output:\n{stdout}"));
            u16::from_str_radix(value, 16)
                .unwrap_or_else(|_| panic!("bad hex {value:?} for {needle}"))
        })
        .collect()
}

/// Run BlastEm's debugger on `rom`, feed it `commands`, and return its output
fn launch_blastem(rom: &Path, commands: &str) -> String {
    // BlastEm's debugger insists on a tty, so run it under `script`; `timeout`
    // guards against a ROM that never reaches the breakpoint.
    let mut child = Command::new("timeout")
        .args(["-k", "2", "30", "script", "-qec"])
        .arg(format!("blastem -d -g '{}'", rom.display()))
        .arg("/dev/null")
        .env("SDL_VIDEODRIVER", "offscreen")
        .env("SDL_AUDIODRIVER", "dummy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn blastem");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(commands.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn assert_rom_results(test: &str, body: &str, expected: &[u16]) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("emulator")
        .join(test);
    let rom = compile_rom(&dir, body);
    let words = run_rom(&rom, expected.len());
    assert_eq!(words, expected);
}

#[test]
fn emulator_arithmetic() {
    require_emulator!();
    assert_rom_results(
        "arithmetic",
        "void main(void) {\n\
             int a = 21;\n\
             int b = 4;\n\
             set_result(0, a + b);\n\
             set_result(1, a - b);\n\
             set_result(2, a * b);\n\
             set_result(3, a / b);\n\
             set_result(4, a % b);\n\
             set_result(5, a << 2);\n\
             set_result(6, a >> 1);\n\
             set_result(7, (a > b) + (a == 21));\n\
             test_done();\n\
         }\n",
        &[25, 17, 84, 5, 1, 84, 10, 2],
    );
}

#[test]
fn emulator_control_flow() {
    require_emulator!();
    assert_rom_results(
        "control_flow",
        "void main(void) {\n\
             int sum = 0;\n\
             int n = 0;\n\
             int x = 3;\n\
             int i;\n\
             for (i = 1; i <= 10; i = i + 1) { sum = sum + i; }\n\
             while (n < 100) { n = n + 7; }\n\
             switch (x) {\n\
                 case 1: x = 10; break;\n\
                 case 3: x = 30; break;\n\
                 default: x = 99; break;\n\
             }\n\
             set_result(0, sum);\n\
             set_result(1, n);\n\
             set_result(2, x);\n\
             test_done();\n\
         }\n",
        &[55, 105, 30],
    );
}

#[test]
fn emulator_function_calls_and_recursion() {
    require_emulator!();
    assert_rom_results(
        "functions",
        "int fib(int n) { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }\n\
         int fact(int n) { if (n <= 1) { return 1; } return n * fact(n - 1); }\n\
         void main(void) {\n\
             set_result(0, fib(10));\n\
             set_result(1, fact(5));\n\
             test_done();\n\
         }\n",
        &[55, 120],
    );
}

#[test]
fn emulator_local_initializers() {
    require_emulator!();
    assert_rom_results(
        "local_init",
        "void main(void) {\n\
             int a[4] = {10, 20, 30, 40};\n\
             char s[4] = \"AB\";\n\
             int sum = 0;\n\
             int i;\n\
             for (i = 0; i < 4; i = i + 1) { sum = sum + a[i]; }\n\
             set_result(0, sum);\n\
             set_result(1, s[0]);\n\
             set_result(2, s[1]);\n\
             set_result(3, s[2] + s[3]);\n\
             test_done();\n\
         }\n",
        &[100, 65, 66, 0],
    );
}

#[test]
fn emulator_structs_and_pointers() {
    require_emulator!();
    assert_rom_results(
        "structs",
        "struct Point { int x; int y; };\n\
         void swap(int *a, int *b) { int t = *a; *a = *b; *b = t; }\n\
         void main(void) {\n\
             struct Point p = {3, 4};\n\
             struct Point *pp = &p;\n\
             int u = 1;\n\
             int v = 2;\n\
             swap(&u, &v);\n\
             set_result(0, pp->x + pp->y);\n\
             set_result(1, u * 10 + v);\n\
             test_done();\n\
         }\n",
        &[7, 21],
    );
}

#[test]
fn emulator_function_pointers() {
    require_emulator!();
    assert_rom_results(
        "function_pointers",
        "int add(int a, int b) { return a + b; }\n\
         int sub(int a, int b) { return a - b; }\n\
         int apply(int (*f)(int, int), int a, int b) { return f(a, b); }\n\
         void main(void) {\n\
             int (*op)(int, int) = add;\n\
             int (*ops[2])(int, int);\n\
             ops[0] = add;\n\
             ops[1] = sub;\n\
             set_result(0, op(30, 12));\n\
             op = sub;\n\
             set_result(1, op(30, 12));\n\
             set_result(2, (*op)(50, 8));\n\
             set_result(3, apply(add, 2, 3) + apply(sub, 10, 4));\n\
             set_result(4, ops[1](9, 4));\n\
             test_done();\n\
         }\n",
        &[42, 18, 42, 11, 5],
    );
}

#[test]
fn emulator_global_data() {
    require_emulator!();
    assert_rom_results(
        "globals",
        "int table[5] = {2, 4, 6, 8, 10};\n\
         int counter = 100;\n\
         void main(void) {\n\
             int sum = 0;\n\
             int i;\n\
             for (i = 0; i < 5; i = i + 1) { sum = sum + table[i]; }\n\
             counter = counter + 1;\n\
             set_result(0, sum);\n\
             set_result(1, counter);\n\
             test_done();\n\
         }\n",
        &[30, 101],
    );
}

/// Register-cache boundary stress: values computed inside both arms of an
/// if/else (the else arm falls through into the join label with no
/// terminator) must be correct when read after the join, for both branch
/// orders, and across loop back-edges.
#[test]
fn emulator_register_cache_boundaries() {
    require_emulator!();
    assert_rom_results(
        "regcache",
        "void main(void) {\n\
             int a = 11;\n\
             int b = 3;\n\
             int r = 0;\n\
             int i;\n\
             for (i = 0; i < 6; i = i + 1) {\n\
                 if (i & 1) { r = r + a; } else { r = r - b; }\n\
             }\n\
             if (r > 0) { r = r + 100; } else { r = r - 100; }\n\
             set_result(0, r);\n\
             set_result(1, a * 2 + b);\n\
             set_result(2, (r & 1) + ((a - b) & 1));\n\
             test_done();\n\
         }\n",
        &[100 + 3 * 11 - 3 * 3, 25, 0],
    );
}

/// High and low result words of a 32-bit value, matching a program that
/// publishes `set_result(i, v >> 16); set_result(i + 1, v);`
fn words(value: u32) -> [u16; 2] {
    [(value >> 16) as u16, value as u16]
}

/// Stores to char and short members write only their own bytes: a
/// longword store would clobber the next member, and at an odd offset
/// raise an address error.
#[test]
fn emulator_struct_member_widths() {
    require_emulator!();
    assert_rom_results(
        "member_widths",
        "struct S { short a; short b; int c; char d; char e; short f; };\n\
         struct S g;\n\
         void main(void) {\n\
             struct S l;\n\
             struct S *p = &l;\n\
             g.a = 1; g.b = 0x1234; g.c = 7; g.d = 3; g.e = 4; g.f = 9;\n\
             l.a = 11; l.b = 12; l.c = 13; l.d = 14; l.e = 15; l.f = 16;\n\
             p->e = 25; p->b += 1;\n\
             set_result(0, g.a); set_result(1, g.b); set_result(2, g.c);\n\
             set_result(3, g.d); set_result(4, g.e); set_result(5, g.f);\n\
             set_result(6, l.a); set_result(7, l.b); set_result(8, l.d);\n\
             set_result(9, l.e); set_result(10, l.f);\n\
             test_done();\n\
         }\n",
        &[1, 0x1234, 7, 3, 4, 9, 11, 13, 14, 25, 16],
    );
}

/// 32-bit multiply, divide and modulo (the 68000's MULS/DIVS are only
/// 16-bit), covering every path of the division helper and C's
/// truncating signed division.
#[test]
fn emulator_32bit_multiply_divide() {
    require_emulator!();
    let mut expected = Vec::new();
    for value in [
        300_000u32,           // 100000 * 3
        (-300_000i32) as u32, // -100000 * 3
        0x3661_76F8,          // 0x12345678 * 0x9ABCDEF1 (mod 2^32)
        142_857,              // 1000000 / 7 (16-bit divisor)
        1,                    // 1000000 % 7
        0x0000_FFFF,          // 0xFFFFFFFF / 0x10001 (loop path)
        0,                    // 0xFFFFFFFF % 0x10001
        1,                    // 0xFFFFFFFF / 0x80000000 (divisor >= 2^31)
        0x7FFF_FFFF,          // 0xFFFFFFFF % 0x80000000
        (-142_857i32) as u32, // -1000000 / 7
        (-1i32) as u32,       // -1000000 % 7
        14,                   // 1000000 / 70000
        300,                  // 3000 / 10 * 1 via multiply by 1
    ] {
        expected.extend(words(value));
    }
    assert_rom_results(
        "mul_div",
        "int s(int x) { return x; }\n\
         unsigned int u(unsigned int x) { return x; }\n\
         void publish(int i, unsigned int v) { set_result(i, v >> 16); set_result(i + 1, v); }\n\
         void main(void) {\n\
             publish(0, s(100000) * s(3));\n\
             publish(2, s(-100000) * 3);\n\
             publish(4, s(0x12345678) * s(0x9ABCDEF1));\n\
             publish(6, s(1000000) / s(7));\n\
             publish(8, s(1000000) % s(7));\n\
             publish(10, u(0xFFFFFFFF) / u(0x10001));\n\
             publish(12, u(0xFFFFFFFF) % u(0x10001));\n\
             publish(14, u(0xFFFFFFFF) / u(0x80000000));\n\
             publish(16, u(0xFFFFFFFF) % u(0x80000000));\n\
             publish(18, s(-1000000) / s(7));\n\
             publish(20, s(-1000000) % s(7));\n\
             publish(22, s(1000000) / s(70000));\n\
             publish(24, s(3000) / 10 * 1);\n\
             test_done();\n\
         }\n",
        &expected,
    );
}
