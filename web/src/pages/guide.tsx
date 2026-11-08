import { ArrowRightIcon, BotIcon, SparklesIcon, TerminalIcon } from "lucide-react"

import { CodeBlock, PageHeader } from "@/components/bits"
import { useCopy } from "@/lib/session"

const API_ROWS = [
  ["GET", "/api/agent/whoami", "确认令牌身份与绑定的工单"],
  ["GET", "/api/agent/tickets/{id}", "读取工单全文（正文 + 现场证据 + 处理结论）"],
  ["POST", "/api/agent/tickets/{id}/evidence", "报障 Agent 写回现场证据"],
  ["POST", "/api/agent/tickets/{id}/resolve", "修复 Agent 写回处理结论并关闭工单"],
]

const FLOW = [
  ["报障人", "写一句哪里不对，工单以草稿开始。"],
  ["报障 Agent", "在报障人自己的机器上只读排查，把现场证据写回工单。"],
  ["提交", "报障人确认信息齐了，点提交。"],
  ["修复 Agent", "Tickets 当场把工单推进项目所有者的修复 Thread，不需要轮询。"],
  ["Resolve & Close", "修复 Agent 修完写下结论，工单在同一个调用里关闭。"],
]

export function GuidePage() {
  const { t } = useCopy()
  return (
    <div className="mx-auto w-full max-w-4xl space-y-6">
      <PageHeader description={t("guideIntro")} />

      <div className="grid gap-3 md:grid-cols-2">
        <section className="space-y-2 rounded-xl border border-reporter/25 bg-reporter/[0.04] p-5">
          <div className="flex items-center gap-2">
            <span className="flex size-6 items-center justify-center rounded-md bg-reporter/15 text-reporter">
              <SparklesIcon className="size-3.5" />
            </span>
            <h2 className="text-sm font-medium">{t("guideReporterTitle")}</h2>
          </div>
          <p className="text-sm text-muted-foreground">{t("guideReporterBody")}</p>
          <p className="text-xs text-muted-foreground">{t("guideReporterScope")}</p>
        </section>

        <section className="space-y-2 rounded-xl border border-resolver/25 bg-resolver/[0.04] p-5">
          <div className="flex items-center gap-2">
            <span className="flex size-6 items-center justify-center rounded-md bg-resolver/15 text-resolver">
              <BotIcon className="size-3.5" />
            </span>
            <h2 className="text-sm font-medium">{t("guideResolverTitle")}</h2>
          </div>
          <p className="text-sm text-muted-foreground">{t("guideResolverBody")}</p>
          <p className="text-xs text-muted-foreground">{t("guideResolverScope")}</p>
        </section>
      </div>

      <section className="space-y-3 rounded-xl border border-border/70 bg-card p-5">
        <h2 className="text-sm font-medium">{t("guideFlowTitle")}</h2>
        <ol className="space-y-2">
          {FLOW.map(([actor, body]) => (
            <li key={actor} className="flex flex-wrap items-baseline gap-2 text-sm">
              <span className="inline-flex shrink-0 items-center gap-1 rounded-md border border-border/70 bg-secondary/50 px-2 py-0.5 text-xs font-medium">
                {actor}
              </span>
              <ArrowRightIcon className="size-3 shrink-0 text-muted-foreground" />
              <span className="min-w-0 text-muted-foreground">{body}</span>
            </li>
          ))}
        </ol>
        <p className="text-xs text-muted-foreground">{t("guideFlowNote")}</p>
      </section>

      <section className="space-y-3 rounded-xl border border-border/70 bg-card p-5">
        <div className="flex items-center gap-2">
          <TerminalIcon className="size-4 text-muted-foreground" />
          <h2 className="text-sm font-medium">Agent API</h2>
        </div>
        <div className="overflow-hidden rounded-lg border border-border/70">
          <table className="w-full text-left text-xs">
            <thead className="bg-secondary/60 text-muted-foreground">
              <tr>
                <th className="px-3 py-2 font-medium">Method</th>
                <th className="px-3 py-2 font-medium">Path</th>
                <th className="px-3 py-2 font-medium">用途</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border/60">
              {API_ROWS.map(([method, path, purpose]) => (
                <tr key={path}>
                  <td className="px-3 py-2 font-mono text-[11px] text-primary">{method}</td>
                  <td className="px-3 py-2 font-mono text-[11px]">{path}</td>
                  <td className="px-3 py-2 text-muted-foreground">{purpose}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <CodeBlock
          code={`curl -sS https://tickets.ntnl.io/api/agent/tickets/<id> \\
  -H "Authorization: Bearer tk_..."`}
        />
        <p className="text-xs text-muted-foreground">{t("guideTokenNote")}</p>
      </section>
    </div>
  )
}
