import { useQuery } from "@tanstack/react-query"
import { ChevronLeftIcon, MessageSquareIcon, PaperclipIcon, PlusIcon, SettingsIcon } from "lucide-react"
import { useState } from "react"
import { useNavigate, useParams } from "react-router-dom"

import { Button } from "@/components/ui/button"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { EmptyState, PageHeader, StatusBadge, VisibilityBadge } from "@/components/bits"
import { ProjectSettingsDialog } from "@/components/project-settings"
import type { Project, TicketSummary } from "@/lib/api"
import { relativeTime } from "@/lib/format"
import { useCopy, useSession } from "@/lib/session"

type Filter = "open" | "closed" | "all" | "draft"

export function ProjectPage() {
  const { id = "" } = useParams()
  const { request } = useSession()
  const { t, lang } = useCopy()
  const navigate = useNavigate()
  const [filter, setFilter] = useState<Filter>("open")
  const [settingsOpen, setSettingsOpen] = useState(false)

  const project = useQuery({
    queryKey: ["project", id],
    queryFn: () => request<Project>(`/api/projects/${id}`),
  })
  const tickets = useQuery({
    queryKey: ["tickets", id, filter],
    queryFn: () => request<TicketSummary[]>(`/api/projects/${id}/tickets?status=${filter}`),
  })

  const emptyKey =
    filter === "open"
      ? "ticketsEmptyOpen"
      : filter === "closed"
        ? "ticketsEmptyClosed"
        : filter === "draft"
          ? "ticketsEmptyDraft"
          : "ticketsEmptyAll"

  return (
    <div className="mx-auto w-full max-w-5xl space-y-6">
      <PageHeader
        breadcrumb={
          <button
            type="button"
            onClick={() => navigate("/projects")}
            className="inline-flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
          >
            <ChevronLeftIcon className="size-3.5" />
            {t("navProjects")}
          </button>
        }
        title={
          project.data ? (
            <span className="flex flex-wrap items-center gap-2">
              {project.data.name}
              <VisibilityBadge visibility={project.data.visibility} />
            </span>
          ) : (
            t("loading")
          )
        }
        description={project.data?.description || undefined}
        actions={
          <>
            {project.data?.is_owner ? (
              <Button variant="outline" onClick={() => setSettingsOpen(true)}>
                <SettingsIcon />
                {t("actionManaging")}
              </Button>
            ) : null}
            <Button onClick={() => navigate(`/projects/${id}/new`)}>
              <PlusIcon />
              {t("actionCreateTicket")}
            </Button>
          </>
        }
      />

      <Tabs value={filter} onValueChange={(value) => setFilter(value as Filter)}>
        <TabsList>
          <TabsTrigger value="open">{t("stateOpen")}</TabsTrigger>
          <TabsTrigger value="closed">{t("stateClosed")}</TabsTrigger>
          <TabsTrigger value="all">{t("stateAll")}</TabsTrigger>
          <TabsTrigger value="draft">{t("stateDraft")}</TabsTrigger>
        </TabsList>
      </Tabs>

      {tickets.isPending ? (
        <div className="space-y-2">
          {[0, 1, 2].map((index) => (
            <div key={index} className="h-13 animate-pulse rounded-lg border border-border/60 bg-card/50" />
          ))}
        </div>
      ) : tickets.isError ? (
        <EmptyState title={t("errorLoadFailed")} hint={(tickets.error as Error).message} />
      ) : tickets.data.length === 0 ? (
        <EmptyState
          title={t(emptyKey)}
          action={
            filter === "open" || filter === "all" ? (
              <Button variant="outline" onClick={() => navigate(`/projects/${id}/new`)}>
                <PlusIcon />
                {t("actionCreateTicket")}
              </Button>
            ) : undefined
          }
        />
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
              <span className="hidden shrink-0 font-mono text-xs text-muted-foreground sm:inline">
                #{ticket.number}
              </span>
              <span className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
                {ticket.title}
              </span>
              {ticket.has_evidence ? (
                <span className="hidden shrink-0 items-center gap-1 rounded-md border border-reporter/25 bg-reporter/8 px-1.5 py-0.5 text-[10px] text-reporter sm:inline-flex">
                  <PaperclipIcon className="size-2.5" />
                  证据
                </span>
              ) : null}
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

      {project.data ? (
        <ProjectSettingsDialog
          project={project.data}
          open={settingsOpen}
          onOpenChange={setSettingsOpen}
        />
      ) : null}
    </div>
  )
}
