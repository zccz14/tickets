import { useQuery } from "@tanstack/react-query"
import { MessageSquareIcon, PaperclipIcon } from "lucide-react"
import { useNavigate } from "react-router-dom"

import { EmptyState, PageHeader, StatusBadge } from "@/components/bits"
import type { TicketSummary } from "@/lib/api"
import { relativeTime } from "@/lib/format"
import { useCopy, useSession } from "@/lib/session"

export function MyTicketsPage() {
  const { request } = useSession()
  const { t, lang } = useCopy()
  const navigate = useNavigate()
  const tickets = useQuery({
    queryKey: ["my-tickets"],
    queryFn: () => request<TicketSummary[]>("/api/me/tickets"),
  })

  return (
    <div className="mx-auto w-full max-w-4xl space-y-6">
      <PageHeader description={t("projectsDescription")} />
      {tickets.isPending ? (
        <div className="space-y-2">
          {[0, 1, 2].map((index) => (
            <div key={index} className="h-13 animate-pulse rounded-lg border border-border/60 bg-card/50" />
          ))}
        </div>
      ) : tickets.isError ? (
        <EmptyState title={t("errorLoadFailed")} hint={(tickets.error as Error).message} />
      ) : tickets.data.length === 0 ? (
        <EmptyState title={t("ticketsEmptyAll")} />
      ) : (
        <div className="divide-y divide-border/60 overflow-hidden rounded-xl border border-border/70 bg-card">
          {tickets.data.map((ticket) => (
            <button
              key={ticket.id}
              type="button"
              onClick={() => navigate(`/tickets/${ticket.id}`)}
              className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-accent/40"
            >
              <StatusBadge status={ticket.status} />
              <span className="min-w-0 flex-1 truncate text-sm font-medium">{ticket.title}</span>
              <span className="hidden shrink-0 text-xs text-muted-foreground sm:inline">
                {ticket.project_name} #{ticket.number}
              </span>
              {ticket.has_evidence ? <PaperclipIcon className="size-3 shrink-0 text-reporter" /> : null}
              <span className="flex shrink-0 items-center gap-1 text-xs text-muted-foreground">
                <MessageSquareIcon className="size-3.5" />
                {ticket.comment_count}
              </span>
              <span className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground sm:inline">
                {relativeTime(lang, ticket.updated_at)}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
