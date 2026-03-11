import { useEffect, useRef, useState } from "react";

// r[ui.composer.image-attach]
export function useDocumentDrop(onDrop: (files: File[]) => void): boolean {
  const [counter, setCounter] = useState(0);
  const onDropRef = useRef(onDrop);
  onDropRef.current = onDrop;

  useEffect(() => {
    function hasFiles(event: DragEvent): boolean {
      return event.dataTransfer?.types.includes("Files") ?? false;
    }

    function handleDragEnter(event: DragEvent) {
      if (!hasFiles(event)) return;
      event.preventDefault();
      setCounter((c) => c + 1);
    }

    function handleDragOver(event: DragEvent) {
      if (!hasFiles(event)) return;
      event.preventDefault();
    }

    function handleDragLeave(event: DragEvent) {
      if (!hasFiles(event)) return;
      setCounter((c) => Math.max(0, c - 1));
    }

    function handleDrop(event: DragEvent) {
      event.preventDefault();
      setCounter(0);
      const files = event.dataTransfer?.files;
      if (files && files.length > 0) {
        onDropRef.current(Array.from(files));
      }
    }

    document.addEventListener("dragenter", handleDragEnter);
    document.addEventListener("dragover", handleDragOver);
    document.addEventListener("dragleave", handleDragLeave);
    document.addEventListener("drop", handleDrop);

    return () => {
      document.removeEventListener("dragenter", handleDragEnter);
      document.removeEventListener("dragover", handleDragOver);
      document.removeEventListener("dragleave", handleDragLeave);
      document.removeEventListener("drop", handleDrop);
    };
  }, []);

  return counter > 0;
}
