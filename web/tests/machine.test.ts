import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { after, test } from "node:test"

import { resolverCommand, shellValue } from "../src/lib/machine.ts"

/**
 * The check command is a product surface: it is pasted into an Ultimate Machine
 * and its stdout is what wakes the resolver Thread. These tests execute the
 * generated bash against stub `curl` and `jq` binaries, so they check the
 * contract (silent on an empty queue, non-zero with the queue on work) rather
 * than the text of the script.
 */

const sandboxes: string[] = []

after(() => {
  for (const dir of sandboxes) rmSync(dir, { recursive: true, force: true })
})

function prepare(command: string) {
  const root = mkdtempSync(path.join(tmpdir(), "tickets-machine-"))
  sandboxes.push(root)
  const bin = path.join(root, "bin")
  const home = path.join(root, "home")
  mkdirSync(bin, { recursive: true })
  mkdirSync(home, { recursive: true })
  writeFileSync(
    path.join(bin, "curl"),
    '#!/usr/bin/env bash\ncat >/dev/null 2>&1 || true\nif [ -n "${STUB_CURL_FAIL:-}" ]; then exit 7; fi\nexit 0\n',
  )
  writeFileSync(
    path.join(bin, "jq"),
    '#!/usr/bin/env bash\ncat >/dev/null\necho "${STUB_PENDING:-0}"\n',
  )
  for (const name of ["curl", "jq"]) chmodSync(path.join(bin, name), 0o755)
  const script = path.join(root, "check.sh")
  writeFileSync(script, command)
  return { root, bin, home, script }
}

function run(command: string, pending: number) {
  const box = prepare(command)
  const result = spawnSync("bash", [box.script], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${box.bin}:/usr/bin:/bin`,
      HOME: box.home,
      STUB_PENDING: String(pending),
    },
  })
  return { ...box, ...result }
}

const KEY = `tk_${"a".repeat(48)}`

test("an empty queue exits 0 and prints nothing", () => {
  const result = run(resolverCommand("https://tickets.test", KEY, "支付网关"), 0)
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), "", "终极机器只在退出非 0 时唤醒 Thread")
})

test("pending work exits 1 with the queue summary", () => {
  const result = run(resolverCommand("https://tickets.test", KEY, "支付网关"), 1)
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stdout, /等待处理/)
  assert.match(result.stdout, /ctl show/)
})

test("the key stays in a 0600 file and never in the output", () => {
  const result = run(resolverCommand("https://tickets.test", KEY, "支付网关"), 1)
  assert.ok(!result.stdout.includes(KEY), "检查命令的输出不应包含密钥")
  const keyFile = path.join(result.home, ".tickets/key")
  assert.equal(readFileSync(keyFile, "utf8"), KEY)
  assert.equal(statSync(keyFile).mode & 0o777, 0o600)
  const ctl = path.join(result.home, ".tickets/ctl")
  assert.ok(statSync(ctl).mode & 0o111, "ctl 应当可执行")
  assert.match(readFileSync(ctl, "utf8"), /queue\)/)
})

test("a project name cannot break out of the command", () => {
  const name = "他说'你好' $(whoami) `id`"
  const result = run(resolverCommand("https://tickets.test", KEY, name), 1)
  assert.equal(result.status, 1, result.stderr)
  assert.ok(result.stdout.includes(name), "项目名应原样输出，而不是被 shell 解释")
})

test("an unreachable tickets service exits 2 with a readable reason", () => {
  const box = prepare(resolverCommand("https://tickets.test", KEY, "支付网关"))
  const result = spawnSync("bash", [box.script], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${box.bin}:/usr/bin:/bin`,
      HOME: box.home,
      STUB_CURL_FAIL: "1",
      STUB_PENDING: "1",
    },
  })
  assert.equal(result.status, 2)
  assert.match(result.stdout, /服务不可达/)
})

test("a worker without jq is told to install it", () => {
  const box = prepare(resolverCommand("https://tickets.test", KEY, "支付网关"))
  const result = spawnSync("/bin/bash", [box.script], {
    encoding: "utf8",
    env: { ...process.env, PATH: "/nonexistent", HOME: box.home },
  })
  assert.equal(result.status, 2)
  assert.match(result.stdout, /缺少 jq/)
})

test("shellValue quotes a single quote the bash way", () => {
  assert.equal(shellValue("plain"), "'plain'")
  assert.equal(shellValue("a'b"), "'a'\\''b'")
})
