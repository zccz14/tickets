/**
 * The resolver's Ultimate Machine check command.
 *
 * Cybion runs one bash command on a Worker every interval and stays silent
 * while it exits 0. This command therefore *is* the resolver's trigger: it
 * installs a tiny `ctl` helper plus the project key on the machine (mode 0600,
 * so the token never lands in Thread history), then exits non-zero with the
 * pending queue on stdout when there is work.
 */
/** Single-quotes a value for bash, so a project name may contain anything. */
export function shellValue(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`
}

export function resolverCommand(base: string, key: string, projectName: string): string {
  return `B=${shellValue(base)}
K=${shellValue(key)}
P=${shellValue(projectName)}
D="$HOME/.tickets"
command -v jq >/dev/null || { echo "缺少 jq，请先安装 jq 再启用该机器"; exit 2; }
mkdir -p "$D" && printf '%s' "$K" > "$D/key" && chmod 600 "$D/key"
cat > "$D/ctl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
B=${shellValue(base)}
K=$(cat "$HOME/.tickets/key")
AUTH=(-H "Authorization: Bearer $K")
JSON=(-H 'Content-Type: application/json')
case "\${1:-}" in
  queue)   curl -fsS "\${AUTH[@]}" "$B/api/agent/queue" ;;
  show)    curl -fsS "\${AUTH[@]}" "$B/api/agent/tickets/\${2:?用法: ctl show <id>}" ;;
  comment) jq -Rs '{body:.}' < "\${2:?用法: ctl comment <文件> <id>}" \\
             | curl -fsS -X POST "\${AUTH[@]}" "\${JSON[@]}" --data-binary @- \\
                 "$B/api/agent/tickets/\${3:?用法: ctl comment <文件> <id>}/comments" ;;
  close)   curl -fsS -X POST "\${AUTH[@]}" "\${JSON[@]}" \\
             -d "{\\"body\\":\\"已修复并验证\\"}" \\
             "$B/api/agent/tickets/\${2:?用法: ctl close <id>}/close" ;;
  *) echo "用法: ctl queue | show <id> | comment <文件> <id> | close <id>" >&2; exit 2 ;;
esac
SH
chmod +x "$D/ctl"
R=$(curl -fsS -H "Authorization: Bearer $K" "$B/api/agent/queue") || { echo "tickets 服务不可达: $B"; exit 2; }
N=$(printf '%s' "$R" | jq -r '.pending|length')
if [ "$N" = "0" ]; then exit 0; fi
printf '%s' "$R" | jq -r '.pending[] | "● #\\(.number) \\(.title)\\n  项目: \\(.project)  报障人: \\(.reporter)  评论: \\(.comments)\\n  链接: \\(.url)\\n  现象: \\(.summary)\\n  最新证据: \\(.evidence // "（暂无）")\\n"'
echo "—— 共 $N 张 Open 工单（项目「$P」）等待处理。"
echo "用 $D/ctl show <id> 读全文，ctl comment <文件> <id> 回复，ctl close <id> 关闭。"
echo "需要报障人补充信息时用 ctl comment 提问并保持 Open；确认解决后关闭。"
exit 1`
}
