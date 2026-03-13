#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "pocket-tts",
# ]
# ///
"""
Pocket TTS worker for ship-server. Reads lines of text from stdin, writes raw
f32-le PCM chunks to stdout using a simple framing protocol:
  - 4-byte little-endian u32: chunk length in bytes (0 = end of utterance)
  - N bytes: f32-le PCM at 24kHz mono
A zero-length frame signals end of audio for that utterance.
"""

import struct
import sys


def write_frame(data: bytes) -> None:
    sys.stdout.buffer.write(struct.pack("<I", len(data)))
    if data:
        sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()


def main() -> None:
    import os
    voice = os.environ.get("TTS_VOICE", "marius")

    from pocket_tts import TTSModel
    print(f"[tts_worker] loading model, voice={voice}", file=sys.stderr, flush=True)
    model = TTSModel.load_model()
    voice_state = model.get_state_for_audio_prompt(voice)
    print(f"[tts_worker] ready, sample_rate={model.sample_rate}", file=sys.stderr, flush=True)

    for line in sys.stdin:
        text = line.rstrip("\n")
        if not text:
            continue
        print(f"[tts_worker] synthesizing: {text!r}", file=sys.stderr, flush=True)
        for chunk in model.generate_audio_stream(voice_state, text):
            pcm_bytes = chunk.numpy().astype("float32").tobytes()
            write_frame(pcm_bytes)
        write_frame(b"")  # end-of-utterance

    write_frame(b"")  # end-of-stream


if __name__ == "__main__":
    main()
