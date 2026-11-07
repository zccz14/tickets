import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import path from "node:path"
import { test } from "node:test"

/**
 * The embedded Thread View is a page by default: it claims the height under a
 * 3.5rem header. The ticket page is a card in a scrolling document, so it has
 * to give the view a box — a definite height on the card and `min-h-0` on the
 * view — or the transcript grows past the card and the composer disappears
 * below the fold.
 */
const panel = readFileSync(
  path.join(import.meta.dirname, "../src/components/thread-live.tsx"),
  "utf8",
)

test("the panel gives the embedded view a box instead of a page", () => {
  assert.match(panel, /<section className="[^"]*\bh-\[/)
  assert.match(panel, /<CybionThreadView[\s\S]*?className="min-h-0 flex-1"/)
})
