import { useEffect, useRef } from "react";

interface Props {
  analyser: AnalyserNode;
  color?: string;
}

export function Waveform({ analyser, color = "var(--accent-9)" }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef = useRef<number>(0);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dataArray = new Uint8Array(analyser.frequencyBinCount);

    function draw() {
      rafRef.current = requestAnimationFrame(draw);
      const c = canvasRef.current;
      if (!c) return;
      const cx = c.getContext("2d");
      if (!cx) return;

      const dpr = window.devicePixelRatio || 1;
      const rect = c.getBoundingClientRect();
      const w = rect.width;
      const h = rect.height;

      if (c.width !== w * dpr || c.height !== h * dpr) {
        c.width = w * dpr;
        c.height = h * dpr;
        cx.scale(dpr, dpr);
      }

      analyser.getByteFrequencyData(dataArray);

      cx.clearRect(0, 0, w, h);

      // Draw frequency bars centered vertically
      const barCount = Math.min(dataArray.length, 64);
      const gap = 1.5;
      const barWidth = Math.max(1.5, (w - gap * (barCount - 1)) / barCount);
      const totalWidth = barCount * barWidth + (barCount - 1) * gap;
      const startX = (w - totalWidth) / 2;

      // Resolve CSS variable color to actual color
      const computedColor =
        getComputedStyle(c).getPropertyValue("--waveform-color").trim() || "#e54666";

      for (let i = 0; i < barCount; i++) {
        const value = dataArray[i] / 255;
        const barHeight = Math.max(2, value * h * 0.6);
        const x = startX + i * (barWidth + gap);
        const y = (h - barHeight) / 2;

        cx.fillStyle = computedColor;
        cx.beginPath();
        cx.roundRect(x, y, barWidth, barHeight, barWidth / 2);
        cx.fill();
      }
    }

    draw();
    return () => cancelAnimationFrame(rafRef.current);
  }, [analyser]);

  return (
    <canvas
      ref={canvasRef}
      style={{
        width: "100%",
        height: "100%",
        display: "block",
        // @ts-expect-error CSS custom property
        "--waveform-color": color,
      }}
    />
  );
}
