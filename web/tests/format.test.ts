import assert from "node:assert/strict"
import { test } from "node:test"

import { relativeTime, shortId } from "../src/lib/format.ts"

const now = Math.floor(Date.now() / 1000)

test("relative time picks the largest sensible unit", () => {
  assert.equal(relativeTime("zh", now - 5), "刚刚")
  assert.equal(relativeTime("en", now - 5), "just now")
  assert.equal(relativeTime("zh", now - 120), "2 分钟前")
  assert.equal(relativeTime("en", now - 7200), "2h ago")
  assert.equal(relativeTime("zh", now - 2 * 86_400), "2 天前")
})

test("past a week it falls back to a date", () => {
  const old = relativeTime("zh", 0)
  assert.ok(!old.includes("天前"))
  assert.ok(old.length > 0)
})

test("short ids are cut, not summarised", () => {
  assert.equal(shortId("0123456789"), "01234567")
  assert.equal(shortId("abc", 2), "ab")
})
