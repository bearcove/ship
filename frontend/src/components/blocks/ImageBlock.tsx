import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { ContentBlock } from "../../generated/ship";
import {
  feedImageThumb,
  feedImageLightboxOverlay,
  feedImageLightboxImg,
} from "../../styles/session-view.css";

type ImageBlockType = Extract<ContentBlock, { tag: "Image" }>;

interface Props {
  block: ImageBlockType;
}

export function ImageBlock({ block }: Props) {
  const [open, setOpen] = useState(false);
  const [objectUrl, setObjectUrl] = useState<string | null>(null);

  useEffect(() => {
    const blob = new Blob([new Uint8Array(block.data)], { type: block.mime_type });
    const url = URL.createObjectURL(blob);
    setObjectUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [block.data, block.mime_type]);

  if (!objectUrl) return null;

  return (
    <>
      <img src={objectUrl} className={feedImageThumb} alt="" onClick={() => setOpen(true)} />
      {open &&
        createPortal(
          <div className={feedImageLightboxOverlay} onClick={() => setOpen(false)}>
            <img src={objectUrl} className={feedImageLightboxImg} alt="" />
          </div>,
          document.body,
        )}
    </>
  );
}
