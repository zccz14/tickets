/**
 * Flat copy dictionary. Two languages, one file, no lazy loading: a missing key
 * is a TypeScript error rather than a blank in production.
 */
export const copy = {
  zh: {
    appName: "Tickets",
    navWorkbench: "工作台",
    navProjects: "项目",
    navMyTickets: "我的工单",
    navAgent: "接入说明",
    navAgentGroup: "Agent",
    navAccount: "账户",
    navSettings: "设置",

    actionAddMember: "添加成员",
    actionCancel: "取消",
    actionClose: "关闭工单",
    actionConnect: "连接 Cybion",
    actionCopy: "复制",
    actionCreateProject: "新建项目",
    actionCreateThread: "创建调查 Thread",
    actionCreateTicket: "新建工单",
    actionDisconnect: "断开连接",
    actionDispatch: "重新派发",
    actionManaging: "项目设置",
    actionOpenThread: "在 Cybion 打开",
    actionRemoveMember: "移除",
    actionReopen: "重新打开",
    actionRetry: "重试",
    actionSave: "保存",
    actionStartInvestigation: "开始调查",
    actionSubmit: "提交工单",

    stateOpen: "Open",
    stateClosed: "Closed",
    stateDraft: "草稿",
    stateAll: "全部",
    projectsDescription:
      "每个项目是一个工单队列。私有项目只有你和授权成员能看到；公开项目所有登录用户都能看到并提工单。",
    projectsEmpty: "还没有项目",
    projectsEmptyHint:
      "创建一个项目，就可以开始收工单了。提交的工单会直接推给项目所有者的修复 Agent。",
    projectOpenCount: "{n} 个待处理",
    projectTotalCount: "{n} 个工单",
    projectNameLabel: "项目名称",
    projectDescriptionLabel: "项目描述",
    projectVisibilityLabel: "可见性",
    projectVisibilityPrivate: "私有",
    projectVisibilityPublic: "公开",
    projectVisibilityPrivateHint: "只有你和授权成员能看到这个项目、它的工单和现场证据。",
    projectVisibilityPublicHint: "所有登录用户都能看到项目、浏览工单并提交新工单。",
    projectMembersLabel: "授权成员",
    projectMembersHint: "私有项目对这些成员开放。修复 Agent 仍然只属于项目所有者。",
    projectMembersEmpty: "还没有授权成员。",
    projectMemberSearchLabel: "查找用户",
    projectMemberSearchHint: "按用户名或 UUID 搜索 Linkit 用户；对方无需先登录过 Tickets。",
    projectMemberSelf: "项目所有者本来就能访问。",
    projectMemberAlready: "该用户已经在成员列表里。",
    projectMemberBadge: "已授权",

    ticketsEmptyOpen: "没有待处理的工单",
    ticketsEmptyClosed: "还没有关闭的工单",
    ticketsEmptyAll: "还没有工单",
    ticketsEmptyDraft: "没有草稿",
    ticketNewTitle: "新建工单",
    ticketNewHeading: "说说你遇到了什么问题",
    ticketNewHint:
      "发送后会自动创建草稿和现场调查 Thread，用你的 Cybion 账号在你这台机器上开始查。标题取自第一条消息。",
    ticketNewPlaceholder:
      "例如：今天 10 点开始，支付回调的成功率从 99% 掉到 93%，日志里是上游 30 秒超时。\n\n写得糙没关系，Agent 会在这里和你一起把现场补齐。",
    ticketNewStepsTitle: "接下来会发生什么",
    ticketNewStepOne: "这段描述成为工单正文，第一行成为标题。",
    ticketNewStepTwo: "你的 Cybion Thread 开始调查，过程直接显示在工单页里。",
    ticketNewStepThree: "你确认信息齐全后提交，工单当场推给修复 Agent。",
    ticketNewPrivacy: "提交前，这张工单和这段对话只有你能看到。",
    ticketDraftBanner:
      "这是草稿，只有你能看到。先在下面的现场调查里和 Agent 一起查清楚，补好信息后提交给修复 Agent。",
    ticketReporter: "报障人",
    ticketOpened: "创建于",
    ticketClosedBy: "由 {name} 关闭",
    ticketEvidenceTitle: "现场证据",
    ticketEvidenceMissing: "报障 Agent 还没有写回现场证据。",
    ticketEvidenceBadge: "证据",
    ticketResolutionTitle: "处理结论",
    ticketResolutionMissing: "修复 Agent 还没有写下结论。",
    ticketResolutionBadge: "结论",
    ticketSidebarReporterAgent: "现场调查 Thread",
    ticketSidebarResolverAgent: "修复 Agent",
    ticketSidebarAgentApi: "Agent 接口",

    threadMissing: "还没有现场调查 Thread",
    threadMissingHint: "这张工单还没有现场调查 Thread；它由报障人的 Cybion 账号创建。",
    threadMissingDraftHint: "草稿第一次打开时，调查 Agent 会自动开始，结果写回这张草稿。",
    threadMissingOpenHint: "提交时本应自动创建 Thread。点下面的按钮重试一次。",
    threadConnectHint: "还没连接 Cybion？先到设置里连接，再回来点一次。",
    threadTokens: "{n} tokens",
    threadOwnerNote: "这个 Thread 由报障人的 Cybion 账号持有，运行在报障人的机器上。只有报障人能打开它。",
    threadLiveTitle: "现场调查",
    threadLiveVisibility: "只有你能看到。和 Agent 一起查，确认后再提交。",
    threadLiveStarting: "正在创建调查 Thread…",
    threadLiveComposerHint: "Enter 发送 · Shift+Enter 换行",
    threadLiveClosed: "工单已关闭，现场调查不能再推进。",
    threadLiveFailed: "现场调查面板没能加载，刷新页面再试一次。",

    resolverAgentHint: "提交的工单会直接推给修复 Agent：不选 Worker，也不轮询。",
    resolverReady: "已就绪",
    resolverNotReady: "未就绪",
    resolverReadyHint: "已连接 Cybion：工单提交后会用这个账号创建修复 Thread。",
    resolverNeedCybion: "先到设置里连接 Cybion 账号，提交的工单才能推给修复 Agent。",
    resolverCardDraft: "提交后会自动推给修复 Agent。",
    resolverCardDispatched: "已推送到修复 Thread。",
    resolverCardNotDispatched: "还没有推送成功，可以重新派发。",
    resolverThreadLabel: "修复 Agent 的 Thread",
    resolverThreadOwnerNote: "这个 Thread 属于项目所有者的 Cybion 账号，只有所有者能打开。",
    guideIntro:
      "这个工单系统默认的使用者是 AI Agent：一半在报障侧查现场，一半在修复侧解决问题。两边都通过 HTTP API 说话。",
    guideReporterTitle: "报障 Agent",
    guideReporterBody:
      "报障人连接 Cybion 后，工单会自动获得一个 Cybion Thread。系统把「工单 + 调查指令 + 一次性令牌」交给它，它在报障人的机器上只读排查，再把现场证据写回工单。报障人也可以在工单页的现场调查面板里直接和它对话。",
    guideReporterScope: "令牌只绑这张工单：能读工单、能写回证据，不能关闭。",
    guideResolverTitle: "修复 Agent",
    guideResolverBody:
      "报障人提交工单的那一刻，Tickets 用项目所有者的 Cybion 账号开一个修复 Thread，把工单、现场证据和结论接口一起推给它。它修完写下结论，工单在同一个调用里关闭。",
    guideResolverScope: "令牌只绑这张工单：能读工单、能写结论并关闭。",
    guideFlowTitle: "一张工单的旅程",
    guideFlowNote:
      "没有队列、没有轮询、也没有定时任务：提交就是一次 HTTP 调用，修复 Agent 由 Cybion 直接唤醒；重新打开工单会把同一条 Thread 再推一次。",
    guideTokenNote: "两边各拿一枚一次性令牌，都只绑定自己那张工单：报障侧只能写证据，修复侧只能写结论。",
    settingsCybionTitle: "Cybion 账号",
    settingsCybionDescription:
      "连接你自己的 Cybion 后，报障时系统会用你的账号开一个专属 Thread，在你自己机器上做现场调查；作为项目所有者，提交到你项目的工单也用它来开修复 Thread。",
    settingsCybionConnected: "已连接",
    settingsCybionMissing: "未连接",
    settingsKeyStored: "已保存的密钥",
    settingsCybionAutoHint:
      "点击即可：系统用你的登录授权在 Cybion 创建一把专用 API Key 并加密保存，不需要手动粘贴。",
    settingsIdentity: "身份",
    settingsLinkit: "Linkit 账号",
    settingsLinkedAccountBody: "本系统使用 Auth Mini 统一登录，语言与主题偏好保存在 Linkit 个人资料里。",
    loading: "加载中…",
    errorLoadFailed: "加载失败",

    timeJustNow: "刚刚",
    timeMinutesAgo: "{n} 分钟前",
    timeHoursAgo: "{n} 小时前",
    timeDaysAgo: "{n} 天前",
  },
  en: {
    appName: "Tickets",
    navWorkbench: "Work",
    navProjects: "Projects",
    navMyTickets: "My tickets",
    navAgent: "Agent guide",
    navAgentGroup: "Agent",
    navAccount: "Account",
    navSettings: "Settings",

    actionAddMember: "Add member",
    actionCancel: "Cancel",
    actionClose: "Close ticket",
    actionConnect: "Connect Cybion",
    actionCopy: "Copy",
    actionCreateProject: "New project",
    actionCreateThread: "Create thread",
    actionCreateTicket: "New ticket",
    actionDisconnect: "Disconnect",
    actionDispatch: "Dispatch again",
    actionManaging: "Project settings",
    actionOpenThread: "Open in Cybion",
    actionRemoveMember: "Remove",
    actionReopen: "Reopen",
    actionRetry: "Retry",
    actionSave: "Save",
    actionStartInvestigation: "Start investigating",
    actionSubmit: "Submit ticket",

    stateOpen: "Open",
    stateClosed: "Closed",
    stateDraft: "Draft",
    stateAll: "All",
    projectsDescription:
      "Each project is one ticket queue. A private project is visible to you and the members you grant; a public one is visible to every signed-in user.",
    projectsEmpty: "No projects yet",
    projectsEmptyHint:
      "Create a project to start collecting tickets. Submitted tickets are pushed straight to the owner's resolver Agent.",
    projectOpenCount: "{n} open",
    projectTotalCount: "{n} tickets",
    projectNameLabel: "Project name",
    projectDescriptionLabel: "Description",
    projectVisibilityLabel: "Visibility",
    projectVisibilityPrivate: "Private",
    projectVisibilityPublic: "Public",
    projectVisibilityPrivateHint:
      "Only you and the members you grant can see this project, its tickets and their evidence.",
    projectVisibilityPublicHint:
      "Every signed-in user can see the project, browse its tickets and file new ones.",
    projectMembersLabel: "Members",
    projectMembersHint:
      "These members can open this private project. The resolver Agent still belongs to the owner alone.",
    projectMembersEmpty: "No members yet.",
    projectMemberSearchLabel: "Find a user",
    projectMemberSearchHint:
      "Search the Linkit directory by username or UUID. The user does not need to have signed in to Tickets before.",
    projectMemberSelf: "The project owner already has access.",
    projectMemberAlready: "This user is already a member.",
    projectMemberBadge: "Granted",

    ticketsEmptyOpen: "No open tickets",
    ticketsEmptyClosed: "No closed tickets yet",
    ticketsEmptyAll: "No tickets yet",
    ticketsEmptyDraft: "No drafts",
    ticketNewTitle: "New ticket",
    ticketNewHeading: "Tell me what went wrong",
    ticketNewHint:
      "Sending creates a draft and starts a Cybion investigation thread on your own machine. The first message becomes the title.",
    ticketNewPlaceholder:
      "For example: since 10:00 today, payment callbacks dropped from 99% to 93% success, and the logs show upstream 30s timeouts.\n\nRough is fine — the Agent will fill in the scene with you here.",
    ticketNewStepsTitle: "What happens next",
    ticketNewStepOne: "This message becomes the ticket body; its first line becomes the title.",
    ticketNewStepTwo: "Your Cybion thread starts investigating and streams into the ticket page.",
    ticketNewStepThree: "Submit once the picture is clear; the resolver Agent is pushed right then.",
    ticketNewPrivacy: "Until you submit, only you can see this ticket and this conversation.",
    ticketDraftBanner:
      "This is a draft only you can see. Let the investigation thread inspect the scene, then submit it to the resolver Agent.",
    ticketReporter: "Reporter",
    ticketOpened: "Opened",
    ticketClosedBy: "Closed by {name}",
    ticketEvidenceTitle: "Field evidence",
    ticketEvidenceMissing: "The reporter Agent has not written any evidence yet.",
    ticketEvidenceBadge: "Evidence",
    ticketResolutionTitle: "Resolution",
    ticketResolutionMissing: "The resolver Agent has not concluded yet.",
    ticketResolutionBadge: "Resolved",
    ticketSidebarReporterAgent: "Investigation thread",
    ticketSidebarResolverAgent: "Resolver Agent",
    ticketSidebarAgentApi: "Agent API",

    threadMissing: "No investigation thread yet",
    threadMissingHint:
      "This ticket has no investigation thread. It is created by the reporter's Cybion account.",
    threadMissingDraftHint:
      "Opening a draft starts its investigation thread automatically; the agent inspects your own machine and writes the findings back into this draft.",
    threadMissingOpenHint: "A submitted ticket should get its thread automatically. Create it again here.",
    threadConnectHint: "Cybion not connected yet? Connect it in Settings, then try again.",
    threadTokens: "{n} tokens",
    threadOwnerNote:
      "This thread belongs to the reporter's Cybion account and runs on their machine. Only they can open it.",
    threadLiveTitle: "Live investigation",
    threadLiveVisibility: "Only you can see this. Investigate with the Agent, then confirm by submitting.",
    threadLiveStarting: "Starting the investigation thread…",
    threadLiveComposerHint: "Enter to send · Shift+Enter for a new line",
    threadLiveClosed: "The ticket is closed; the investigation cannot continue.",
    threadLiveFailed: "The investigation panel failed to load. Try refreshing the page.",

    resolverAgentHint: "Submitted tickets are pushed to the resolver Agent — no Worker, no polling.",
    resolverReady: "Ready",
    resolverNotReady: "Not ready",
    resolverReadyHint: "Cybion is connected: submitted tickets open a resolver thread on this account.",
    resolverNeedCybion: "Connect your Cybion account in Settings so submitted tickets can reach the resolver Agent.",
    resolverCardDraft: "Submitting pushes this ticket to the resolver Agent.",
    resolverCardDispatched: "Pushed to the resolver thread.",
    resolverCardNotDispatched: "Not pushed yet — dispatch it again.",
    resolverThreadLabel: "Resolver thread",
    resolverThreadOwnerNote: "This thread belongs to the project owner's Cybion account; only they can open it.",
    guideIntro:
      "This tracker's users are Agents: one half investigates on the reporter's side, the other half fixes on the resolver's side. Both talk HTTP.",
    guideReporterTitle: "Reporter Agent",
    guideReporterBody:
      "Once a reporter connects Cybion, a ticket gets a Cybion thread automatically. Tickets hands it the ticket, the investigation brief and a one-off token; it inspects the reporter's machine read-only and writes the evidence back. The reporter can also talk to it from the live investigation panel.",
    guideReporterScope: "The token is bound to this ticket: read it, write evidence, nothing else.",
    guideResolverTitle: "Resolver Agent",
    guideResolverBody:
      "The moment a reporter submits, Tickets opens a resolver thread on the project owner's Cybion account and pushes the ticket, the evidence and the resolve endpoint into it. The Agent fixes it, writes its conclusion, and the ticket closes in that same call.",
    guideResolverScope: "The token is bound to this ticket: read it, write the conclusion and close.",
    guideFlowTitle: "The life of a ticket",
    guideFlowNote:
      "No queue, no polling, no scheduled job: submitting is one HTTP call, and Cybion wakes the resolver Agent directly. Reopening pushes the same thread again.",
    guideTokenNote:
      "Each side holds one one-off token bound to its own ticket: the reporter side writes evidence, the resolver side writes the conclusion.",
    settingsCybionTitle: "Cybion account",
    settingsCybionDescription:
      "Connect your own Cybion: reporting gets a dedicated thread that inspects your machine, and as a project owner the same account opens the resolver thread for tickets submitted to your projects.",
    settingsCybionConnected: "Connected",
    settingsCybionMissing: "Not connected",
    settingsKeyStored: "Stored key",
    settingsCybionAutoHint:
      "One click: Tickets mints a dedicated Cybion API key with your sign-in and stores it encrypted. Nothing to paste.",
    settingsIdentity: "Identity",
    settingsLinkit: "Linkit account",
    settingsLinkedAccountBody: "Sign-in runs on Auth Mini; language and theme live in your Linkit profile.",
    loading: "Loading…",
    errorLoadFailed: "Failed to load",

    timeJustNow: "just now",
    timeMinutesAgo: "{n}m ago",
    timeHoursAgo: "{n}h ago",
    timeDaysAgo: "{n}d ago",
  },
} as const

export type Lang = keyof typeof copy
export type CopyKey = keyof (typeof copy)["zh"]

export function resolveLang(raw: string | undefined | null): Lang {
  if (!raw) return "zh"
  return raw.toLowerCase().startsWith("en") ? "en" : "zh"
}

export function translate(lang: Lang, key: CopyKey, vars?: Record<string, string | number>): string {
  const template: string = copy[lang][key] ?? copy.zh[key] ?? key
  if (!vars) return template
  return template.replace(/\{(\w+)\}/g, (_, name: string) => String(vars[name] ?? `{${name}}`))
}
