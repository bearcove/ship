# Switching sessions replays every event one by one

Switching sessions via the left session switcher reconnects from scratch and replays every event one by one, which is slow. Every event seems to trigger a render.

Possible approaches:
1. Batch event replay
2. A more efficient hydration/seed mechanism
3. Cache event data locally
