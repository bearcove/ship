export function WrongPortMessage() {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 9999,
        background: "var(--color-background)",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "1rem",
        padding: "2rem",
        textAlign: "center",
      }}
    >
      <span style={{ fontSize: "2rem" }}>⚓</span>
      <h2 style={{ margin: 0, color: "var(--gray-12)", fontSize: "1.25rem", fontWeight: 600 }}>
        Wrong port
      </h2>
      <p style={{ margin: 0, color: "var(--gray-11)", maxWidth: 420, lineHeight: 1.6 }}>
        You opened Ship directly through Vite's dev server. Close this tab and open the Ship server
        URL instead — check your terminal for the correct address.
      </p>
    </div>
  );
}
