import { useMutation, useQuery } from "@tanstack/react-query"
import { ChevronLeftIcon, Loader2Icon, SendIcon, SparklesIcon } from "lucide-react"
import { useState } from "react"
import { useNavigate, useParams } from "react-router-dom"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import type { Project, Ticket } from "@/lib/api"
import { usePageTitle } from "@/lib/page-title"
import { useCopy, useSession } from "@/lib/session"

/**
 * A Ticket starts as a conversation, not a form. The first message creates the
 * draft; the server derives its title from that message, and the ticket page
 * opens the investigation Thread as soon as it loads.
 */
export function NewTicketPage() {
  const { id = "" } = useParams()
  const { request } = useSession()
  const { t } = useCopy()
  const navigate = useNavigate()
  const [body, setBody] = useState("")

  const project = useQuery({
    queryKey: ["project", id],
    queryFn: () => request<Project>(`/api/projects/${id}`),
  })

  usePageTitle(
    project.data ? `${t("ticketNewTitle")} · ${project.data.name}` : t("ticketNewTitle"),
  )

  const create = useMutation({
    mutationFn: () =>
      request<Ticket>(`/api/projects/${id}/tickets`, {
        method: "POST",
        body: JSON.stringify({ body }),
      }),
    onSuccess: (ticket) => navigate(`/tickets/${ticket.id}`),
  })

  function submit() {
    const message = body.trim()
    if (!message || create.isPending) return
    create.mutate()
  }

  return (
    <div className="mx-auto w-full max-w-3xl space-y-5">
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <button
          type="button"
          onClick={() => navigate(`/projects/${id}`)}
          className="inline-flex items-center gap-1 transition-colors hover:text-foreground"
        >
          <ChevronLeftIcon className="size-3.5" />
          {project.data?.name ?? t("navProjects")}
        </button>
        <span className="opacity-40">/</span>
        <span>{t("ticketNewTitle")}</span>
      </div>

      <section className="overflow-hidden rounded-xl border border-border/70 bg-card">
        <header className="flex items-start gap-3 border-b border-border/60 px-5 py-4">
          <span className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md bg-reporter/12 text-reporter">
            <SparklesIcon className="size-4" />
          </span>
          <div className="min-w-0">
            <p className="text-sm font-medium">{t("ticketNewHeading")}</p>
            <p className="mt-1 text-xs text-muted-foreground">{t("ticketNewHint")}</p>
          </div>
        </header>

        <form
          className="space-y-3 p-4"
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <Textarea
            autoFocus
            rows={7}
            value={body}
            onChange={(event) => setBody(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                event.preventDefault()
                submit()
              }
            }}
            placeholder={t("ticketNewPlaceholder")}
            className="min-h-40 resize-none border-0 bg-transparent p-1 text-sm shadow-none focus-visible:ring-0"
          />
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs text-muted-foreground">{t("threadLiveComposerHint")}</span>
            <Button type="submit" size="sm" disabled={!body.trim() || create.isPending}>
              {create.isPending ? <Loader2Icon className="animate-spin" /> : <SendIcon />}
              {t("actionStartInvestigation")}
            </Button>
          </div>
          {create.isError ? (
            <p className="text-sm text-destructive">{(create.error as Error).message}</p>
          ) : null}
        </form>
      </section>

      <div className="rounded-xl border border-dashed border-border/70 px-5 py-4">
        <p className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          {t("ticketNewStepsTitle")}
        </p>
        <ol className="mt-3 space-y-2 text-sm">
          {[t("ticketNewStepOne"), t("ticketNewStepTwo"), t("ticketNewStepThree")].map(
            (step, index) => (
              <li key={step} className="flex gap-3">
                <span className="flex size-5 shrink-0 items-center justify-center rounded-full border border-border text-[10px] text-muted-foreground">
                  {index + 1}
                </span>
                <span className="min-w-0 text-muted-foreground">{step}</span>
              </li>
            ),
          )}
        </ol>
        <p className="mt-3 text-xs text-muted-foreground">{t("ticketNewPrivacy")}</p>
      </div>
    </div>
  )
}
