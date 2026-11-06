import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  BotIcon,
  ChevronDownIcon,
  CircleAlertIcon,
  Loader2Icon,
  SendIcon,
  SparklesIcon,
  TerminalSquareIcon,
} from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { Markdown } from "@zccz14/ux"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import type { ThreadInfo } from "@/lib/api"
import { relativeTime } from "@/lib/format"
import { useCopy, useSession } from "@/lib/session"
import {
  callSummary,
  clip,
  functionCallOf,
  generatedImageSource,
  mergeRecords,
  outputSummary,
  payloadObject,
  pendingRecords,
  threadText,
  visibleRecords,
  type ThreadRecord,
  type ThreadResponse,
} from "@/lib/thread-records"
import { cn } from "@/lib/utils"

type LiveThread = {
  thread: ThreadInfo
  records: ThreadRecord[]
  response: ThreadResponse | null
}

/**
 * The reporter's side of the investigation, in the ticket page.
 *
 * The Thread belongs to the reporter (it runs on their Worker), so this panel
 * only exists for them. Polling reads history from the newest record and the
 * in-flight reply separately: the conversation keeps moving while a turn is
 * being written, without re-transferring evidence the page already has.
 */
export function ThreadLivePanel({
  ticketId,
  thread,
  status,
  intro,
}: {
  ticketId: string
  thread: ThreadInfo
  status: "draft" | "open" | "closed"
  /// The message that started a draft: it is the first turn of the same
  /// conversation, so it belongs inside the panel, not above it.
  intro?: { body: string; createdAt: number }
}) {
  const { request } = useSession()
  const { t } = useCopy()
  const queryClient = useQueryClient()
  const [records, setRecords] = useState<ThreadRecord[]>([])
  const [draft, setDraft] = useState("")
  const cursor = useRef(0)
  const bottom = useRef<HTMLDivElement>(null)
  const knownThread = useRef<string | null>(thread.thread_id)

  const live = useQuery({
    queryKey: ["ticket-thread-live", ticketId],
    queryFn: async () => {
      const data = await request<LiveThread>(
        `/api/tickets/${ticketId}/thread/live?after=${cursor.current}`,
      )
      return data
    },
    refetchInterval: (query) =>
      query.state.data?.response?.status === "running" ? 800 : 2500,
    refetchIntervalInBackground: true,
  })

  const threadId = live.data?.thread.thread_id ?? thread.thread_id
  // Only a creation failure belongs here; the sidebar's probe error is its own.
  const threadError = live.data?.thread.error ?? null
  const running = live.data?.response?.status === "running"
  const starting = !threadId && !threadError && status !== "closed"

  useEffect(() => {
    const incoming = live.data?.records ?? []
    if (incoming.length === 0) return
    setRecords((current) => mergeRecords(current, incoming))
    cursor.current = incoming.reduce(
      (newest, record) => Math.max(newest, record.id),
      cursor.current,
    )
  }, [live.data])

  // A draft starts its Thread on the first read; the rest of the page still
  // shows the old "no Thread" state until its own queries refresh.
  useEffect(() => {
    const next = live.data?.thread.thread_id ?? null
    if (next === knownThread.current) return
    knownThread.current = next
    void queryClient.invalidateQueries({ queryKey: ["ticket", ticketId] })
    void queryClient.invalidateQueries({ queryKey: ["ticket-thread", ticketId] })
    void queryClient.invalidateQueries({ queryKey: ["tickets"] })
  }, [live.data?.thread.thread_id, queryClient, ticketId])

  const hasIntro = Boolean(intro?.body.trim())
  const pending = pendingRecords(
    live.data?.response,
    new Set(records.map((record) => record.id)),
    threadId ?? "",
  )
  const messages = visibleRecords([...records, ...pending])

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" })
  }, [messages.length, running])

  const send = useMutation({
    mutationFn: (body: string) =>
      request<{ accepted: boolean }>(`/api/tickets/${ticketId}/thread/live`, {
        method: "POST",
        body: JSON.stringify({ body }),
      }),
    onSuccess: () => {
      setDraft("")
      void live.refetch()
    },
  })
  const retry = useMutation({
    mutationFn: () =>
      request<unknown>(`/api/tickets/${ticketId}/thread`, {
        method: "POST",
        body: JSON.stringify({}),
      }),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: ["ticket", ticketId] })
      void queryClient.invalidateQueries({ queryKey: ["ticket-thread", ticketId] })
      void live.refetch()
    },
  })

  function submit() {
    const body = draft.trim()
    if (!body || send.isPending) return
    send.mutate(body)
  }

  const error = live.isError ? (live.error as Error).message : null

  return (
    <section className="overflow-hidden rounded-xl border border-border/70 bg-card">
      <header className="flex flex-wrap items-center gap-2 border-b border-border/60 px-4 py-3">
        <span className="flex size-6 items-center justify-center rounded-md bg-reporter/12 text-reporter">
          <BotIcon className="size-3.5" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium">{t("threadLiveTitle")}</p>
          <p className="text-xs text-muted-foreground">{t("threadLiveVisibility")}</p>
        </div>
        <span
          className={cn(
            "inline-flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[10px]",
            running
              ? "border-primary/25 bg-primary/8 text-primary"
              : "border-border bg-muted/50 text-muted-foreground",
          )}
        >
          {running ? <Loader2Icon className="size-2.5 animate-spin" /> : null}
          {starting
            ? t("threadLiveStarting")
            : running
              ? t("threadLiveRunning")
              : t("threadLiveIdle")}
        </span>
      </header>

      <div
        className={cn(
          "overflow-y-auto px-4 py-4",
          status === "draft" ? "max-h-[34rem]" : "max-h-[26rem]",
        )}
      >
        {starting ? (
          <div className="space-y-3">
            {hasIntro ? (
              <ul className="space-y-3">
                <IntroEntry intro={intro!} />
              </ul>
            ) : null}
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2Icon className="size-3.5 animate-spin" />
              {t("threadLiveStarting")}
            </p>
          </div>
        ) : messages.length === 0 && !hasIntro ? (
          <p className="rounded-lg border border-dashed border-border/70 px-3 py-6 text-center text-sm text-muted-foreground">
            {error ?? t("threadLiveEmpty")}
          </p>
        ) : (
          <ul className="space-y-3">
            {hasIntro ? <IntroEntry intro={intro!} /> : null}
            {messages.map((record) => (
              <ThreadEntry key={`${record.id}:${record.created_at}`} record={record} />
            ))}
          </ul>
        )}
        <div ref={bottom} />
      </div>

      {threadError ? (
        <div className="flex flex-wrap items-center gap-2 border-t border-border/60 px-4 py-3 text-xs text-destructive">
          <CircleAlertIcon className="size-3.5" />
          <span className="min-w-0 flex-1">{threadError}</span>
          <Button size="sm" variant="outline" disabled={retry.isPending} onClick={() => retry.mutate()}>
            {t("actionRetry")}
          </Button>
        </div>
      ) : null}
      {error && !threadError ? (
        <p className="border-t border-border/60 px-4 py-3 text-xs text-destructive">{error}</p>
      ) : null}

      {status !== "closed" ? (
        <form
          className="space-y-2 border-t border-border/60 p-3"
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <Textarea
            rows={2}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                event.preventDefault()
                submit()
              }
            }}
            placeholder={t("threadLivePlaceholder")}
            disabled={starting || Boolean(threadError)}
            className="resize-none border-0 bg-transparent p-1 shadow-none focus-visible:ring-0"
          />
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs text-muted-foreground">{t("threadLiveComposerHint")}</span>
            <Button
              type="submit"
              size="sm"
              disabled={!draft.trim() || send.isPending || starting || Boolean(threadError)}
            >
              {send.isPending ? <Loader2Icon className="animate-spin" /> : <SendIcon />}
              {t("actionSendToAgent")}
            </Button>
          </div>
          {send.isError ? (
            <p className="text-xs text-destructive">{(send.error as Error).message}</p>
          ) : null}
        </form>
      ) : (
        <p className="border-t border-border/60 px-4 py-3 text-xs text-muted-foreground">
          {t("threadLiveClosed")}
        </p>
      )}
    </section>
  )
}

