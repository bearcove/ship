import { useEffect, useMemo, useState } from "react";
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

  const objectUrl = useMemo(() => {
    const bytes = new Uint8Array(block.data);
    const blob = new Blob([bytes], { type: block.mime_type });
    return URL.createObjectURL(blob);
  }, [block.data, block.mime_type]);

  useEffect(() => {
    return () => {
      URL.revokeObjectURL(objectUrl);
    };
  }, [objectUrl]);

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
