#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "pocket-tts",
#   "sounddevice",
# ]
# ///
"""
Quick standalone TTS test using pocket-tts: synthesize and play audio.
Usage:  uv run crates/ship-server/tts_play.py "Hello world"
        uv run crates/ship-server/tts_play.py --voice marius "Hello world"
"""

import sys
import time
import argparse
import sounddevice as sd


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--voice", default="alba", help="Voice name (alba, marius, javert, ...)")
    parser.add_argument("text", nargs="+")
    args = parser.parse_args()
    text = " ".join(args.text)

    from pocket_tts import TTSModel

    print("loading model...", file=sys.stderr)
    model = TTSModel.load_model()
    print(f"model ready, sample_rate={model.sample_rate}", file=sys.stderr)

    voice_state = model.get_state_for_audio_prompt(args.voice)

    print(f"synthesizing [{args.voice}]: {text!r}", file=sys.stderr)
    t0 = time.time()
    audio = model.generate_audio(voice_state, text)
    elapsed = time.time() - t0
    wav = audio.numpy()
    audio_duration = len(wav) / model.sample_rate
    print(f"done in {elapsed:.1f}s for {audio_duration:.1f}s of audio (RTF {elapsed/audio_duration:.2f}x)", file=sys.stderr)

    sd.play(wav, samplerate=model.sample_rate)
    sd.wait()


if __name__ == "__main__":
    main()
