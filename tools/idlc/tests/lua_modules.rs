// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `generate --lang lua` for types inside an IDL module: the module
//! tables are declared before the first type that sits in them, and the
//! file loads and round-trips a sample through `runtime/lua`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::Path;
use std::process::Command;

const IDL: &str = "module Robot {\n    @appendable\n    struct Pose {\n        @key string<32> robot_id;\n        double x;\n        double y;\n        double theta;\n    };\n};\n";

fn generated(tag: &str) -> (std::path::PathBuf, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Robot.idl"), IDL).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args(["generate", "--lang", "lua", "--out"])
        .arg(dir.join("out"))
        .arg(dir.join("Robot.idl"))
        .status()
        .unwrap();
    assert!(status.success());
    let text = std::fs::read_to_string(dir.join("out/Robot.lua")).unwrap();
    (dir, text)
}

#[test]
fn a_module_table_is_declared_before_the_types_in_it() {
    let (_, text) = generated("lua-modules-order");
    let table = text
        .find("M.Robot = {}")
        .unwrap_or_else(|| panic!("{text}"));
    let first_type = text
        .find("M.Robot.Pose =")
        .unwrap_or_else(|| panic!("{text}"));
    assert!(table < first_type, "{text}");
}

#[test]
fn the_generated_file_loads_and_round_trips_a_sample() {
    if !Command::new("lua")
        .arg("-v")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("SKIP: no `lua` on PATH; the declaration order is still checked above");
        return;
    }
    let (dir, _) = generated("lua-modules-run");
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/lua");
    let lua_path = format!(
        "{r}/?.lua;{r}/?/init.lua;{o}/?.lua",
        r = runtime.display(),
        o = dir.join("out").display()
    );
    let script = r#"
local ZD = require("Robot")
local zdw = require("zerodds_wire")
local w = zdw.writer_new({ endianness = "le", max_align = 4 })
ZD.zd_enc_Robot_Pose(w, { robot_id = "r2d2", x = 1.5, y = -2.5, theta = 0.75 })
local bytes = zdw.finish(w)
local r = zdw.reader_new(bytes, "le", 4)
local v = ZD.zd_dec_Robot_Pose(r, "discard")
assert(v.robot_id == "r2d2" and v.x == 1.5 and v.y == -2.5 and v.theta == 0.75)
print(#bytes)
"#;
    let output = Command::new("lua")
        .env("LUA_PATH", lua_path)
        .arg("-e")
        .arg(script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "40");
}
