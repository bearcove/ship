import { useEffect, useRef, useState } from "react";

function getDraggedImageFiles(event: DragEvent): File[] {
  const files = event.dataTransfer?.files;
  if (!files) return [];
  return Array.from(files).filter((file) => file.type.startsWith("image/"));
}

function hasDraggedImageFiles(event: DragEvent): boolean {
  const items = event.dataTransfer?.items;
  if (items && items.length > 0) {
    return Array.from(items).some((item) => item.kind === "file" && item.type.startsWith("image/"));
  }
  return getDraggedImageFiles(event).length > 0;
}

// r[ui.composer.image-attach]
export function useDocumentDrop(
  target: HTMLElement | null,
  onDrop: (files: File[]) => void,
): boolean {
  const [counter, setCounter] = useState(0);
  const onDropRef = useRef(onDrop);
  onDropRef.current = onDrop;

  useEffect(() => {
    if (!target) {
      setCounter(0);
      return;
    }

    function handleDragEnter(event: DragEvent) {
      if (!hasDraggedImageFiles(event)) return;
      event.preventDefault();
      setCounter((current) => current + 1);
    }

    function handleDragOver(event: DragEvent) {
      if (!hasDraggedImageFiles(event)) return;
      event.preventDefault();
    }

    function handleDragLeave(event: DragEvent) {
      if (!hasDraggedImageFiles(event)) return;
      setCounter((current) => Math.max(0, current - 1));
    }

    function handleDrop(event: DragEvent) {
      if (!hasDraggedImageFiles(event)) return;
      event.preventDefault();
      setCounter(0);
      const files = getDraggedImageFiles(event);
      if (files.length > 0) {
        onDropRef.current(files);
      }
    }

    target.addEventListener("dragenter", handleDragEnter);
    target.addEventListener("dragover", handleDragOver);
    target.addEventListener("dragleave", handleDragLeave);
    target.addEventListener("drop", handleDrop);

    return () => {
      target.removeEventListener("dragenter", handleDragEnter);
      target.removeEventListener("dragover", handleDragOver);
      target.removeEventListener("dragleave", handleDragLeave);
      target.removeEventListener("drop", handleDrop);
    };
  }, [target]);

  return counter > 0;
}
