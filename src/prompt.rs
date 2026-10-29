//! The briefing the Tickets app hands to a Cybion Thread.
//!
//! This text is the product: a bare "help me report a bug" Thread is useless,
//! while a Thread that knows which Ticket it owns, what to inspect, what not to
//! touch, and exactly how to write its findings back becomes the reporter's
//! instrument. Keep it specific, keep it short enough to survive compaction.

pub struct ReporterBriefing<'a> {
    pub cybion_base: String,
    pub ticket_number: i64,
    pub project_name: &'a str,
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

    format!(
        r#"你是工单 #{number} 的「现场调查 Agent」，服务于报障人 {reporter}。

## 工单
- 项目：{project}
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

jq -n --rawfile b /tmp/ticket-report.md '{{kind:"evidence", body:$b}}' \
  | curl -fsS -X POST '{api_base}/api/agent/tickets/{ticket_id}/comments' \
      -H 'Authorization: Bearer {token}' \
      -H 'Content-Type: application/json' \
      --data-binary @-
```

报告用 Markdown，四个小节固定不变，内容按你实际查到的写；查不到的条目写「未确认」。
写完后再在 Thread 里用 2–3 句话告诉报障人：查到了什么、还缺什么、下一步该做什么。

补充：如果后续报障人在 Thread 里让你再查一次，就用同样的方式再写一份报告（会作为新的证据追加到工单）。
你可以在 {cybion_base}/ 里看到这个 Thread 的完整上下文。"#,
        number = briefing.ticket_number,
        project = briefing.project_name,
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

/// The intent shown next to the resolver's Ultimate Machine.
pub fn resolver_intent(project: &str) -> String {
    format!(
        "确保 tickets.ntnl.io 的项目「{project}」没有等待处理的 Open 工单：\
检查命令返回非 0 时，读队列里的工单，逐个诊断、通过工单 API 回复，并关闭已解决的工单。"
    )
}

/// The briefing a resolver agent gets when it opens a ticket through the API.
pub fn resolver_hint() -> &'static str {
    "排障优先只读；需要报障人补充信息时用 ctl comment 提问并保持 Open；\
确认解决后用 ctl close 关闭。每次只处理一个工单，处理完再处理下一个。"
}
