/**
 * Reading a Cybion Thread from inside a Ticket.
 *
 * The live endpoint hands over the same records the Cybion conversation uses,
 * but a ticket page only needs a readable subset: what the reporter said, what
 * the Agent answered, and a one-line trace of the tools in between. The two
 * marker checks below keep Tickets' own briefing and comment nudges out of the
 * human conversation — they are prompts, not messages.
 */

export type ThreadRecord = {
  id: number
  thread_id: string
  kind: "input" | "response_output" | "tool_output" | "checkpoint" | "activity" | string
  payload: unknown
  created_at: number
  screenshot?: boolean
}

export type ThreadResponseOutput = {
  item: unknown
  done: boolean
  record_id: number | null
}

export type ThreadResponse = {
  audit_id: number
  input_record_id: number | null
  started_at: number
  status: string
  response: {
    completed: boolean
    output: ThreadResponseOutput[]
    safety_buffering: unknown
  }
}

const BRIEFING_MARKER = "<!-- tickets-briefing -->"
const NUDGE_MARKER = "<!-- tickets-nudge -->"

export function payloadObject(payload: unknown): Record<string, unknown> | null {
  return payload && typeof payload === "object" && !Array.isArray(payload)
    ? (payload as Record<string, unknown>)
    : null
}

function textParts(value: unknown): string {
  if (!Array.isArray(value)) return ""
  return value
    .flatMap((part) => {
      const object = payloadObject(part)
      if (typeof object?.text === "string") return [object.text]
      if (typeof object?.refusal === "string") return [object.refusal]
      return []
    })
    .join("")
}

/** The text a record carries, whether it is an input, a reply or a tool result. */
export function threadText(payload: unknown): string {
  const object = payloadObject(payload)
  if (typeof object?.content === "string") return object.content
  const parts = textParts(object?.content)
  if (parts) return parts
  if (object?.type === "reasoning") {
    const summary = Array.isArray(object.summary)
      ? object.summary
          .flatMap((part) => {
            const summaryPart = payloadObject(part)
            return typeof summaryPart?.text === "string" ? [summaryPart.text] : []
          })
          .join("\n\n")
      : ""
    if (summary) return summary
  }
  if (typeof object?.output === "string") return object.output
  return ""
}

/** Tickets' own prompts are bookkeeping: briefings and comment nudges. */
export function isInternalInput(record: ThreadRecord): boolean {
  if (record.kind !== "input") return false
  const text = threadText(record.payload)
  return (
    text.includes(BRIEFING_MARKER) ||
    text.includes(NUDGE_MARKER) ||
    // Threads created before the markers were added.
    (text.startsWith("你是工单 #") && text.includes("现场调查 Agent")) ||
    (text.startsWith("## 工单 #") && text.includes("有新评论"))
  )
}

export function visibleRecords(records: readonly ThreadRecord[]): ThreadRecord[] {
  return records.filter((record) => !isInternalInput(record))
}

/** Merge incremental polls without reordering or duplicating records. */
export function mergeRecords(
  existing: readonly ThreadRecord[],
  incoming: readonly ThreadRecord[],
): ThreadRecord[] {
  if (incoming.length === 0) return existing as ThreadRecord[]
  const byId = new Map<number, ThreadRecord>()
  for (const record of existing) byId.set(record.id, record)
  for (const record of incoming) byId.set(record.id, record)
  return [...byId.values()].sort((left, right) => left.id - right.id)
}

/** The reply still being produced, shaped like durable records. */
export function pendingRecords(
  view: ThreadResponse | null | undefined,
  existingIds: ReadonlySet<number>,
  threadId: string,
): ThreadRecord[] {
  if (!view || view.status === "cancelled") return []
  return view.response.output.flatMap((output, index) => {
    if (output.record_id !== null && existingIds.has(output.record_id)) return []
    return [
      {
        id: output.record_id ?? -(index + 1),
        thread_id: threadId,
        kind: "response_output",
        payload: output.item,
        created_at: view.started_at,
      },
    ]
  })
}

export type FunctionCall = {
  name: string
  args: Record<string, unknown> | null
  raw: string
}

export function functionCallOf(payload: unknown): FunctionCall | null {
  const object = payloadObject(payload)
  if (object?.type !== "function_call" || typeof object.name !== "string") return null
  const raw = typeof object.arguments === "string" ? object.arguments : ""
  let args: Record<string, unknown> | null = null
  if (raw) {
    try {
      const parsed: unknown = JSON.parse(raw)
      const value = payloadObject(parsed)
      if (value) args = value
    } catch {
      // A malformed argument blob is still worth showing raw.
    }
  }
  return { name: object.name, args, raw }
}

function firstString(...values: unknown[]): string | null {
  for (const value of values) {
    if (typeof value === "string" && value.trim()) return value.trim()
  }
  return null
}

/** One readable line for a tool call: the field that says what it is doing. */
export function callSummary(call: FunctionCall): string {
  const args = call.args ?? {}
  const summary =
    firstString(
      args.command,
      args.query,
      args.url,
      args.path,
      args.manual_id,
      args.prompt,
      args.conversation_id,
      args.worker_id,
    ) ?? call.raw
  return clip(summary, 140)
}

export function outputSummary(payload: unknown): string {
  const text = threadText(payload).replace(/\s+/g, " ").trim()
  if (text) return clip(text, 160)
  const object = payloadObject(payload)
  if (typeof object?.type === "string") return object.type.replaceAll("_", " ")
  return ""
}

export function generatedImageSource(payload: unknown): string | null {
  const object = payloadObject(payload)
  if (object?.type !== "image_generation_call" || typeof object.result !== "string") return null
  const format =
    typeof object.output_format === "string" ? object.output_format : "png"
  if (format !== "png" && format !== "jpeg" && format !== "webp") return null
  return `data:image/${format};base64,${object.result}`
}

export function clip(text: string, limit: number): string {
  const flattened = text.trim()
  return flattened.length > limit ? `${flattened.slice(0, limit)}…` : flattened
}
