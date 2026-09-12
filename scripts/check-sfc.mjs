#!/usr/bin/env node
// 沙箱 SFC 校验：用 @vue/compiler-sfc 对 src 下每个 .vue parse + compileScript + compileTemplate
// 用法：node scripts/check-sfc.mjs （只编译不打包，适合无 vite/cargo 的沙箱）
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { parse, compileScript, compileTemplate } = require("@vue/compiler-sfc");
const scriptDir = path.dirname(fileURLToPath(import.meta.url));

function walk(dir) {
  const out = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...walk(p));
    else if (e.name.endsWith(".vue")) out.push(p);
  }
  return out;
}

const files = walk(path.resolve(import.meta.dirname, "../src"));
let failed = 0;
for (const f of files) {
  const { descriptor, errors } = parse(readFileSync(f, "utf8"), { filename: f });
  const errs = [...errors];
  if (!errs.length && descriptor.scriptSetup) {
    try {
      compileScript(descriptor, { id: f });
    } catch (e) {
      errs.push(e);
    }
  }
  if (!errs.length && descriptor.template) {
    const r = compileTemplate({ source: descriptor.template.content, filename: f, id: f });
    if (r.errors && r.errors.length) errs.push(...r.errors);
  }
  if (errs.length) {
    failed += 1;
    console.log("FAIL", f);
    for (const e of errs) console.log("   ", String(e && (e.message || e)));
  } else {
    console.log("OK  ", f);
  }
}
console.log(failed ? `FAILED=${failed}` : "ALL SFC COMPILE OK");
process.exit(failed ? 1 : 0);
