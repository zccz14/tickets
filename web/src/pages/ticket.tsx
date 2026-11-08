import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  BotIcon,
  ChevronLeftIcon,
  ExternalLinkIcon,
  Loader2Icon,
  SendIcon,
  SparklesIcon,
  WrenchIcon,
  XCircleIcon,
  RefreshCwIcon,
} from "lucide-react"
import { useEffect, useRef } from "react"
import { useNavigate, useParams } from "react-router-dom"
import { Markdown } from "@zccz14/ux"

import { Button } from "@/components/ui/button"
import { AuthorName, CodeBlock, CopyButton, StatusBadge } from "@/components/bits"
import { ThreadLivePanel } from "@/components/thread-live"
import type { Ticket, ThreadInfo } from "@/lib/api"
import { absoluteTime, relativeTime } from "@/lib/format"
import { usePageTitle } from "@/lib/page-title"
import { useCopy, useSession } from "@/lib/session"

/**
 * A Ticket is three pieces of text and two Threads: what the reporter saw, what
 * the reporter Agent measured, and what the resolver Agent did about it. There
 * is no comment timeline — the reporter talks to their own Agent in the live
 * panel, and the resolver Agent is pushed the finished report.
 */
export function TicketPage() {
  const { id = "" } = useParams()
  const { request } = useSession()
  const { t, lang } = useCopy()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const ticket = useQuery({
    queryKey: ["ticket", id],
    queryFn: () => request<Ticket>(`/api/tickets/${id}`),
  })
  const thread = useQuery({
    queryKey: ["ticket-thread", id],
    queryFn: () => request<ThreadInfo>(`/api/tickets/${id}/thread`),
    enabled: Boolean(ticket.data),
    refetchInterval: ticket.data?.reporter_thread.thread_id ? 15_000 : false,
  })

  // The Thread names the Ticket; a rename in Cybion syncs on the next read.
  // Refresh the page header when the title the server reports changes.
  const knownThreadTitle = useRef<string | null>(null)
  const threadTitle = thread.data?.title ?? null
  useEffect(() => {
    if (!threadTitle) return
    const first = knownThreadTitle.current === null
    if (threadTitle === knownThreadTitle.current) return
    knownThreadTitle.current = threadTitle
    if (first) return
    void queryClient.invalidateQueries({ queryKey: ["ticket", id] })
    void queryClient.invalidateQueries({ queryKey: ["tickets"] })
  }, [threadTitle, queryClient, id])

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["ticket", id] })
    void queryClient.invalidateQueries({ queryKey: ["ticket-thread", id] })
    void queryClient.invalidateQueries({ queryKey: ["ticket-thread-live", id] })
    void queryClient.invalidateQueries({ queryKey: ["tickets", ticket.data?.project_id] })
    void queryClient.invalidateQueries({ queryKey: ["projects"] })
  }

  const act = (path: string) =>
    request<Ticket>(`/api/tickets/${id}/${path}`, { method: "POST", body: JSON.stringify({}) })

  const submit = useMutation({ mutationFn: () => act("submit"), onSuccess: invalidate })
  const close = useMutation({ mutationFn: () => act("close"), onSuccess: invalidate })
  const reopen = useMutation({ mutationFn: () => act("reopen"), onSuccess: invalidate })
  const dispatch = useMutation({ mutationFn: () => act("dispatch"), onSuccess: invalidate })
  const createThread = useMutation({
    mutationFn: () => act("thread"),
    // Refresh even on failure: the server records the reason on the Ticket and
    // the sidebar shows it there.
    onSettled: invalidate,
  })

  usePageTitle(
    ticket.data ? `#${ticket.data.number} ${ticket.data.title}` : undefined,
  )

  if (ticket.isPending) {
    return (
      <div className="mx-auto w-full max-w-5xl space-y-4">
        <div className="h-6 w-40 animate-pulse rounded bg-muted" />
        <div className="h-32 animate-pulse rounded-xl border border-border/60 bg-card/50" />
      </div>
    )
  }
  if (ticket.isError || !ticket.data) {
    return (
      <div className="mx-auto w-full max-w-5xl rounded-xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">
        {(ticket.error as Error)?.message ?? t("errorLoadFailed")}
      </div>
    )
  }

  const data = ticket.data
  const threadInfo = thread.data ?? data.reporter_thread
  const canClose = data.is_reporter || data.is_project_owner
  const pending =
    submit.isPending || close.isPending || reopen.isPending || createThread.isPending || dispatch.isPending
  // Two ways a Ticket can be waiting for the resolver: the push failed, or it
  // was submitted before Tickets pushed at all — a pre-push Ticket has neither a
  // Thread nor an error, and still deserves the button.
  const dispatchRetryable =
    data.is_project_owner &&
    data.status === "open" &&
    (!data.resolver_thread.thread_id || Boolean(data.resolver_thread.error))

  return (
    <div className="mx-auto w-full max-w-5xl space-y-5">
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <button
          type="button"
          onClick={() => navigate(`/projects/${data.project_id}`)}
          className="inline-flex items-center gap-1 transition-colors hover:text-foreground"
        >
          <ChevronLeftIcon className="size-3.5" />
          {data.project_name}
        </button>
        <span className="opacity-40">/</span>
        <span className="font-mono">#{data.number}</span>
      </div>

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_19rem]">
        <main className="min-w-0 space-y-4">
          <div className="space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <StatusBadge status={data.status} />
              {data.status === "closed" && data.closed_by_name ? (
                <span className="text-xs text-muted-foreground">
                  {t("ticketClosedBy", { name: data.closed_by_name })}
                </span>
              ) : null}
            </div>
            <h1 className="text-xl font-medium tracking-tight text-foreground">{data.title}</h1>
            <p className="text-xs text-muted-foreground">
              {t("ticketReporter")}:
              <span className="mx-1 text-foreground">
                <AuthorName kind="user" userId={data.reporter_user_id} name={data.reporter_name} />
              </span>
              · {t("ticketOpened")} {relativeTime(lang, data.created_at)}
            </p>
          </div>

          {data.status === "draft" && data.is_reporter ? (
            <div className="flex flex-wrap items-center gap-3 rounded-xl border border-dashed border-primary/40 bg-primary/5 px-4 py-3">
              <p className="flex-1 text-sm text-foreground">{t("ticketDraftBanner")}</p>
              <Button size="sm" disabled={submit.isPending} onClick={() => submit.mutate()}>
                {t("actionSubmit")}
              </Button>
            </div>
          ) : null}

          <article className="rounded-xl border border-border/70 bg-card p-5 text-sm">
            {data.body.trim() ? (
              <Markdown content={data.body} />
            ) : (
              <p className="text-sm text-muted-foreground">—</p>
            )}
          </article>

          {data.is_reporter && (threadInfo.thread_id || data.status !== "closed") ? (
            <ThreadLivePanel ticketId={data.id} thread={threadInfo} status={data.status} />
          ) : null}

          {data.status !== "draft" ? (
            <>
            <section className="rounded-xl border border-reporter/25 bg-reporter/[0.03] p-4">
              <div className="flex flex-wrap items-center gap-2">
                <span className="flex size-6 items-center justify-center rounded-md bg-reporter/15 text-reporter">
                  <SparklesIcon className="size-3.5" />
                </span>
                <p className="text-sm font-medium">{t("ticketEvidenceTitle")}</p>
                {data.evidence_at ? (
                  <span
                    className="ml-auto text-[11px] text-muted-foreground"
                    title={absoluteTime(lang, data.evidence_at)}
                  >
                    {relativeTime(lang, data.evidence_at)}
                  </span>
                ) : null}
              </div>
              {data.evidence ? (
                <div className="mt-3 text-sm">
                  <Markdown content={data.evidence} />
                </div>
              ) : (
                <p className="mt-2 text-xs text-muted-foreground">{t("ticketEvidenceMissing")}</p>
              )}
            </section>

            <section className="rounded-xl border border-resolver/25 bg-resolver/[0.03] p-4">
              <div className="flex flex-wrap items-center gap-2">
                <span className="flex size-6 items-center justify-center rounded-md bg-resolver/15 text-resolver">
                  <WrenchIcon className="size-3.5" />
                </span>
                <p className="text-sm font-medium">{t("ticketResolutionTitle")}</p>
                {data.resolution_at ? (
                  <span
                    className="ml-auto text-[11px] text-muted-foreground"
                    title={absoluteTime(lang, data.resolution_at)}
                  >
                    {relativeTime(lang, data.resolution_at)}
                  </span>
                ) : null}
              </div>
              {data.resolution ? (
                <div className="mt-3 text-sm">
                  <Markdown content={data.resolution} />
                </div>
              ) : (
                <p className="mt-2 text-xs text-muted-foreground">{t("ticketResolutionMissing")}</p>
              )}
            </section>
            </>
          ) : null}

          {canClose ? (
            <div className="flex justify-end gap-2 pt-1">
              {data.status === "closed" ? (
                <Button variant="outline" size="sm" disabled={pending} onClick={() => reopen.mutate()}>
                  <RefreshCwIcon />
                  {t("actionReopen")}
                </Button>
              ) : data.status === "open" ? (
                <Button variant="outline" size="sm" disabled={pending} onClick={() => close.mutate()}>
                  <XCircleIcon />
                  {t("actionClose")}
                </Button>
              ) : null}
            </div>
          ) : null}
        </main>

        <aside className="space-y-3">
          <ThreadCard
            title={t("ticketSidebarReporterAgent")}
            thread={threadInfo}
            loading={thread.isFetching}
            canCreate={data.is_reporter && data.status === "open"}
            isDraft={data.status === "draft"}
            creating={createThread.isPending}
            onCreate={() => createThread.mutate()}
            connectHint={t("threadConnectHint")}
          />

          <section className="rounded-xl border border-resolver/25 bg-resolver/[0.04] p-4">
            <div className="flex items-center gap-2">
              <span className="flex size-6 items-center justify-center rounded-md bg-resolver/15 text-resolver">
                <BotIcon className="size-3.5" />
              </span>
              <p className="text-sm font-medium">{t("ticketSidebarResolverAgent")}</p>
            </div>
            <p className="mt-2 text-xs text-muted-foreground">
              {data.status === "draft"
                ? t("resolverCardDraft")
                : data.resolver_thread.thread_id
                  ? t("resolverCardDispatched")
                  : t("resolverCardNotDispatched")}
            </p>
            {data.resolver_thread.url ? (
              <a
                href={data.resolver_thread.url}
                target="_blank"
                rel="noreferrer noopener"
                className="mt-2 inline-flex items-center gap-1 text-xs text-primary hover:underline"
              >
                {t("resolverThreadLabel")}
                <ExternalLinkIcon className="size-3" />
              </a>
            ) : data.resolver_thread.thread_id ? (
              <p className="mt-2 text-[11px] text-muted-foreground">{t("resolverThreadOwnerNote")}</p>
            ) : null}
            {data.resolver_thread.error ? (
              <p className="mt-2 text-[11px] text-destructive">{data.resolver_thread.error}</p>
            ) : null}
            {dispatchRetryable ? (
              <Button
                className="mt-2"
                size="sm"
                variant="outline"
                disabled={dispatch.isPending}
                onClick={() => dispatch.mutate()}
              >
                {dispatch.isPending ? <Loader2Icon className="animate-spin" /> : <SendIcon />}
                {t("actionDispatch")}
              </Button>
            ) : null}
            {dispatch.isError ? (
              <p className="mt-2 text-[11px] text-destructive">{(dispatch.error as Error).message}</p>
            ) : null}
          </section>

          <details className="rounded-xl border border-border/70 bg-card p-4">
            <summary className="cursor-pointer text-sm font-medium">{t("ticketSidebarAgentApi")}</summary>
            <p className="mt-2 text-xs text-muted-foreground">{t("guideTokenNote")}</p>
            <div className="mt-2 space-y-2">
              <CodeBlock
                code={`curl -sS ${data.agent.ticket_url} \\
  -H "Authorization: Bearer <token>"`}
              />
              <CopyButton value={data.agent.ticket_url} />
            </div>
          </details>
        </aside>
      </div>
    </div>
  )
}

