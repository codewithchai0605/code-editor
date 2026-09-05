#!/usr/bin/env node
// Run with: npm run doctor
//
// Tauri needs a real Rust toolchain on top of Node — if it's missing, `tauri
// dev`'s failure mode is a wall of cargo/linker errors that don't say "go
// install Rust". This just checks the obvious things up front.

import { execSync } from "node:child_process";

function has(cmd) {
  try {
    execSync(`${cmd} --version`, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

const checks = [
  { cmd: "cargo", label: "Rust toolchain (cargo)", hint: "install from https://rustup.rs" },
  { cmd: "rustc", label: "Rust compiler (rustc)", hint: "included with rustup" },
];

let allOk = true;
for (const { cmd, label, hint } of checks) {
  if (has(cmd)) {
    console.log(`\u2714 ${label}`);
  } else {
    allOk = false;
    console.log(`\u2717 ${label} not found \u2014 ${hint}`);
  }
}

if (process.platform === "win32" && allOk) {
  console.log("\u2714 Windows detected \u2014 make sure the \"Desktop development with C++\" Visual Studio workload is installed (required by the MSVC linker Tauri uses).");
}

if (!allOk) {
  console.log("\nMissing tools above. Install them, then re-run this check.");
  process.exit(1);
}

console.log("\nEnvironment looks good.");
