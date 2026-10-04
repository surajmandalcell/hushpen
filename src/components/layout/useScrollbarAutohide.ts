import { useCallback, useEffect, useRef, useState } from "react";

export function useScrollbarAutohide(resetKey: unknown) {
  const [isScrollbarVisible, setIsScrollbarVisible] = useState(true);
  const scrollbarTimerRef = useRef<number | null>(null);

  const showScrollbarTemporarily = useCallback((durationMs = 1200) => {
    setIsScrollbarVisible(true);
    if (scrollbarTimerRef.current) {
      window.clearTimeout(scrollbarTimerRef.current);
    }

    scrollbarTimerRef.current = window.setTimeout(() => {
      setIsScrollbarVisible(false);
      scrollbarTimerRef.current = null;
    }, durationMs);
  }, []);

  useEffect(() => {
    showScrollbarTemporarily(1600);
  }, [resetKey, showScrollbarTemporarily]);

  useEffect(() => () => {
    if (scrollbarTimerRef.current) {
      window.clearTimeout(scrollbarTimerRef.current);
    }
  }, []);

  const handleScrollActivity = useCallback(() => {
    showScrollbarTemporarily(1100);
  }, [showScrollbarTemporarily]);

  return { isScrollbarVisible, handleScrollActivity };
}
