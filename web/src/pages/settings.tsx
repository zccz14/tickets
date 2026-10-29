import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { CheckCircle2Icon, LinkIcon, UnplugIcon } from "lucide-react"
import { LinkitUserInfo } from "linkit-react-components"
import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { PageHeader } from "@/components/bits"
import type { Me } from "@/lib/api"
import { useCopy, useSession } from "@/lib/session"

export function SettingsPage() {
  const { request } = useSession()
  const { t } = useCopy()
  const queryClient = useQueryClient()
  const [apiKey, setApiKey] = useState("")

  const me = useQuery({ queryKey: ["me"], queryFn: () => request<Me>("/api/me") })

  const connect = useMutation({
    mutationFn: () =>
      request<Me>("/api/account/cybion", {
        method: "PUT",
        body: JSON.stringify({ api_key: apiKey }),
      }),
    onSuccess: () => {
      setApiKey("")
      void queryClient.invalidateQueries({ queryKey: ["me"] })
    },
  })
  const disconnect = useMutation({
    mutationFn: () => request<unknown>("/api/account/cybion", { method: "DELETE" }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["me"] }),
  })

  const data = me.data

  return (
    <div className="mx-auto w-full max-w-3xl space-y-6">
      <PageHeader description={t("settingsCybionDescription")} />

      <section className="space-y-4 rounded-xl border border-border/70 bg-card p-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h2 className="text-sm font-medium">{t("settingsCybionTitle")}</h2>
            <p className="mt-1 text-xs text-muted-foreground">{t("settingsCybionDescription")}</p>
          </div>
          <span
            className={
              data?.cybion_connected
                ? "inline-flex shrink-0 items-center gap-1 rounded-full bg-resolver/12 px-2.5 py-1 text-xs font-medium text-resolver"
                : "inline-flex shrink-0 items-center gap-1 rounded-full bg-muted px-2.5 py-1 text-xs text-muted-foreground"
            }
          >
            {data?.cybion_connected ? (
              <>
                <CheckCircle2Icon className="size-3.5" />
                {t("settingsCybionConnected")}
              </>
            ) : (
              t("settingsCybionMissing")
            )}
          </span>
        </div>

        {data?.cybion_connected ? (
          <div className="space-y-3">
            <div className="flex items-center gap-2 rounded-lg border border-border/70 bg-secondary/40 px-3 py-2">
              <LinkIcon className="size-3.5 text-muted-foreground" />
              <span className="text-xs text-muted-foreground">{t("settingsKeyStored")}</span>
              <code className="font-mono text-xs text-foreground">{data.cybion_key_hint}</code>
            </div>
            {data.cybion_last_error ? (
              <p className="text-xs text-destructive">{data.cybion_last_error}</p>
            ) : null}
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={disconnect.isPending}
                onClick={() => disconnect.mutate()}
              >
                <UnplugIcon />
                {t("actionDisconnect")}
              </Button>
            </div>
          </div>
        ) : (
          <form
            className="space-y-3"
            onSubmit={(event) => {
              event.preventDefault()
              if (apiKey.trim()) connect.mutate()
            }}
          >
            <div className="space-y-1.5">
              <Label htmlFor="cybion-key">{t("settingsKeyLabel")}</Label>
              <Input
                id="cybion-key"
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={apiKey}
                onChange={(event) => setApiKey(event.target.value)}
                placeholder={t("settingsKeyPlaceholder")}
                className="font-mono text-xs"
              />
              <p className="text-xs text-muted-foreground">{t("settingsKeyHint")}</p>
            </div>
            {connect.isError ? (
              <p className="text-sm text-destructive">{(connect.error as Error).message}</p>
            ) : null}
            <Button type="submit" disabled={!apiKey.trim() || connect.isPending}>
              {t("actionConnect")}
            </Button>
          </form>
        )}
      </section>

      <section className="space-y-3 rounded-xl border border-border/70 bg-card p-5">
        <h2 className="text-sm font-medium">{t("settingsIdentity")}</h2>
        <dl className="grid gap-3 sm:grid-cols-2">
          <div>
            <dt>Auth Mini</dt>
            <dd className="mt-0.5 font-mono text-xs break-all">{data?.id ?? "—"}</dd>
          </div>
          <div>
            <dt>{t("settingsLinkit")}</dt>
            <dd className="mt-0.5 text-xs">
              {data ? <LinkitUserInfo userId={data.id} compact /> : "—"}
            </dd>
          </div>
        </dl>
        <p className="text-xs text-muted-foreground">{t("settingsLinkedAccountBody")}</p>
      </section>
    </div>
  )
}
