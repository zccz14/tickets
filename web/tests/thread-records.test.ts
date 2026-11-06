import assert from "node:assert/strict"
import { test } from "node:test"

import {
  callSummary,
  generatedImageSource,
  isInternalInput,
  mergeRecords,
  outputSummary,
  pendingRecords,
  threadText,
  visibleRecords,
  type ThreadRecord,
} from "../src/lib/thread-records.ts"

function record(partial: Partial<ThreadRecord> & { id: number }): ThreadRecord {
  return {
    thread_id: "t1",
    kind: "input",
    payload: null,
    created_at: 1,
    ...partial,
  }
}

test("thread text reads input and output content parts", () => {
  assert.equal(threadText({ content: "直接写的" }), "直接写的")
  assert.equal(
    threadText({
      type: "message",
      content: [
        { type: "output_text", text: "第一段" },
        { type: "output_text", text: "第二段" },
      ],
    }),
    "第一段第二段",
  )
  assert.equal(threadText({ type: "reasoning", summary: [{ text: "想一想" }] }), "想一想")
})

test("tickets' own prompts stay out of the conversation", () => {
  const briefing = record({
    id: 1,
    payload: { content: "<!-- tickets-briefing -->\n你是工单 #1 的「现场调查 Agent」" },
  })
  const legacyBriefing = record({
    id: 2,
    payload: { content: "你是工单 #2 的「现场调查 Agent」，服务于报障人 alice。" },
  })
  const nudge = record({
    id: 3,
    payload: { content: "<!-- tickets-nudge -->\n## 工单 #3 有新评论" },
  })
  const legacyNudge = record({
    id: 4,
    payload: { content: "## 工单 #4 有新评论\n\nalice 写道" },
  })
  const human = record({ id: 5, payload: { content: "日志在 /tmp/x.log" } })
  const reply = record({ id: 6, kind: "response_output", payload: { content: [] } })

  for (const internal of [briefing, legacyBriefing, nudge, legacyNudge]) {
    assert.equal(isInternalInput(internal), true, `#${internal.id} 应被隐藏`)
  }
  assert.equal(isInternalInput(human), false)
  assert.equal(isInternalInput(reply), false)
  assert.deepEqual(
    visibleRecords([briefing, human, reply]).map((item) => item.id),
    [5, 6],
  )
})

test("incremental polls merge by id and keep order", () => {
  const merged = mergeRecords(
    [record({ id: 2 }), record({ id: 1 })],
    [record({ id: 2 }), record({ id: 3 })],
  )
  assert.deepEqual(
    merged.map((item) => item.id),
    [1, 2, 3],
  )
  // A duplicate id keeps the newer payload.
  const updated = mergeRecords([record({ id: 1, payload: { content: "旧" } })], [
    record({ id: 1, payload: { content: "新" } }),
  ])
  assert.equal(threadText(updated[0].payload), "新")
})

test("the in-flight reply is previewed until its record is durable", () => {
  const view = {
    audit_id: 1,
    input_record_id: 1,
    started_at: 100,
    status: "running",
    response: {
      completed: false,
      output: [
        { item: { type: "message", content: [{ type: "output_text", text: "正在查" }] }, done: false, record_id: null },
        { item: { type: "message", content: [{ type: "output_text", text: "已落库" }] }, done: true, record_id: 7 },
      ],
      safety_buffering: null,
    },
  }
  const pending = pendingRecords(view, new Set([7]), "t1")
  assert.equal(pending.length, 1)
  assert.equal(pending[0].id, -1)
  assert.equal(threadText(pending[0].payload), "正在查")
  assert.equal(pendingRecords({ ...view, status: "cancelled" }, new Set(), "t1").length, 0)
})

test("tool calls get one readable line", () => {
  const call = {
    name: "bash",
    raw: '{"worker_id":"w1","command":"journalctl -u gateway -n 50"}',
    args: { worker_id: "w1", command: "journalctl -u gateway -n 50" },
  }
  assert.equal(callSummary(call), "journalctl -u gateway -n 50")
  assert.equal(
    callSummary({ name: "normai_web_search", raw: "", args: { query: "HTTP 502 日志" } }),
    "HTTP 502 日志",
  )
})

test("tool output summaries collapse whitespace and stop at the limit", () => {
  assert.equal(outputSummary({ output: "line one\nline two" }), "line one line two")
  assert.equal(outputSummary({ type: "function_call_output" }), "function call output")
})

test("generated images become data URLs", () => {
  assert.equal(
    generatedImageSource({ type: "image_generation_call", result: "AAAA", output_format: "webp" }),
    "data:image/webp;base64,AAAA",
  )
  assert.equal(generatedImageSource({ type: "message" }), null)
})
