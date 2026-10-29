import assert from "node:assert/strict"
import { test } from "node:test"

import { copy, resolveLang, translate, type CopyKey } from "../src/lib/i18n.ts"

const languages = ["zh", "en"] as const

test("both languages expose exactly the same keys", () => {
  assert.deepEqual(Object.keys(copy.zh).sort(), Object.keys(copy.en).sort())
})

test("every entry is a non-empty flat string", () => {
  for (const language of languages) {
    for (const [key, value] of Object.entries(copy[language])) {
      assert.equal(typeof value, "string", `${language}.${key} 必须是字符串`)
      assert.ok((value as string).trim().length > 0, `${language}.${key} 不能为空`)
    }
  }
})

test("placeholders match across languages", () => {
  const placeholders = (text: string) =>
    [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort()
  for (const key of Object.keys(copy.zh) as CopyKey[]) {
    assert.deepEqual(placeholders(copy.zh[key]), placeholders(copy.en[key]), key)
  }
})

test("translate interpolates and keeps unknown variables visible", () => {
  assert.equal(translate("en", "projectOpenCount", { n: 3 }), "3 open")
  assert.equal(translate("zh", "threadTokens", { n: 1200 }), "1200 tokens")
  assert.equal(translate("zh", "timeMinutesAgo"), "{n} 分钟前")
})

test("the browser language decides the copy, defaulting to Chinese", () => {
  assert.equal(resolveLang(undefined), "zh")
  assert.equal(resolveLang(""), "zh")
  assert.equal(resolveLang("zh-CN"), "zh")
  assert.equal(resolveLang("en-US"), "en")
  assert.equal(resolveLang("fr-FR"), "zh")
})
