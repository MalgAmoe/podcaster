#!/usr/bin/env python3
"""
Add random clicks/pops to audio files for testing declicker.

Usage:
    python add_clicks.py input.wav output.wav --num-clicks 20
    python add_clicks.py input.wav output.wav --num-clicks 50 --severity heavy
"""

import argparse
import random
import numpy as np
import soundfile as sf


def generate_click(sample_rate: int, click_type: str) -> np.ndarray:
    """Generate a single click/pop of various types."""

    if click_type == "spike":
        # Single sample spike (digital click)
        return np.array([random.choice([-1.0, 1.0]) * random.uniform(0.7, 1.0)])

    elif click_type == "burst":
        # Short burst (2-8 samples)
        length = random.randint(2, 8)
        amplitude = random.uniform(0.6, 1.0)
        sign = random.choice([-1, 1])
        return sign * amplitude * np.ones(length)

    elif click_type == "crackle":
        # Crackle pattern (alternating polarity)
        length = random.randint(3, 10)
        amplitude = random.uniform(0.5, 0.9)
        return amplitude * np.array([(-1)**i for i in range(length)])

    elif click_type == "pop":
        # Pop with decay (vinyl-style)
        length = random.randint(10, 30)
        amplitude = random.uniform(0.6, 1.0)
        decay = np.exp(-np.linspace(0, 4, length))
        sign = random.choice([-1, 1])
        return sign * amplitude * decay

    elif click_type == "dc_offset":
        # Sudden DC offset jump
        length = random.randint(5, 15)
        offset = random.uniform(0.3, 0.7) * random.choice([-1, 1])
        return np.full(length, offset)

    elif click_type == "glitch":
        # Random noise burst
        length = random.randint(5, 20)
        amplitude = random.uniform(0.5, 0.9)
        return amplitude * (np.random.rand(length) * 2 - 1)

    else:
        # Default: simple spike
        return np.array([random.uniform(0.7, 1.0) * random.choice([-1, 1])])


def add_clicks_to_audio(
    audio: np.ndarray,
    sample_rate: int,
    num_clicks: int,
    severity: str = "medium"
) -> tuple[np.ndarray, list[dict]]:
    """
    Add random clicks to audio.

    Returns: (modified_audio, click_info_list)
    """
    # Severity affects click type distribution
    if severity == "light":
        click_types = ["spike", "spike", "burst"]
        amplitude_scale = 0.6
    elif severity == "heavy":
        click_types = ["spike", "burst", "crackle", "pop", "glitch", "dc_offset"]
        amplitude_scale = 1.0
    else:  # medium
        click_types = ["spike", "burst", "crackle", "pop"]
        amplitude_scale = 0.8

    output = audio.copy()
    is_stereo = len(audio.shape) > 1 and audio.shape[1] == 2

    # Don't place clicks in first/last 0.5 seconds
    margin = int(sample_rate * 0.5)
    if is_stereo:
        valid_range = (margin, audio.shape[0] - margin)
    else:
        valid_range = (margin, len(audio) - margin)

    click_info = []

    for i in range(num_clicks):
        # Random position
        pos = random.randint(valid_range[0], valid_range[1])

        # Random type
        click_type = random.choice(click_types)

        # Generate click
        click = generate_click(sample_rate, click_type) * amplitude_scale
        click_len = len(click)

        # Random channel for stereo (or both)
        if is_stereo:
            channel = random.choice([0, 1, "both"])
        else:
            channel = 0

        # Insert click
        end_pos = min(pos + click_len, valid_range[1])
        actual_len = end_pos - pos

        if is_stereo:
            if channel == "both":
                output[pos:end_pos, 0] = click[:actual_len]
                output[pos:end_pos, 1] = click[:actual_len]
            else:
                output[pos:end_pos, channel] = click[:actual_len]
        else:
            output[pos:end_pos] = click[:actual_len]

        click_info.append({
            "index": i,
            "position": pos,
            "time_ms": pos / sample_rate * 1000,
            "type": click_type,
            "length": actual_len,
            "channel": channel if is_stereo else "mono"
        })

    return output, click_info


def main():
    parser = argparse.ArgumentParser(description="Add random clicks to audio for testing")
    parser.add_argument("input", help="Input audio file")
    parser.add_argument("output", help="Output audio file")
    parser.add_argument("--num-clicks", "-n", type=int, default=20, help="Number of clicks to add")
    parser.add_argument("--severity", "-s", choices=["light", "medium", "heavy"], default="medium",
                        help="Click severity (affects types and amplitude)")
    parser.add_argument("--seed", type=int, help="Random seed for reproducibility")
    parser.add_argument("--list", "-l", action="store_true", help="Print click positions")

    args = parser.parse_args()

    if args.seed is not None:
        random.seed(args.seed)
        np.random.seed(args.seed)

    # Load audio
    print(f"Loading: {args.input}")
    audio, sample_rate = sf.read(args.input)
    print(f"  Sample rate: {sample_rate} Hz")
    print(f"  Duration: {len(audio) / sample_rate:.2f}s")
    print(f"  Channels: {'stereo' if len(audio.shape) > 1 else 'mono'}")

    # Add clicks
    print(f"\nAdding {args.num_clicks} clicks (severity: {args.severity})...")
    output, click_info = add_clicks_to_audio(audio, sample_rate, args.num_clicks, args.severity)

    # Print click info
    if args.list:
        print("\nClick positions:")
        for info in click_info:
            print(f"  {info['index']:3d}: {info['time_ms']:8.1f}ms - {info['type']:8s} ({info['length']} samples) ch={info['channel']}")

    # Summary by type
    type_counts = {}
    for info in click_info:
        t = info['type']
        type_counts[t] = type_counts.get(t, 0) + 1
    print("\nClick types:")
    for t, count in sorted(type_counts.items()):
        print(f"  {t}: {count}")

    # Save
    print(f"\nSaving: {args.output}")
    sf.write(args.output, output, sample_rate)
    print("Done.")


if __name__ == "__main__":
    main()
