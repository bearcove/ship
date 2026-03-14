import { useEffect } from "react";

type SwipeDirection = "left" | "right";

export function useSwipeGesture(
  el: HTMLElement | null,
  onSwipe: (direction: SwipeDirection) => void,
): void {
  useEffect(() => {
    if (!el) return;

    let startX = 0;
    let startY = 0;
    let locked: "horizontal" | "vertical" | null = null;

    function onTouchStart(e: TouchEvent) {
      const touch = e.touches[0];
      startX = touch.clientX;
      startY = touch.clientY;
      locked = null;
    }

    function onTouchMove(e: TouchEvent) {
      const touch = e.touches[0];
      const dx = touch.clientX - startX;
      const dy = touch.clientY - startY;

      if (locked === null && (Math.abs(dx) > 5 || Math.abs(dy) > 5)) {
        locked = Math.abs(dx) > Math.abs(dy) ? "horizontal" : "vertical";
      }

      if (locked === "horizontal") {
        e.preventDefault();
      }
    }

    function onTouchEnd(e: TouchEvent) {
      if (locked !== "horizontal") return;
      const touch = e.changedTouches[0];
      const dx = touch.clientX - startX;
      if (Math.abs(dx) < 50) return;
      onSwipe(dx < 0 ? "left" : "right");
    }

    el.addEventListener("touchstart", onTouchStart, { passive: true });
    el.addEventListener("touchmove", onTouchMove, { passive: false });
    el.addEventListener("touchend", onTouchEnd, { passive: true });

    return () => {
      el.removeEventListener("touchstart", onTouchStart);
      el.removeEventListener("touchmove", onTouchMove);
      el.removeEventListener("touchend", onTouchEnd);
    };
  }, [el, onSwipe]);
}
