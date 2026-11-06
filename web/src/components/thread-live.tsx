import { useQuery, useQueryClient } from "@tanstack/react-query"
import { BotIcon, CircleAlertIcon, Loader2Icon } from "lucide-react"
import { useEffect, useRef } from "react"
import { CybionThreadProvider, CybionThreadView } from "@zccz14/cybion-components"

import { Button } from "@/components/ui/button"
import type { ThreadInfo } from "@/lib/api"
import { useCopy, useSession } from "@/lib/session"

type LiveThread = {
  thread: ThreadInfo
  records: unknown[]
  response: unknown
}

/**
 * The reporter's side of the investigation, in the ticket page.
 *
 * Tickets owns two things here and nothing else: starting the Thread when a
 * draft is first opened, and binding it to the ticket. The conversation itself —
 * history, tool cards, composer, model settings, compaction, voice — is the
 * embeddable `CybionThreadView`, the same one Cybion serves on its own pages.
 *
 * The Thread belongs to the reporter and runs on their Cybion account, so the
 * browser talks to Cybion with that person's Auth Mini token.
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
  const { request, lang, getToken, cybionBaseUrl } = useSession()
  const { t } = useCopy()
  const queryClient = useQueryClient()
  const knownThread = useRef<string | null>(thread.thread_id)

  // A draft has no Thread until its reporter opens it. This read is what starts
  // the investigation, and it keeps polling only until the Thread exists.
  const ensure = useQuery({
    queryKey: ["ticket-thread", ticketId],
    queryFn: () => request<LiveThread>(`/api/tickets/${ticketId}/thread/live?after=0`),
    refetchInterval: thread.thread_id ? false : 2500,
    enabled: status !== "closed",
  })

  const threadId = ensure.data?.thread.thread_id ?? thread.thread_id
  // Only a creation failure belongs here; the sidebar's probe error is its own.
  const threadError = ensure.data?.thread.error ?? thread.error

  // The rest of the page still shows the old "no Thread" state until its own
  // queries refresh, so a freshly started Thread has to wake them.
  useEffect(() => {
    if (knownThread.current === threadId) return
    knownThread.current = threadId
    void queryClient.invalidateQueries({ queryKey: ["ticket", ticketId] })
    void queryClient.invalidateQueries({ queryKey: ["tickets"] })
  }, [threadId, queryClient, ticketId])

  return (
    <section className="flex max-h-[40rem] min-h-0 flex-col overflow-hidden rounded-xl border border-border/70 bg-card">
      <header className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border/60 px-4 py-3">
        <span className="flex size-6 items-center justify-center rounded-md bg-reporter/12 text-reporter">
          <BotIcon className="size-3.5" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium">{t("threadLiveTitle")}</p>
          <p className="text-xs text-muted-foreground">{t("threadLiveVisibility")}</p>
        </div>
      </header>

      {threadId ? (
        <CybionThreadProvider
          baseUrl={cybionBaseUrl}
          getToken={getToken}
          language={lang}
          link={({ href, children, ...rest }) => (
            <a href={href} {...rest}>
              {children}
            </a>
          )}
        >
          <CybionThreadView
            threadId={threadId}
            intro={intro}
            readOnly={status === "closed"}
            className="min-h-[18rem]"
          />
        </CybionThreadProvider>
      ) : (
        <div className="min-h-[10rem] flex-1 space-y-3 px-4 py-4">
          {intro?.body.trim() ? (
            <div className="ml-auto max-w-[85%] rounded-xl rounded-br-sm bg-primary/8 px-3 py-2 text-sm whitespace-pre-wrap">
              {intro.body}
            </div>
          ) : null}
          {threadError ? (
            <div className="flex flex-wrap items-center gap-2 text-xs text-destructive">
              <CircleAlertIcon className="size-3.5" />
              <span className="min-w-0 flex-1">{threadError}</span>
              <Button size="sm" variant="outline" onClick={() => void ensure.refetch()}>
                {t("actionRetry")}
              </Button>
            </div>
          ) : (
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2Icon className="size-3.5 animate-spin" />
              {status === "closed" ? t("threadLiveClosed") : t("threadLiveStarting")}
            </p>
          )}
        </div>
      )}
    </section>
  )
}
