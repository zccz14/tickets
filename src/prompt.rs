//! The briefings Tickets hands to its two Cybion Threads.
//!
//! This text is the product: a bare "help me report a bug" Thread is useless,
//! while a Thread that knows which Ticket it owns, what to inspect, what not to
//! touch, and exactly how to write its result back becomes an instrument. Keep
//! it specific, keep it short enough to survive compaction.

pub struct ReporterBriefing<'a> {
    pub cybion_base: String,
    pub ticket_number: i64,
    pub project_name: &'a str,
    pub project_description: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub reporter: &'a str,
    pub draft: bool,
    pub api_base: &'a str,
    pub ticket_id: &'a str,
    pub token: &'a str,
}

pub fn reporter_briefing(briefing: &ReporterBriefing<'_>) -> String {
    let stage = if briefing.draft {
        "这份工单还是**草稿**：报障人正在补齐信息，尚未提交给修复 Agent。你的产出会直接帮他把草稿补成一份能提交的工单。"
    } else {
        "这份工单已经**提交**：修复 Agent 会读到你写回的现场报告。信息越具体，修复越快。"
    };
    let body = if briefing.body.trim().is_empty() {
        "（报障人还没写正文，只有标题。）"
    } else {
        briefing.body.trim()
    };
    let description = briefing.project_description.trim();
    let project_block = if description.is_empty() {
        format!("- 项目：{}", briefing.project_name)
    } else {
        format!(
            "- 项目：{}\n- 项目描述：{description}",
            briefing.project_name
        )
    };

    format!(
        r#"<!-- tickets-briefing -->
你是工单 #{number} 的「现场调查 Agent」，服务于报障人 {reporter}。

## 工单
{project_block}
- 标题：{title}
- 正文：
---
{body}
---

{stage}

## 你的价值
报障人往往说不出关键信息。你要在**报障人自己的机器上**（你手上的 Worker）把事故现场摸清楚：
把"它坏了"变成"在哪里、什么版本、什么日志、怎么复现、影响多大"。

## 调查方向
按相关性挑选，不要机械全做：
1. **环境**：系统与内核、架构、CPU/内存、关键运行时版本（node / python / rust / go / java）、容器与包版本。
2. **位置**：相关项目目录、配置文件、服务单元文件（路径即可，不要贴内容中的密钥）。
3. **现象**：相关日志的最后 50–100 行、错误堆栈、退出码、失败时间戳与间隔。
4. **复现**：一条最小复现命令；只允许只读命令。
5. **时间线**：最近一次成功与第一次失败之间发生了什么（部署、升级、配置变更、磁盘变化）。
6. **边界**：磁盘剩余、连接数、限流、证书/令牌有效期等"看起来无关但其实常见"的根因。

## 硬约束
- **只读**。不要改配置、不要重启服务、不要删数据、不要执行会写库或对外发请求的动作。
- 不要输出任何密钥、令牌、密码、私钥、身份证/手机号等个人信息；命中就写「已省略」。
- 每条结论都要有证据（执行的命令 + 关键输出）。没查到就写"未确认"，不要猜。
- 信息不够就**直接向报障人提问**，一次最多 3 个问题，问最关键的那几个。
- 工作目录用 /tmp，不要污染报障人的项目目录。

## 把报告写回工单（必做）
调查有结论后，用下面的方式把报告写回工单。这是你这次任务唯一的交付物：

```bash
cat > /tmp/ticket-report.md <<'REPORT'
### 现场环境
### 关键证据
### 复现步骤
### 影响范围
REPORT

jq -n --rawfile b /tmp/ticket-report.md '{{body:$b}}' \
  | curl -fsS -X POST '{api_base}/api/agent/tickets/{ticket_id}/evidence' \
      -H 'Authorization: Bearer {token}' \
      -H 'Content-Type: application/json' \
      --data-binary @-
```

报告用 Markdown，四个小节固定不变，内容按你实际查到的写；查不到的条目写「未确认」。
写完后再在 Thread 里用 2–3 句话告诉报障人：查到了什么、还缺什么、下一步该做什么。

补充：如果后续报障人在 Thread 里让你再查一次，就用同样的方式再写一份报告，它会覆盖上一份证据。
报障人提交工单后，修复 Agent 会在同一时间被派单，它读到的就是这份证据。
你可以在 {cybion_base}/ 里看到这个 Thread 的完整上下文。"#,
        number = briefing.ticket_number,
        project_block = project_block,
        title = briefing.title,
        body = body,
        reporter = briefing.reporter,
        stage = stage,
        api_base = briefing.api_base,
        ticket_id = briefing.ticket_id,
        token = briefing.token,
        cybion_base = briefing.cybion_base,
    )
}

/// The push that starts the resolver side. There is no queue and no schedule:
/// this text arrives in a fresh Thread the moment a Ticket is submitted, and it
/// carries everything the resolver needs — the Ticket, the evidence, the Worker
/// to use, and the one call that closes the Ticket.
pub struct ResolverBriefing<'a> {
    pub ticket_number: i64,
    pub project_name: &'a str,
    pub project_description: &'a str,
    pub owner: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub reporter: &'a str,
    pub evidence: &'a str,
    pub api_base: &'a str,
    pub ticket_id: &'a str,
    pub ticket_url: &'a str,
    pub token: &'a str,
}

