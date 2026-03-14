import { useEffect, type RefObject } from "react";

type SwipeDirection = "left" | "right";

export function useSwipeGesture(
  ref: RefObject<HTMLElement | null>,
  onSwipe: (direction: SwipeDirection) => void,
): void {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    let startX = 0;
    let startY = 0;

    function onTouchStart(e: TouchEvent) {
      const touch = e.touches[0];
      startX = touch.clientX;
      startY = touch.clientY;
    }

    function onTouchEnd(e: TouchEvent) {
      const touch = e.changedTouches[0];
      const dx = touch.clientX - startX;
      const dy = touch.clientY - startY;

      if (Math.abs(dx) < 50) return;
      if (Math.abs(dy) >= Math.abs(dx)) return;

      onSwipe(dx < 0 ? "left" : "right");
    }

    el.addEventListener("touchstart", onTouchStart, { passive: true });
    el.addEventListener("touchend", onTouchEnd, { passive: true });

    return () => {
      el.removeEventListener("touchstart", onTouchStart);
      el.removeEventListener("touchend", onTouchEnd);
    };
  }, [ref, onSwipe]);
}
