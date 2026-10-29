import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  BotIcon,
  ChevronLeftIcon,
  ExternalLinkIcon,
  Loader2Icon,
  MessageSquareIcon,
  RefreshCwIcon,
  SendIcon,
  SparklesIcon,
  XCircleIcon,
} from "lucide-react"
import { useState } from "react"
import { useNavigate, useParams } from "react-router-dom"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { Markdown } from "@/lib/markdown"
import { AuthorName, CodeBlock, CopyButton, StatusBadge } from "@/components/bits"
import type { Ticket, ThreadInfo } from "@/lib/api"
import { absoluteTime, relativeTime } from "@/lib/format"
import { usePageTitle } from "@/lib/page-title"
import { useCopy, useSession } from "@/lib/session"
import { cn } from "@/lib/utils"

export function TicketPage() {
  const { id = "" } = useParams()
  const { request } = useSession()
  const { t, lang } = useCopy()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [draft, setDraft] = useState("")

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

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["ticket", id] })
    void queryClient.invalidateQueries({ queryKey: ["ticket-thread", id] })
    void queryClient.invalidateQueries({ queryKey: ["tickets", ticket.data?.project_id] })
    void queryClient.invalidateQueries({ queryKey: ["projects"] })
  }

  const act = (path: string) =>
    request<Ticket>(`/api/tickets/${id}/${path}`, { method: "POST", body: JSON.stringify({}) })

  const comment = useMutation({
    mutationFn: () =>
      request<Ticket>(`/api/tickets/${id}/comments`, {
        method: "POST",
        body: JSON.stringify({ body: draft }),
      }),
    onSuccess: () => {
      setDraft("")
      invalidate()
    },
  })
  const submit = useMutation({ mutationFn: () => act("submit"), onSuccess: invalidate })
  const close = useMutation({ mutationFn: () => act("close"), onSuccess: invalidate })
  const reopen = useMutation({ mutationFn: () => act("reopen"), onSuccess: invalidate })
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
    comment.isPending || submit.isPending || close.isPending || reopen.isPending || createThread.isPending

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

          <article className="rounded-xl border border-border/70 bg-card p-5">
            {data.body.trim() ? (
              <Markdown text={data.body} />
            ) : (
              <p className="text-sm text-muted-foreground">—</p>
            )}
          </article>

          <div className="space-y-3">
            <div className="flex items-center gap-2 text-xs font-medium tracking-wide text-muted-foreground uppercase">
              <MessageSquareIcon className="size-3.5" />
              {t("ticketComments")}
              <span className="text-foreground">{data.comments.length}</span>
            </div>

            {data.comments.length === 0 ? (
              <p className="rounded-xl border border-dashed border-border/70 px-4 py-6 text-center text-sm text-muted-foreground">
                {t("ticketNoComments")}
              </p>
            ) : (
              <ul className="space-y-3">
                {data.comments.map((entry) =>
                  entry.kind === "system" ? (
                    <li
                      key={entry.id}
                      className="flex items-center justify-center gap-2 py-1 text-center text-xs text-muted-foreground"
                    >
                      <span className="h-px w-6 bg-border" />
                      <span className="[&_.prose-ticket]:text-xs [&_p]:my-0 [&_p]:text-xs">
                        <Markdown text={entry.body} />
                      </span>
                      <span className="tabular-nums opacity-70">{relativeTime(lang, entry.created_at)}</span>
                      <span className="h-px w-6 bg-border" />
                    </li>
                  ) : (
                    <li
                      key={entry.id}
                      className={cn(
                        "rounded-xl border bg-card p-4",
                        entry.kind === "evidence"
                          ? "border-reporter/30 bg-reporter/[0.04]"
                          : "border-border/70",
                      )}
                    >
                      <div className="mb-2 flex flex-wrap items-center gap-2 text-xs">
                        <AuthorName
                          kind={entry.author_kind}
                          userId={entry.author_user_id}
                          name={entry.author_name}
                        />
                        {entry.kind === "evidence" ? (
                          <span className="inline-flex items-center gap-1 rounded-md border border-reporter/25 bg-reporter/10 px-1.5 py-0.5 text-[10px] text-reporter">
                            <SparklesIcon className="size-2.5" />
                            现场证据
                          </span>
                        ) : null}
                        <span
                          className="ml-auto text-muted-foreground"
                          title={absoluteTime(lang, entry.created_at)}
                        >
                          {relativeTime(lang, entry.created_at)}
                        </span>
                      </div>
                      <Markdown text={entry.body} />
                    </li>
                  ),
                )}
              </ul>
            )}
          </div>

          {data.status !== "closed" ? (
            <form
              className="space-y-2 rounded-xl border border-border/70 bg-card p-3"
              onSubmit={(event) => {
                event.preventDefault()
                if (draft.trim()) comment.mutate()
              }}
            >
              <Textarea
                rows={3}
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
                placeholder={t("ticketCommentPlaceholder")}
                className="border-0 bg-transparent p-0 shadow-none focus-visible:ring-0"
              />
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs text-muted-foreground">Markdown</span>
                <Button type="submit" size="sm" disabled={!draft.trim() || comment.isPending}>
                  <SendIcon />
                  {t("actionComment")}
                </Button>
              </div>
              {comment.isError ? (
                <p className="text-sm text-destructive">{(comment.error as Error).message}</p>
              ) : null}
            </form>
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
            canCreate={data.is_reporter}
            isDraft={data.status === "draft"}
            creating={createThread.isPending}
            onCreate={() => createThread.mutate()}
            connectHint={t("threadConnectHint")}
          />

          <section className="rounded-xl border border-border/70 bg-card p-4">
            <div className="flex items-center gap-2">
              <span className="flex size-6 items-center justify-center rounded-md bg-resolver/12 text-resolver">
                <BotIcon className="size-3.5" />
              </span>
              <p className="text-sm font-medium">{t("ticketSidebarResolverAgent")}</p>
            </div>
            {data.resolver_thread_url ? (
              <a
                href={data.resolver_thread_url}
                target="_blank"
                rel="noreferrer noopener"
                className="mt-2 inline-flex items-center gap-1 text-xs text-primary hover:underline"
              >
                {t("resolverThreadLabel")}
                <ExternalLinkIcon className="size-3" />
              </a>
            ) : (
              <p className="mt-2 text-xs text-muted-foreground">{t("resolverMissingHint")}</p>
            )}
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
