# Images not shown in feed

When a user attaches and sends an image, it appears as a `[image]` placeholder in the feed instead of rendering the actual image.

The `ContentBlock` type has an `Image` variant (`{ tag: 'Image'; mime_type: string; data: Uint8Array }`) but the `SingleBlock` switch in `UnifiedFeed.tsx` has no `case "Image":` handler.

Fix: add an Image case that renders `<img>` with a blob URL created from `block.data` and `block.mime_type`. The blob URL should be created once per block (e.g. via `useMemo`) and revoked on cleanup.
