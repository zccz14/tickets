import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { LinkitUserInfo, LinkitUserPicker } from "linkit-react-components"
import { KeyRoundIcon, PauseIcon, PlayIcon, UnplugIcon, UserPlusIcon, XIcon } from "lucide-react"
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import type { CybionWorker, Machine, Me, Project, ProjectGrant, ProjectVisibility } from "@/lib/api"
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
  const [visibility, setVisibility] = useState<ProjectVisibility>(project.visibility)
  const [memberId, setMemberId] = useState("")

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
  const grants = useQuery({
    queryKey: ["project-grants", project.id],
    queryFn: () =>
      request<{ grants: ProjectGrant[] }>(`/api/projects/${project.id}/grants`),
    enabled: open && project.is_owner,
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
    void queryClient.invalidateQueries({ queryKey: ["project-grants", project.id] })
  }

  const save = useMutation({
    mutationFn: () =>
      request<Project>(`/api/projects/${project.id}`, {
        method: "PATCH",
        body: JSON.stringify({ name, description, visibility }),
      }),
    onSuccess: () => {
      refresh()
      onOpenChange(false)
    },
  })

  const addMember = useMutation({
    mutationFn: () =>
      request<unknown>(`/api/projects/${project.id}/grants`, {
        method: "POST",
        body: JSON.stringify({ user_id: memberId.trim() }),
      }),
    onSuccess: () => {
      setMemberId("")
      refresh()
    },
  })

  const removeMember = useMutation({
    mutationFn: (userId: string) =>
      request<unknown>(`/api/projects/${project.id}/grants/${userId}`, {
        method: "DELETE",
      }),
    onSuccess: refresh,
  })

  // The picker searches the whole Linkit directory, so it can also produce the
  // owner or someone already on the list; both are rejected before the request.
  const memberIsOwner = Boolean(memberId) && memberId === me.data?.id
  const memberAlreadyGranted =
    grants.data?.grants.some((grant) => grant.user_id === memberId) ?? false
  const memberInvalid = memberIsOwner || memberAlreadyGranted

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

          <div className="space-y-1.5">
            <Label>{t("projectVisibilityLabel")}</Label>
            <Select
              value={visibility}
              onValueChange={(value) => setVisibility(value as ProjectVisibility)}
            >
              <SelectTrigger className="w-full">
                <SelectValue>
                  {visibility === "private"
                    ? t("projectVisibilityPrivate")
                    : t("projectVisibilityPublic")}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="private">{t("projectVisibilityPrivate")}</SelectItem>
                <SelectItem value="public">{t("projectVisibilityPublic")}</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              {visibility === "private"
                ? t("projectVisibilityPrivateHint")
                : t("projectVisibilityPublicHint")}
            </p>
          </div>

          <div className="space-y-3 rounded-xl border border-border/70 bg-secondary/30 p-4">
            <div>
              <p className="text-sm font-medium">{t("projectMembersLabel")}</p>
              <p className="text-xs text-muted-foreground">{t("projectMembersHint")}</p>
            </div>
            {grants.isPending ? (
              <p className="text-xs text-muted-foreground">…</p>
            ) : grants.data?.grants.length ? (
              <ul className="space-y-1.5">
                {grants.data.grants.map((grant) => (
                  <li
                    key={grant.user_id}
                    className="flex items-center gap-2 rounded-lg border border-border/60 bg-background/60 px-2.5 py-1.5"
                  >
                    <span className="min-w-0 flex-1">
                      <LinkitUserInfo userId={grant.user_id} compact />
                    </span>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={removeMember.isPending}
                      onClick={() => removeMember.mutate(grant.user_id)}
                      aria-label={t("actionRemoveMember")}
                    >
                      <XIcon />
                    </Button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-xs text-muted-foreground">{t("projectMembersEmpty")}</p>
            )}
            <div className="flex flex-wrap items-end gap-2">
              <div className="min-w-0 flex-1">
                <LinkitUserPicker
                  lang={lang}
                  label={t("projectMemberSearchLabel")}
                  value={memberId}
                  onValueChange={(userId) => setMemberId(userId)}
                />
              </div>
              <Button
                size="sm"
                variant="outline"
                disabled={!memberId.trim() || addMember.isPending || memberInvalid}
                onClick={() => addMember.mutate()}
              >
                <UserPlusIcon />
                {t("actionAddMember")}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">{t("projectMemberSearchHint")}</p>
            {memberIsOwner ? (
              <p className="text-xs text-muted-foreground">{t("projectMemberSelf")}</p>
            ) : null}
            {memberAlreadyGranted ? (
              <p className="text-xs text-muted-foreground">{t("projectMemberAlready")}</p>
            ) : null}
            {addMember.isError ? (
              <p className="text-xs text-destructive">{(addMember.error as Error).message}</p>
            ) : null}
            {removeMember.isError ? (
              <p className="text-xs text-destructive">{(removeMember.error as Error).message}</p>
            ) : null}
            {grants.isError ? (
              <p className="text-xs text-destructive">{(grants.error as Error).message}</p>
            ) : null}
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

            {resolver.isError ? (
              <p className="text-xs text-destructive">{(resolver.error as Error).message}</p>
            ) : null}
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