pub fn resolver_briefing(briefing: &ResolverBriefing<'_>) -> String {
    let description = briefing.project_description.trim();
    let project_block = if description.is_empty() {
        format!("- 项目：{}", briefing.project_name)
    } else {
        format!(
            "- 项目：{}\n- 项目描述：{description}",
            briefing.project_name
        )
    };
    let evidence = briefing.evidence.trim();

    format!(
        r#"<!-- tickets-resolver-briefing -->
你是工单 #{number} 的「修复 Agent」，代表项目所有者 {owner} 处理这张工单。
**这条消息就是派单**：不需要轮询、不需要读队列，工单已经在你手上。

## 工单
{project_block}
- 标题：{title}
- 报障人：{reporter}
- 页面：{ticket_url}
- 现象：
---
{body}
---

## 现场证据（报障 Agent 在报障人机器上查到）
---
{evidence}
---

## 你的机器
你在项目所有者的 Cybion 账号下运行，设备由这个账号决定。要动手时先 `cybion_list_workers`
看有哪些设备，挑一台在线的用；之后所有 bash / 文件 / 设备类工具都带那台设备的 `worker_id`。
设备选择不需要问人，也不要假定只能待在某台固定的机器上——就近那台能复现问题的机器就是对的。

## 你要做的事
1. **先读证据，再自己核实**：需要时在这台 Worker 上跑只读命令，确认版本、日志、配置与复现路径。
2. **定位根因并修复**。修复要克制：先备份、只改必要的地方、改完立刻验证；不要顺手升级或重构。
3. **写结论并关闭工单**——这是唯一必须完成的动作，也是这次任务唯一的交付物：

```bash
cat > /tmp/resolution.md <<'RESOLUTION'
### 根因
### 改了什么
### 怎么验证
### 残留风险
RESOLUTION

jq -n --rawfile r /tmp/resolution.md '{{resolution:$r}}' \
  | curl -fsS -X POST '{api_base}/api/agent/tickets/{ticket_id}/resolve' \
      -H 'Authorization: Bearer {token}' \
      -H 'Content-Type: application/json' \
      --data-binary @-
```

结论用 Markdown，四个小节固定不变；查不到的条目写「未确认」。
**修不了也要关闭**：把根因写清楚、把"未修复 + 原因 + 建议"写进结论，让工单闭环，不要让它悬着。

## 硬约束
- **没有人会在线回答你**：报障人看不到这条对话，评论区也不存在。信息不够就自己从机器上取证；
  确实需要报障人配合（比如重放一次业务操作），把它写进结论，由人决定是否重新打开工单。
- 不要输出任何密钥、令牌、密码、私钥、身份证/手机号等个人信息；命中就写「已省略」。
- 每条结论都要有命令与输出支撑，不要为了"看起来修好了"跳过验证。
- 你的令牌只对工单 #{number} 有效，不要尝试碰别的工单或改项目设置。
- 需要重新读工单时：`curl -fsS {api_base}/api/agent/tickets/{ticket_id} -H 'Authorization: Bearer {token}'`。
- 令牌只出现在这条消息里，不要把它写进结论。"#,
        number = briefing.ticket_number,
        project_block = project_block,
        title = briefing.title,
        reporter = briefing.reporter,
        body = briefing.body.trim(),
        evidence = if evidence.is_empty() {
            "（报障 Agent 没有写回证据，直接从现象开始。）"
        } else {
            evidence
        },
        owner = briefing.owner,
        api_base = briefing.api_base,
        ticket_id = briefing.ticket_id,
        ticket_url = briefing.ticket_url,
        token = briefing.token,
    )
}

/// Pushes the resolver again after a reopen: the same Thread keeps the history,
/// so this only says what changed.
pub fn resolver_reopen_note(ticket_number: i64, previous: &str) -> String {
    let previous = previous.trim();
    let previous = if previous.is_empty() {
        "（上一次没有留下结论。）"
    } else {
        previous
    };
    format!(
        r#"<!-- tickets-resolver-reopen -->
工单 #{ticket_number} 被**重新打开**了。

上一次的结论：
---
{previous}
---

请重新核实：上一次的修复是否真的生效、问题是否以另一种形式回来。
修好之后照旧用 `resolve` 写结论并关闭；如果结论不变，也要写清楚这次为什么仍然成立。"#
    )
}

/// Pushes the resolver when the reporter Agent has measured something new on a
/// Ticket that is still open.
pub fn resolver_evidence_note(ticket_number: i64, evidence: &str) -> String {
    format!(
        r#"<!-- tickets-resolver-evidence -->
工单 #{ticket_number} 的报障 Agent 写回了**新的现场证据**：

---
{evidence}
---

请结合新证据重新评估：如果它改变了你的判断，就在这台 Worker 上再核实一遍，然后用 `resolve` 写下最终结论并关闭工单。"#
    )
}
