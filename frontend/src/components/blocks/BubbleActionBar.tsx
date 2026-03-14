import { useState } from "react";
import { IconButton } from "@radix-ui/themes";
import { ArrowBendUpLeft, Check, CopySimple, CircleNotch, SpeakerHigh } from "@phosphor-icons/react";
import { feedBubbleActionBar } from "../../styles/session-view.css";
import { spinAnimation } from "../../styles/global.css";
import { usePlayback } from "../../context/PlaybackContext";

interface BubbleActionBarProps {
  text: string;
  speakable?: boolean;
  onReply?: () => void;
}

export function BubbleActionBar({ text, speakable, onReply }: BubbleActionBarProps) {
  const [copied, setCopied] = useState(false);
  const playback = usePlayback();

  const handleCopy = () => {
    void navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const isThisActive = playback.state !== "idle" && playback.activeText === text;
  const isBusy = playback.state !== "idle";

  return (
    <div className={feedBubbleActionBar} onClick={(e) => e.stopPropagation()}>
      <IconButton size="1" variant="ghost" onClick={handleCopy} aria-label="Copy">
        {copied ? <Check size={14} /> : <CopySimple size={14} />}
      </IconButton>
      {onReply && (
        <IconButton size="1" variant="ghost" onClick={onReply} aria-label="Reply">
          <ArrowBendUpLeft size={14} />
        </IconButton>
      )}
      {speakable && (
        <IconButton
          size="1"
          variant="ghost"
          onClick={() => isThisActive ? playback.stop() : playback.speak(text)}
          aria-label={isThisActive ? "Stop" : "Speak"}
          disabled={isBusy && !isThisActive}
        >
          {isThisActive ? (
            <CircleNotch size={14} style={{ animation: `${spinAnimation} 2.5s linear infinite` }} />
          ) : (
            <SpeakerHigh size={14} />
          )}
        </IconButton>
      )}
    </div>
  );
}
