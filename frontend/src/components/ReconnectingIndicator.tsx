export function ReconnectingIndicator() {
  return (
    <div
      style={{
        position: "fixed",
        bottom: "1rem",
        right: "1rem",
        zIndex: 9998,
        display: "flex",
        alignItems: "center",
        gap: "0.5rem",
        padding: "0.4rem 0.75rem",
        borderRadius: "var(--radius-3)",
        background: "var(--accent-9)",
        color: "white",
        fontSize: "0.8rem",
        fontWeight: 500,
        boxShadow: "var(--shadow-3)",
      }}
    >
      <div
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: "white",
          opacity: 0.9,
          animation: "pulse 1.2s ease-in-out infinite",
        }}
      />
      <style>{`@keyframes pulse { 0%,100%{opacity:.9} 50%{opacity:.3} }`}</style>
      Reconnecting…
    </div>
  );
}
