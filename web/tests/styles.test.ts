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

/**
 * Markdown typography belongs to the shared shell: `@zccz14/ux` owns `.ux-markdown`,
 * and this app only decides the font size of the container it drops content into.
 * A local renderer or a local `prose-*` utility would drift away from every other
 * app in the family, so both stay fenced off.
 */
test("markdown renders through the shared component, not a local fork", () => {
  const page = readFileSync(path.join(import.meta.dirname, "../src/pages/ticket.tsx"), "utf8")
  assert.match(page, /import \{ Markdown \} from "@zccz14\/ux"/)
  assert.doesNotMatch(page, /@\/lib\/markdown/)
  assert.doesNotMatch(css, /@utility prose-ticket/)
})