function ThreadCard({
  title,
  thread,
  loading,
  canCreate,
  isDraft,
  creating,
  onCreate,
  connectHint,
}: {
  title: string
  thread: ThreadInfo
  loading: boolean
  canCreate: boolean
  isDraft: boolean
  creating: boolean
  onCreate: () => void
  connectHint: string
}) {
  const { t } = useCopy()
  return (
    <section className="rounded-xl border border-reporter/25 bg-reporter/[0.04] p-4">
      <div className="flex items-center gap-2">
        <span className="flex size-6 items-center justify-center rounded-md bg-reporter/15 text-reporter">
          <SparklesIcon className="size-3.5" />
        </span>
        <p className="flex-1 text-sm font-medium">{title}</p>
        {loading ? <Loader2Icon className="size-3.5 animate-spin text-muted-foreground" /> : null}
      </div>

      {thread.thread_id ? (
        <div className="mt-3 space-y-2">
          <div className="flex flex-wrap items-center gap-2 text-xs">
            <span className="rounded-md bg-background/70 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
              {thread.thread_id.slice(0, 8)}
            </span>
            {thread.display_status || thread.status ? (
              <span className="rounded-full bg-background/70 px-2 py-0.5 text-[10px] text-foreground">
                {thread.display_status || thread.status}
              </span>
            ) : null}
            {thread.context_tokens ? (
              <span className="text-[10px] text-muted-foreground">
                {t("threadTokens", { n: thread.context_tokens.toLocaleString() })}
              </span>
            ) : null}
          </div>
          {thread.url ? (
            <a
              href={thread.url}
              target="_blank"
              rel="noreferrer noopener"
              className="inline-flex items-center gap-1 text-xs font-medium text-primary hover:underline"
            >
              {t("actionOpenThread")}
              <ExternalLinkIcon className="size-3" />
            </a>
          ) : (
            <p className="text-[11px] text-muted-foreground">{t("threadOwnerNote")}</p>
          )}
          {thread.error ? <p className="text-[11px] text-destructive">{thread.error}</p> : null}
        </div>
      ) : (
        <div className="mt-3 space-y-2">
          <p className="text-xs text-muted-foreground">{t("threadMissing")}</p>
          <p className="text-[11px] text-muted-foreground">
            {canCreate
              ? t(isDraft ? "threadMissingDraftHint" : "threadMissingOpenHint")
              : t("threadMissingHint")}
          </p>
          {thread.error ? <p className="text-[11px] text-destructive">{thread.error}</p> : null}
          {canCreate ? (
            <>
              <Button size="sm" variant="outline" disabled={creating} onClick={onCreate}>
                {t("actionCreateThread")}
              </Button>
              <p className="text-[11px] text-muted-foreground">{connectHint}</p>
            </>
          ) : null}
        </div>
      )}
    </section>
  )
}
