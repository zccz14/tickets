import { useMutation, useQueryClient } from "@tanstack/react-query"
import { KeyRoundIcon, RotateCcwIcon, Trash2Icon } from "lucide-react"
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
import { CodeBlock, CopyButton } from "@/components/bits"
import type { Project, ResolverKey } from "@/lib/api"
import { resolverCommand } from "@/lib/machine"
import { useCopy, useSession } from "@/lib/session"

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
  const { t } = useCopy()
  const queryClient = useQueryClient()
  const [name, setName] = useState(project.name)
  const [description, setDescription] = useState(project.description)
  const [threadUrl, setThreadUrl] = useState(project.resolver_thread_url ?? "")
  const [issued, setIssued] = useState<ResolverKey | null>(null)

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["project", project.id] })
    void queryClient.invalidateQueries({ queryKey: ["projects"] })
  }

  const save = useMutation({
    mutationFn: () =>
      request<Project>(`/api/projects/${project.id}`, {
        method: "PATCH",
        body: JSON.stringify({
          name,
          description,
          resolver_thread_url: threadUrl,
        }),
      }),
    onSuccess: () => {
      refresh()
      onOpenChange(false)
    },
  })

  const issueKey = useMutation({
    mutationFn: () =>
      request<ResolverKey>(`/api/projects/${project.id}/resolver-key`, { method: "POST" }),
    onSuccess: (key) => {
      setIssued(key)
      refresh()
    },
  })

  const revokeKey = useMutation({
    mutationFn: () =>
      request<unknown>(`/api/projects/${project.id}/resolver-key`, { method: "DELETE" }),
    onSuccess: () => {
      setIssued(null)
      refresh()
    },
  })

  const base = window.location.origin

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("actionManaging")}</DialogTitle>
          <DialogDescription>{t("projectsDescription")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-5">
          <div className="space-y-1.5">
            <Label htmlFor="settings-name">{t("projectsTitle")}</Label>
            <Input id="settings-name" value={name} onChange={(event) => setName(event.target.value)} />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="settings-description">{t("ticketBodyLabel")}</Label>
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
                  project.resolver_ready
                    ? "shrink-0 rounded-full bg-resolver/15 px-2 py-0.5 text-xs font-medium text-resolver"
                    : "shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground"
                }
              >
                {project.resolver_ready ? t("settingsCybionConnected") : t("settingsCybionMissing")}
              </span>
            </div>

            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                variant={project.resolver_ready ? "outline" : "default"}
                disabled={issueKey.isPending}
                onClick={() => issueKey.mutate()}
              >
                <KeyRoundIcon />
                {project.resolver_ready ? t("actionGenerateKey") : t("actionGenerateKey")}
              </Button>
              {project.resolver_ready ? (
                <Button
                  size="sm"
                  variant="destructive"
                  disabled={revokeKey.isPending}
                  onClick={() => revokeKey.mutate()}
                >
                  <Trash2Icon />
                  {t("actionRevokeKey")}
                </Button>
              ) : null}
            </div>

            {issued ? (
              <div className="space-y-2">
                <p className="text-xs font-medium text-primary">
                  密钥只显示这一次，请立刻复制到 Cybion 终极机器的检查命令里。
                </p>
                <CodeBlock code={resolverCommand(base, issued.token, project.name)} />
                <div className="flex flex-wrap items-center gap-2">
                  <CopyButton
                    value={resolverCommand(base, issued.token, project.name)}
                    label={t("actionCopy")}
                  />
                  <span className="text-xs text-muted-foreground">
                    机器名建议：<code className="font-mono">工单修复 · {project.name}</code>，间隔 120 秒。
                  </span>
                </div>
              </div>
            ) : null}

            <div className="space-y-1.5">
              <Label htmlFor="resolver-thread">{t("resolverThreadLabel")}</Label>
              <Input
                id="resolver-thread"
                value={threadUrl}
                onChange={(event) => setThreadUrl(event.target.value)}
                placeholder="https://cybion.ntnl.io/#/threads/<id>"
              />
              <p className="text-xs text-muted-foreground">
                把终极机器的 Thread 链接贴到这里，工单页就能直接跳到修复 Agent 的现场。
              </p>
            </div>
          </div>

          {save.isError ? (
            <p className="text-sm text-destructive">{(save.error as Error).message}</p>
          ) : null}
          {issueKey.isError ? (
            <p className="text-sm text-destructive">{(issueKey.error as Error).message}</p>
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

export function RotateIcon() {
  return <RotateCcwIcon />
}
