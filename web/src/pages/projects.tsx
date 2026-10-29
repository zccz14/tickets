import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { FolderIcon, PlusIcon } from "lucide-react"
import { useState } from "react"
import { useNavigate } from "react-router-dom"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { EmptyState, PageHeader } from "@/components/bits"
import type { Project } from "@/lib/api"
import { relativeTime } from "@/lib/format"
import { useCopy, useSession } from "@/lib/session"

export function ProjectsPage() {
  const { request } = useSession()
  const { t, lang } = useCopy()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [creating, setCreating] = useState(false)
  const [name, setName] = useState("")
  const [description, setDescription] = useState("")

  const projects = useQuery({
    queryKey: ["projects"],
    queryFn: () => request<Project[]>("/api/projects"),
  })

  const create = useMutation({
    mutationFn: () =>
      request<Project>("/api/projects", {
        method: "POST",
        body: JSON.stringify({ name, description }),
      }),
    onSuccess: (project) => {
      void queryClient.invalidateQueries({ queryKey: ["projects"] })
      setCreating(false)
      setName("")
      setDescription("")
      navigate(`/projects/${project.id}`)
    },
  })

  return (
    <div className="mx-auto w-full max-w-5xl space-y-6">
      <PageHeader
        description={t("projectsDescription")}
        actions={
          <Button onClick={() => setCreating(true)}>
            <PlusIcon />
            {t("actionCreateProject")}
          </Button>
        }
      />

      {projects.isPending ? (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {[0, 1, 2].map((index) => (
            <div key={index} className="h-36 animate-pulse rounded-xl border border-border/60 bg-card/50" />
          ))}
        </div>
      ) : projects.isError ? (
        <EmptyState title={t("errorLoadFailed")} hint={(projects.error as Error).message} />
      ) : projects.data.length === 0 ? (
        <EmptyState
          title={t("projectsEmpty")}
          hint={t("projectsEmptyHint")}
          action={
            <Button variant="outline" onClick={() => setCreating(true)}>
              <PlusIcon />
              {t("actionCreateProject")}
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {projects.data.map((project) => (
            <button
              key={project.id}
              type="button"
              onClick={() => navigate(`/projects/${project.id}`)}
              className="group flex h-full flex-col gap-3 rounded-xl border border-border/70 bg-card p-4 text-left transition-colors hover:border-primary/40 hover:bg-accent/40"
            >
              <div className="flex items-start gap-2">
                <span className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  <FolderIcon className="size-4" />
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium text-foreground">{project.name}</p>
                  <p className="truncate text-xs text-muted-foreground">
                    {project.owner_name ?? project.owner_user_id.slice(0, 8)}
                    <span className="mx-1.5 opacity-40">·</span>
                    {relativeTime(lang, project.updated_at)}
                  </p>
                </div>
              </div>
              <p className="line-clamp-2 min-h-9 text-sm text-muted-foreground">
                {project.description || "—"}
              </p>
              <div className="mt-auto flex items-center gap-2 text-xs">
                <span
                  className={
                    project.open_tickets > 0
                      ? "rounded-full bg-primary/10 px-2 py-0.5 font-medium text-primary"
                      : "rounded-full bg-muted px-2 py-0.5 text-muted-foreground"
                  }
                >
                  {t("projectOpenCount", { n: project.open_tickets })}
                </span>
                <span className="text-muted-foreground">
                  {t("projectTotalCount", { n: project.total_tickets })}
                </span>
                {project.resolver_ready ? (
                  <span className="ml-auto inline-flex items-center gap-1 text-muted-foreground">
                    <span className="size-1.5 rounded-full bg-resolver" />
                    Agent
                  </span>
                ) : null}
              </div>
            </button>
          ))}
        </div>
      )}

      <Dialog open={creating} onOpenChange={setCreating}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("actionCreateProject")}</DialogTitle>
            <DialogDescription>{t("projectsEmptyHint")}</DialogDescription>
          </DialogHeader>
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault()
              if (name.trim()) create.mutate()
            }}
          >
            <div className="space-y-1.5">
              <Label htmlFor="project-name">{t("projectsTitle")}</Label>
              <Input
                id="project-name"
                autoFocus
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="payments"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="project-description">{t("ticketBodyLabel")}</Label>
              <Textarea
                id="project-description"
                rows={3}
                value={description}
                onChange={(event) => setDescription(event.target.value)}
                placeholder="这个项目负责什么，谁来修。"
              />
            </div>
            {create.isError ? (
              <p className="text-sm text-destructive">{(create.error as Error).message}</p>
            ) : null}
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setCreating(false)}>
                {t("actionCancel")}
              </Button>
              <Button type="submit" disabled={!name.trim() || create.isPending}>
                {t("actionCreateProject")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  )
}
