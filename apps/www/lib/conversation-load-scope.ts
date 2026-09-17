/** Monotonic scope for conversation loads; stale async work must not commit after identity changes. */
export type LoadScopeRef = { current: number };

export function createLoadScopeRef(): LoadScopeRef {
  return { current: 0 };
}

export function bumpLoadScope(scope: LoadScopeRef): number {
  scope.current += 1;
  return scope.current;
}

export function isActiveLoadScope(scope: LoadScopeRef, generation: number): boolean {
  return scope.current === generation;
}
