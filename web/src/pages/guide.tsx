import { BotIcon, SparklesIcon, TerminalIcon } from "lucide-react"

import { CodeBlock, PageHeader } from "@/components/bits"
import { useCopy } from "@/lib/session"

const API_ROWS = [
  ["GET", "/api/agent/queue", "修复 Agent 读取待处理工单"],
  ["GET", "/api/agent/tickets/{id}", "读取工单全文（正文 + 全部评论）"],
  ["POST", "/api/agent/tickets/{id}/comments", "追加评论，报障 Agent 可用 kind=evidence"],
  ["POST", "/api/agent/tickets/{id}/close", "关闭工单"],
  ["GET", "/api/agent/whoami", "确认令牌身份与作用域"],
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
          <p className="text-xs text-muted-foreground">
            令牌作用域：<code className="font-mono">ticket:{"{id}"}</code>，只能读自己那张工单、只能追加评论。
          </p>
        </section>

        <section className="space-y-2 rounded-xl border border-resolver/25 bg-resolver/[0.04] p-5">
          <div className="flex items-center gap-2">
            <span className="flex size-6 items-center justify-center rounded-md bg-resolver/15 text-resolver">
              <BotIcon className="size-3.5" />
            </span>
            <h2 className="text-sm font-medium">{t("guideResolverTitle")}</h2>
          </div>
          <p className="text-sm text-muted-foreground">{t("guideResolverBody")}</p>
          <p className="text-xs text-muted-foreground">
            令牌作用域：<code className="font-mono">project:{"{id}"}</code>，只能碰本项目、只能碰 Open 工单。
          </p>
        </section>
      </div>

      <section className="space-y-3 rounded-xl border border-border/70 bg-card p-5">
        <div className="flex items-center gap-2">
          <TerminalIcon className="size-4 text-muted-foreground" />
          <h2 className="text-sm font-medium">修复 Agent 的四步接入</h2>
        </div>
        <ol className="list-decimal space-y-2 pl-5 text-sm text-muted-foreground">
          <li>
            在项目 → <span className="text-foreground">项目设置</span> → 生成修复 Agent 密钥。
          </li>
          <li>在 Cybion 的 <span className="text-foreground">终极机器</span> 页面新建一台机器，粘上生成的检查命令。</li>
          <li>机器名建议 <code className="font-mono">工单修复 · 项目名</code>，间隔 120 秒，意图写清楚"确保没有待处理工单"。</li>
          <li>把机器的 Thread 链接贴回项目设置，工单页就能一眼跳到修复现场。</li>
        </ol>
        <p className="text-xs text-muted-foreground">
          检查命令是幂等的：队列为空 → 退出码 0 → 机器保持沉默；队列有工单 → 打印队列并退出码 1 → 终极机器唤醒它的 Thread。
          机器人在 Thread 里用 <code className="font-mono">~/.tickets/ctl</code> 读单、回复、关闭，然后机器立刻复查。
        </p>
      </section>

      <section className="space-y-3 rounded-xl border border-border/70 bg-card p-5">
        <h2 className="text-sm font-medium">Agent API</h2>
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
          code={`curl -sS https://tickets.ntnl.io/api/agent/queue \\
  -H "Authorization: Bearer tk_..."`}
        />
        <p className="text-xs text-muted-foreground">{t("guideTokenNote")}</p>
      </section>
    </div>
  )
}
