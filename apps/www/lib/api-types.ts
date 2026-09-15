export interface ProviderStatus {
  codexInstalled: boolean;
  chatgptConnected: boolean;
  chatgptConnectionState: "connected" | "not_connected" | "unavailable";
  connectionDetail: string | null;
  chatgptPlanType: string | null;
  preferredEngine: string;
  apiFallbackConfigured: boolean;
  defaultModel: string;
  codexLoginAllowed: boolean;
}

export interface BotSummary {
  id: string;
  name: string;
  instructions: string;
  model: string;
  computerId: string | null;
  enginePreference: string;
  avatarId?: string;
}

export interface ComputerSummary {
  id: string;
  displayName: string;
  provider: string;
  state: string;
  lastUsedAt: string | null;
  providerMetadata: { provisioned: boolean };
}

export interface RunSummary {
  task: string;
  botName: string;
  createdAt: string;
  runId: string;
  requestId: string;
  botId: string;
  conversationId: string;
  status: string;
  model: string;
  computerId: string | null;
  startedAt: string | null;
  finishedAt: string | null;
  archivedAt?: string | null;
}

export interface RunDetail extends Omit<RunSummary, "task" | "botName" | "createdAt"> {
  task: string | null;
  stepCount: number;
  errorCode: string | null;
  assistantResult: string | null;
}

export interface CreateRunResponse {
  runId: string;
  requestId: string;
  conversationId: string;
  computerId: string;
  model: string;
  status: string;
}

export interface ConversationSummary {
  id: string;
  botId: string | null;
  conversationType: "direct" | "group" | string;
  name?: string | null;
  updatedAt: string;
}

export interface GroupParticipantSummary {
  botId: string;
  name: string;
  avatarId: string;
  ordinal: number;
  joinedAt: string;
  leftAt?: string | null;
}

export interface GroupConversationDetail {
  id: string;
  name: string;
  conversationType: string;
  updatedAt: string;
  participants: GroupParticipantSummary[];
}

export interface MessageRouting {
  mode: string;
  status: string;
  errorCode?: string | null;
}

export interface MessageRecipient {
  botId: string;
  botName: string;
  avatarId: string;
  routingKind: string;
  status: string;
  runId?: string | null;
}

export interface TranscriptMessage {
  id: string;
  sequence: number;
  role: string;
  body: string;
  status: string;
  authorKind: "human" | "bot" | "system" | string;
  authorBotId?: string | null;
  authorBotName?: string | null;
  authorAvatarId?: string | null;
  createdAt: string;
  runId?: string | null;
  recipients?: MessageRecipient[];
  routing?: MessageRouting | null;
}

export interface GroupListItem {
  id: string;
  name: string;
  updatedAt: string;
  participants: GroupParticipantSummary[];
  queuedRuns: number;
  workingRuns: number;
}

export interface SendGroupMessageResponse {
  message: TranscriptMessage;
  recipients: MessageRecipient[];
}

export interface CreateConversationResponse {
  id: string;
  botId: string;
}

export interface DelegationArtifactSummary {
  resultId: string;
  name: string;
  size: number;
  transferStatus: string;
  destinationPath?: string | null;
  error?: string | null;
}

export interface DelegationSummary {
  id: string;
  status: string;
  instruction: string;
  context?: string | null;
  depth: number;
  sourceBotId: string;
  sourceBotName: string;
  targetBotId: string;
  targetBotName: string;
  targetBotAvatarId?: string;
  sourceRunId: string;
  targetRunId?: string | null;
  targetRunStatus?: string | null;
  createdAt: string;
  startedAt?: string | null;
  finishedAt?: string | null;
  errorCode?: string | null;
  errorMessage?: string | null;
  returnPolicy?: string;
  sourceResumeRunId?: string | null;
  resumeStatus?: string | null;
  resumeError?: string | null;
  artifacts?: DelegationArtifactSummary[];
}

export interface RoutineRun {
  id: string;
  routineId: string;
  runId: string | null;
  scheduledFor: string;
  startedAt: string | null;
  finishedAt: string | null;
  status: string;
  errorCode: string | null;
  errorMessage: string | null;
  triggerKind: string;
  createdAt: string;
}

export interface Routine {
  id: string;
  botId: string;
  name: string;
  instructions: string;
  intervalMinutes: number;
  enabled: boolean;
  nextRunAt: string;
  lastRunId: string | null;
  lastError: string | null;
  scheduleKind: string;
  scheduleExpression: string;
  timezone: string;
  scheduleLabel: string;
  destinationConversationId: string | null;
  lastSuccessAt: string | null;
  lastFailureAt: string | null;
  consecutiveFailures: number;
  failurePolicy: string;
  recentRuns: RoutineRun[];
}
