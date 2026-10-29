#!/usr/bin/env bash
# Sends the release archive's deploy step to the production instance over SSM.
#
# Args: <instance-id> <release-tag>; env: AWS_DEPLOY_BUCKET.
set -euo pipefail

instance_id="${1:?EC2 instance ID is required}"
tag="${2:?release tag is required}"
bucket="${AWS_DEPLOY_BUCKET:?AWS_DEPLOY_BUCKET is required}"

script_base64="$(base64 < deploy/remote-deploy.sh | tr -d '\n')"
printf -v quoted_tag '%q' "$tag"
printf -v quoted_bucket '%q' "$bucket"
install_command="printf '%s' '$script_base64' | base64 -d > /tmp/tickets-deploy.sh"
run_command="BUCKET=$quoted_bucket bash /tmp/tickets-deploy.sh $quoted_tag"
parameters="$(jq -cn \
  --arg install "$install_command" \
  --arg run "$run_command" \
  '{commands:[$install,$run],executionTimeout:["900"]}')"

command_id="$(aws ssm send-command \
  --instance-ids "$instance_id" \
  --document-name AWS-RunShellScript \
  --comment "Deploy tickets $tag" \
  --parameters "$parameters" \
  --query 'Command.CommandId' \
  --output text)"
echo "SSM command ${command_id} sent for ${tag}"

status="InProgress"
for _ in $(seq 1 100); do
  status="$(aws ssm get-command-invocation \
    --command-id "$command_id" \
    --instance-id "$instance_id" \
    --query Status \
    --output text 2>/dev/null || echo InProgress)"
  case "$status" in
    Success) break ;;
    InProgress|Pending|Delayed) sleep 5 ;;
    *) break ;;
  esac
done

result="$(aws ssm get-command-invocation \
  --command-id "$command_id" \
  --instance-id "$instance_id" \
  --query '{Status:Status,StandardOutput:StandardOutputContent,StandardError:StandardErrorContent}' \
  --output json)"
printf '%s\n' "$result" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("::group::SSM stdout"); print(d.get("StandardOutput") or ""); print("::endgroup::"); print("::group::SSM stderr"); print(d.get("StandardError") or ""); print("::endgroup::"); print("Status:", d.get("Status"))'
resolved="$(printf '%s' "$result" | python3 -c 'import json,sys; print(json.load(sys.stdin)["Status"])')"
test "$resolved" = "Success"
