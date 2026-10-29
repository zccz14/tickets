import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { KeyRoundIcon, PauseIcon, PlayIcon, UnplugIcon } from "lucide-react"
import { useState } from "react"

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
import type { CybionWorker, Machine, Me, Project } from "@/lib/api"
import { relativeTime } from "@/lib/format"
import { useCopy, useSession } from "@/lib/session"

type ResolverView = { machine: Machine | null }

export function ProjectSettingsDialog({
  project,
  open,
  onOpenChange,
}: {
  project: Project
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { request } = useSession()
  const { t, lang } = useCopy()
  const queryClient = useQueryClient()
  const [name, setName] = useState(project.name)
  const [description, setDescription] = useState(project.description)
  const [workerId, setWorkerId] = useState("")

  const me = useQuery({
    queryKey: ["me"],
    queryFn: () => request<Me>("/api/me"),
    enabled: open,
  })
  const connected = Boolean(me.data?.cybion_connected)
  const workers = useQuery({
    queryKey: ["cybion-workers"],
    queryFn: () => request<{ workers: CybionWorker[] }>("/api/cybion/workers"),
    enabled: open && connected,
  })
  const resolver = useQuery({
    queryKey: ["project-resolver", project.id],
    queryFn: () => request<ResolverView>(`/api/projects/${project.id}/resolver`),
    enabled: open && project.is_owner && connected,
  })
  const machine = resolver.data?.machine ?? null

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["project", project.id] })
    void queryClient.invalidateQueries({ queryKey: ["projects"] })
    void queryClient.invalidateQueries({ queryKey: ["project-resolver", project.id] })
  }

  const save = useMutation({
    mutationFn: () =>
      request<Project>(`/api/projects/${project.id}`, {
        method: "PATCH",
        body: JSON.stringify({ name, description }),
      }),
    onSuccess: () => {
      refresh()
      onOpenChange(false)
    },
  })

  const connectMachine = useMutation({
    mutationFn: () =>
      request<ResolverView>(`/api/projects/${project.id}/resolver`, {
        method: "POST",
        body: JSON.stringify({ worker_id: workerId, interval_seconds: 120 }),
      }),
    onSuccess: refresh,
  })

  const toggleMachine = useMutation({
    mutationFn: (enabled: boolean) =>
      request<ResolverView>(`/api/projects/${project.id}/resolver`, {
        method: "PATCH",
        body: JSON.stringify({ enabled }),
      }),
    onSuccess: refresh,
  })

  const runNow = useMutation({
    mutationFn: () =>
      request<unknown>(`/api/projects/${project.id}/resolver/run`, { method: "POST" }),
  })

  const disconnectMachine = useMutation({
    mutationFn: () =>
      request<unknown>(`/api/projects/${project.id}/resolver`, { method: "DELETE" }),
    onSuccess: refresh,
  })

  const available = workers.data?.workers ?? []
  const selected =
    workerId || available.find((worker) => worker.status === "online")?.id || available[0]?.id || ""
  const failure =
    connectMachine.error ?? toggleMachine.error ?? runNow.error ?? disconnectMachine.error

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("actionManaging")}</DialogTitle>
          <DialogDescription>{t("projectsDescription")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-5">
          <div className="space-y-1.5">
            <Label htmlFor="settings-name">{t("projectNameLabel")}</Label>
            <Input id="settings-name" value={name} onChange={(event) => setName(event.target.value)} />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="settings-description">{t("projectDescriptionLabel")}</Label>
            <Textarea
              id="settings-description"
              rows={3}
              value={description}
              onChange={(event) => setDescription(event.target.value)}
            />
          </div>

          <div className="space-y-3 rounded-xl border border-border/70 bg-secondary/30 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-sm font-medium">{t("ticketSidebarResolverAgent")}</p>
                <p className="text-xs text-muted-foreground">{t("resolverMissingHint")}</p>
              </div>
              <span
                className={
                  machine
                    ? "shrink-0 rounded-full bg-resolver/15 px-2 py-0.5 text-xs font-medium text-resolver"
                    : "shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground"
                }
              >
                {machine ? t("resolverConnected") : t("resolverNotConnected")}
              </span>
            </div>

            {!connected ? (
              <p className="text-xs text-muted-foreground">{t("resolverNeedCybion")}</p>
            ) : machine ? (
              <div className="space-y-3">
                <div className="flex flex-wrap items-center gap-2 text-xs">
                  <span
                    className={
                      machine.enabled
                        ? "rounded-full bg-resolver/15 px-2 py-0.5 font-medium text-resolver"
                        : "rounded-full bg-muted px-2 py-0.5 text-muted-foreground"
                    }
                  >
                    {machine.enabled ? t("resolverEnabled") : t("resolverPaused")}
                  </span>
                  <span className="text-muted-foreground">
                    {machine.worker_label ?? machine.worker_id}
                  </span>
                  <span className="opacity-40">·</span>
                  <span className="text-muted-foreground">
                    {t("resolverInterval", { n: machine.interval_seconds })}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground">
                  {machine.last_run_at
                    ? `${t("resolverLastRun")} ${relativeTime(lang, machine.last_run_at)} · ${
                        machine.last_exit === 0
                          ? t("resolverExitOk")
                          : t("resolverExit", { code: machine.last_exit ?? "—" })
                      }`
                    : t("resolverNoRun")}
                </p>
                {project.resolver_thread_url ? (
                  <a
                    className="inline-flex text-xs font-medium text-primary underline-offset-4 hover:underline"
                    href={project.resolver_thread_url}
                    rel="noreferrer"
                    target="_blank"
                  >
                    {t("actionOpenThread")}
                  </a>
                ) : null}
                <div className="flex flex-wrap gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={toggleMachine.isPending}
                    onClick={() => toggleMachine.mutate(!machine.enabled)}
                  >
                    {machine.enabled ? <PauseIcon /> : <PlayIcon />}
                    {machine.enabled ? t("actionPause") : t("actionResume")}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={runNow.isPending}
                    onClick={() => runNow.mutate()}
                  >
                    <PlayIcon />
                    {t("actionRunNow")}
                  </Button>
                  <Button
                    size="sm"
                    variant="destructive"
                    disabled={disconnectMachine.isPending}
                    onClick={() => disconnectMachine.mutate()}
                  >
                    <UnplugIcon />
                    {t("actionDisconnectMachine")}
                  </Button>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                {workers.isError ? (
                  <p className="text-xs text-destructive">
                    {(workers.error as Error).message}
                  </p>
                ) : available.length === 0 ? (
                  <p className="text-xs text-muted-foreground">{t("resolverNoWorkers")}</p>
                ) : (
                  <div className="space-y-1.5">
                    <Label>{t("resolverWorkersLabel")}</Label>
                    <div className="flex flex-wrap gap-2">
                      {available.map((worker) => (
                        <Button
                          key={worker.id}
                          size="sm"
                          variant={selected === worker.id ? "default" : "outline"}
                          onClick={() => setWorkerId(worker.id)}
                        >
                          {worker.label}
                          <span className="text-[10px] opacity-70">
                            {worker.status === "online"
                              ? t("resolverWorkerOnline")
                              : t("resolverWorkerOffline")}
                          </span>
                        </Button>
                      ))}
                    </div>
                  </div>
                )}
                <p className="text-xs text-muted-foreground">{t("resolverConnectHint")}</p>
                <Button
                  size="sm"
                  disabled={!selected || connectMachine.isPending}
                  onClick={() => connectMachine.mutate()}
                >
                  <KeyRoundIcon />
                  {t("resolverConnect")}
                </Button>
              </div>
            )}

            {failure ? (
              <p className="text-xs text-destructive">{(failure as Error).message}</p>
            ) : null}
          </div>

          {save.isError ? (
            <p className="text-sm text-destructive">{(save.error as Error).message}</p>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("actionCancel")}
          </Button>
          <Button disabled={save.isPending || !name.trim()} onClick={() => save.mutate()}>
            {t("actionSave")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
