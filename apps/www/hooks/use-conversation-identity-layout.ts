import { useLayoutEffect, useRef } from "react";

/**
 * Runs `onIdentityChange` only when `identity` changes — not on mount and not when
 * unrelated parent state causes re-renders. Callback is read from a ref so it does not
 * need to sit in the effect dependency array.
 */
export function useConversationIdentityLayout(
  identity: string,
  onIdentityChange: () => void,
): void {
  const identityRef = useRef(identity);
  const onChangeRef = useRef(onIdentityChange);
  onChangeRef.current = onIdentityChange;

  useLayoutEffect(() => {
    if (identityRef.current === identity) {
      return;
    }
    identityRef.current = identity;
    onChangeRef.current();
  }, [identity]);
}
