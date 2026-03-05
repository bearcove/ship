import { createContext, useContext, useState } from "react";

interface SoundContextValue {
  soundEnabled: boolean;
  setSoundEnabled: (v: boolean) => void;
}

const SoundContext = createContext<SoundContextValue>(null!);

export function SoundProvider({ children }: { children: React.ReactNode }) {
  const [soundEnabled, setSoundEnabled] = useState(true);
  return (
    <SoundContext.Provider value={{ soundEnabled, setSoundEnabled }}>
      {children}
    </SoundContext.Provider>
  );
}

// r[ui.notify.sound-toggle]
export function useSoundEnabled() {
  return useContext(SoundContext);
}
