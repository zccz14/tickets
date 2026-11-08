import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { LinkitUserInfo, LinkitUserPicker } from "linkit-react-components"
import { UserPlusIcon, XIcon } from "lucide-react"
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
import type { Me, Project, ProjectGrant, ProjectVisibility } from "@/lib/api"
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
  const { t, lang } = useCopy()
  const queryClient = useQueryClient()
  const [name, setName] = useState(project.name)
  const [description, setDescription] = useState(project.description)
  const [visibility, setVisibility] = useState<ProjectVisibility>(project.visibility)
  const [memberId, setMemberId] = useState("")

  const me = useQuery({
    queryKey: ["me"],
    queryFn: () => request<Me>("/api/me"),
    enabled: open,
  })
  const grants = useQuery({
    queryKey: ["project-grants", project.id],
    queryFn: () =>
      request<{ grants: ProjectGrant[] }>(`/api/projects/${project.id}/grants`),
    enabled: open && project.is_owner,
  })

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["project", project.id] })
    void queryClient.invalidateQueries({ queryKey: ["projects"] })
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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {/* `!` 是必要的：@zccz14/cybion-components 自带一份预编译的 Tailwind
          工具类，排在本地 utilities 之后，会把同属性的 sm: 变体盖掉。 */}
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:!max-w-2xl">
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

          <div className="space-y-2 rounded-xl border border-border/70 bg-secondary/30 p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <p className="text-sm font-medium">{t("ticketSidebarResolverAgent")}</p>
                <p className="mt-1 text-xs text-muted-foreground">{t("resolverAgentHint")}</p>
              </div>
              <span
                className={
                  project.resolver_ready
                    ? "shrink-0 rounded-full bg-resolver/15 px-2 py-0.5 text-xs font-medium text-resolver"
                    : "shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground"
                }
              >
                {project.resolver_ready ? t("resolverReady") : t("resolverNotReady")}
              </span>
            </div>
            <p className="text-xs text-muted-foreground">
              {project.resolver_ready ? t("resolverReadyHint") : t("resolverNeedCybion")}
            </p>
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
