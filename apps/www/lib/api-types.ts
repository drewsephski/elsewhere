export interface ProviderStatus {
  codexInstalled: boolean;
  chatgptConnected: boolean;
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
  runId: string;
  requestId: string;
  botId: string;
  conversationId: string;
  status: string;
  model: string;
  computerId: string | null;
  startedAt: string | null;
  finishedAt: string | null;
}

export interface RunDetail extends RunSummary {
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
