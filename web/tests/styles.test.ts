import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import path from "node:path"
import { test } from "node:test"

/**
 * `@zccz14/ux` documents two stylesheet imports: its own shell and the Linkit
 * components it embeds in the header. The Linkit file is not optional — without
 * it `.linkit-my-info` has no rules and the account avatar renders at its
 * natural size, which is how this regression reached production once.
 */
const css = readFileSync(path.join(import.meta.dirname, "../src/index.css"), "utf8")

test("the app imports both the shell and the Linkit component styles", () => {
  assert.match(css, /@import\s+"@zccz14\/ux\/styles\.css";/)
  assert.match(css, /@import\s+"linkit-react-components\/styles\.css";/)
})
