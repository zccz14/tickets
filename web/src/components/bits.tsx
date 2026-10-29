import { LinkitUserInfo } from "linkit-react-components"
import { GlobeIcon, LockIcon } from "lucide-react"
import type { ReactNode } from "react"

import { useCopy } from "@/lib/session"
import { cn } from "@/lib/utils"
import type { Comment, ProjectVisibility } from "@/lib/api"

export function VisibilityBadge({
  visibility,
  className,
}: {
  visibility: ProjectVisibility
  className?: string
}) {
  const { t } = useCopy()
  const Icon = visibility === "private" ? LockIcon : GlobeIcon
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-medium",
        visibility === "private"
          ? "border-border bg-muted/60 text-muted-foreground"
          : "border-primary/25 bg-primary/8 text-primary",
        className,
      )}
    >
      <Icon className="size-2.5" />
      {visibility === "private" ? t("projectVisibilityPrivate") : t("projectVisibilityPublic")}
    </span>
  )
}

export function StatusBadge({ status }: { status: "draft" | "open" | "closed" }) {
  const { t } = useCopy()
  const label = status === "open" ? t("stateOpen") : status === "closed" ? t("stateClosed") : t("stateDraft")
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium",
        status === "open" && "border-primary/35 bg-primary/10 text-primary",
        status === "closed" && "border-border bg-muted text-muted-foreground",
        status === "draft" && "border-dashed border-border bg-transparent text-muted-foreground",
      )}
    >
      <span
        className={cn(
          "size-1.5 rounded-full",
          status === "open" ? "bg-primary" : status === "closed" ? "bg-muted-foreground/60" : "bg-transparent ring-1 ring-muted-foreground/60",
        )}
      />
      {label}
    </span>
  )
}

const AUTHOR_STYLE: Record<Comment["author_kind"], { label: string; className: string }> = {
  user: { label: "", className: "" },
  reporter_agent: {
    label: "reporting-agent",
    className: "border-reporter/30 bg-reporter/10 text-reporter",
  },
  resolver_agent: {
    label: "resolver-agent",
    className: "border-resolver/30 bg-resolver/10 text-resolver",
  },
  system: { label: "", className: "" },
}

export function AuthorTag({ kind }: { kind: Comment["author_kind"] }) {
  const style = AUTHOR_STYLE[kind]
  if (!style.label) return null
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-1.5 py-0.5 font-mono text-[10px] tracking-wide uppercase",
        style.className,
      )}
    >
      {style.label}
    </span>
  )
}

export function AuthorName({
  kind,
  userId,
  name,
}: {
  kind: Comment["author_kind"]
  userId: string | null
  name: string | null
}) {
  const { t } = useCopy()
  if (kind === "reporter_agent") return <span className="font-medium">报障 Agent</span>
  if (kind === "resolver_agent") return <span className="font-medium">修复 Agent</span>
  if (kind === "system") return <span className="font-medium">{name ?? "系统"}</span>
  // Linkit owns how a person is shown: avatar, username and the profile popup.
  // The server-side display name only stands in when there is no user behind
  // the comment at all.
  if (userId) return <LinkitUserInfo userId={userId} compact />
  return <span className="font-medium">{name ?? t("ticketReporter")}</span>
}

/** Page-level header used by every non-shell page. */
export function PageHeader({
  title,
  description,
  actions,
  breadcrumb,
}: {
  /** Omit on pages whose name is already the shell header's title. */
  title?: ReactNode
  description?: ReactNode
  actions?: ReactNode
  breadcrumb?: ReactNode
}) {
  return (
    <div className="flex flex-col gap-3 border-b border-border/70 pb-5">
      {breadcrumb}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 space-y-1">
          {title ? (
            <h1 className="text-xl font-medium tracking-tight text-foreground">{title}</h1>
          ) : null}
          {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
        </div>
        {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
      </div>
    </div>
  )
}

export function EmptyState({ title, hint, action }: { title: string; hint?: string; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border/80 bg-card/40 px-6 py-14 text-center">
      <p className="text-sm font-medium text-foreground">{title}</p>
      {hint ? <p className="max-w-md text-sm text-muted-foreground">{hint}</p> : null}
      {action}
    </div>
  )
}

export function CopyButton({ value, label }: { value: string; label?: string }) {
  const { t } = useCopy()
  return (
    <button
      type="button"
      className="inline-flex items-center rounded-md border border-border bg-background px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
      onClick={() => void navigator.clipboard.writeText(value)}
    >
      {label ?? t("actionCopy")}
    </button>
  )
}

export function CodeBlock({ code }: { code: string }) {
  return (
    <pre className="max-h-80 overflow-auto rounded-lg border border-border/70 bg-secondary/70 p-3 text-[11px] leading-relaxed">
      <code className="whitespace-pre-wrap break-all font-mono">{code}</code>
    </pre>
  )
}
