import { useEffect, useRef, useState } from "react";
import type { Phase } from "../engine/types";

/** A mirrored loudness trail, not a frequency spectrum. Silence stays still. */
export function AudioVisualization({
  phase,
  level,
  demo,
}: {
  phase: Phase;
  level: number;
  demo: boolean;
}) {
  const current = useRef(level);
  current.current = level;
  const [trail, setTrail] = useState(() => Array<number>(31).fill(0));
  const [reduced, setReduced] = useState(
    () => matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  useEffect(() => {
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    const change = () => setReduced(media.matches);
    media.addEventListener("change", change);
    return () => media.removeEventListener("change", change);
  }, []);
  useEffect(() => {
    setTrail(Array<number>(31).fill(0));
    if (phase !== "recording") return;
    if (reduced) {
      setTrail(Array<number>(31).fill(0.2));
      return;
    }
    let tick = 0;
    const timer = setInterval(() => {
      const sample = demo
        ? Math.max(0, Math.sin(++tick * 0.19) * 0.35 + Math.sin(tick * 0.43) * 0.2 + 0.35)
        : Math.min(1, Math.max(0, current.current));
      setTrail((previous) => [sample, ...previous.slice(0, 30)]);
    }, 60);
    return () => clearInterval(timer);
  }, [phase, demo, reduced]);
  return (
    <div className={`audio-visualization ${phase === "processing" ? "audio-processing" : ""}`}>
      <svg viewBox="0 0 488 240" className="w-full overflow-visible" aria-hidden="true">
        {Array.from({ length: 61 }, (_, i) => {
          const distance = Math.abs(i - 30);
          const height = 3 + trail[distance] * 190 * Math.exp((-distance * distance) / 650);
          return (
            <rect
              key={i}
              x={i * 8 + 2}
              y={120 - height / 2}
              width="3"
              height={height}
              rx="1.5"
              style={{
                opacity: 0.25 + (1 - distance / 32) * 0.75,
                animationDelay: `${distance * -45}ms`,
              }}
            />
          );
        })}
      </svg>
    </div>
  );
}