function IntroEntry({ intro }: { intro: { body: string; createdAt: number } }) {
  const { lang } = useCopy()
  return (
    <li className="flex justify-end">
      <div className="max-w-[85%] rounded-xl rounded-br-sm bg-primary/8 px-3 py-2 text-sm">
        <Markdown content={intro.body} />
        <p className="mt-1 text-right text-[10px] text-muted-foreground">
          {relativeTime(lang, intro.createdAt)}
        </p>
      </div>
    </li>
  )
}

function ThreadEntry({ record }: { record: ThreadRecord }) {
  const { t, lang } = useCopy()
  const time = relativeTime(lang, record.created_at)
  const image = generatedImageSource(record.payload)
  if (image) {
    return (
      <li>
        <img
          src={image}
          alt={t("threadLiveGeneratedImage")}
          loading="lazy"
          decoding="async"
          className="block h-auto max-w-full rounded-lg border border-border/60"
        />
        <p className="mt-1 text-[10px] text-muted-foreground">{time}</p>
      </li>
    )
  }

  if (record.kind === "input") {
    return (
      <li className="flex justify-end">
        <div className="max-w-[85%] rounded-xl rounded-br-sm bg-primary/8 px-3 py-2 text-sm whitespace-pre-wrap">
          {threadText(record.payload)}
          <p className="mt-1 text-right text-[10px] text-muted-foreground">{time}</p>
        </div>
      </li>
    )
  }

  const call = functionCallOf(record.payload)
  if (call) {
    return (
      <li className="rounded-lg border border-border/60 bg-secondary/30 px-3 py-2">
        <details className="group">
          <summary className="flex cursor-pointer list-none items-center gap-2 text-xs [&::-webkit-details-marker]:hidden">
            <TerminalSquareIcon className="size-3.5 shrink-0 text-muted-foreground" />
            <span className="font-mono text-[11px] text-foreground">{call.name}</span>
            <span className="min-w-0 flex-1 truncate text-muted-foreground">
              {callSummary(call)}
            </span>
            <span className="shrink-0 text-[10px] text-muted-foreground">{time}</span>
            <ChevronDownIcon className="size-3 shrink-0 text-muted-foreground transition-transform group-open:rotate-180" />
          </summary>
          <pre className="mt-2 max-h-48 overflow-auto rounded-md bg-background/70 p-2 text-[11px] whitespace-pre-wrap">
            {call.raw || JSON.stringify(call.args, null, 2)}
          </pre>
        </details>
      </li>
    )
  }

  const payload = payloadObject(record.payload)
  if (record.kind === "response_output" && payload?.type === "message") {
    const text = threadText(record.payload)
    if (!text.trim()) return null
    return (
      <li className="flex gap-2.5">
        <span className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-md bg-reporter/12 text-reporter">
          <SparklesIcon className="size-3.5" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="max-w-[75ch] text-sm break-words">
            <Markdown content={text} />
          </div>
          <p className="mt-1 text-[10px] text-muted-foreground">{time}</p>
        </div>
      </li>
    )
  }

  if (record.kind === "tool_output") {
    const summary = outputSummary(record.payload)
    return (
      <li className="px-1">
        <details className="group">
          <summary className="flex cursor-pointer list-none items-center gap-2 text-xs text-muted-foreground [&::-webkit-details-marker]:hidden">
            <span className="size-1.5 shrink-0 rounded-full bg-border" />
            <span className="shrink-0">{t("threadLiveToolOutput")}</span>
            <span className="min-w-0 flex-1 truncate">{summary || "—"}</span>
            <span className="shrink-0 text-[10px]">{time}</span>
          </summary>
          <pre className="mt-1.5 max-h-48 overflow-auto rounded-md border border-border/50 bg-background/70 p-2 text-[11px] whitespace-pre-wrap">
            {clip(threadText(record.payload), 4000) || JSON.stringify(record.payload, null, 2)}
          </pre>
        </details>
      </li>
    )
  }

  if (record.kind === "checkpoint") {
    return (
      <li className="flex items-center gap-2 py-1 text-center text-[10px] text-muted-foreground">
        <span className="h-px flex-1 bg-border" />
        {t("threadLiveCompacted")}
        <span className="tabular-nums">{time}</span>
        <span className="h-px flex-1 bg-border" />
      </li>
    )
  }

  if (record.kind === "activity") {
    return (
      <li className="px-1 text-[11px] text-muted-foreground">
        {t("threadLiveActivity")} · {time}
      </li>
    )
  }

  const fallback = clip(threadText(record.payload), 200)
  if (!fallback) return null
  return (
    <li className="px-1 text-xs break-words text-muted-foreground">
      {fallback}
    </li>
  )
}
