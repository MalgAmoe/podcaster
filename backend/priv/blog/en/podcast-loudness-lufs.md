%{
  title: "LUFS for Podcasters: Why Your Episodes Sound Different on Every Platform",
  description: "Your podcast sounds great on your laptop but quiet on Spotify and loud on Apple Podcasts. Here's the one number that explains why — and how to fix it.",
  date: ~D[2026-03-09]
}
---

You publish an episode. It sounds fine on your computer. Then a listener messages you: "Hey, your show is really quiet — I have to crank the volume all the way up." Another listener on a different app says it sounds fine. What's going on?

The answer is a unit of measurement called LUFS, and once you understand it, your podcast will sound consistent everywhere.

## What is LUFS?

LUFS stands for Loudness Units relative to Full Scale. It measures how loud audio *sounds* to human ears — not just how high the waveform peaks are.

This distinction matters. A whisper and a shout can have the same peak level if the shout is brief and the whisper is sustained. But they don't sound equally loud. LUFS accounts for this by measuring loudness over time, weighted to match human hearing.

Peak level tells you the tallest wave in the ocean. LUFS tells you how rough the sea actually is.

## Why podcasters need to care

Every major podcast platform has a loudness target. If your episode doesn't match, the platform adjusts it — sometimes well, sometimes badly.

| Platform | Target | What happens if you're off |
|----------|--------|--------------------------|
| **Spotify** | -14 LUFS | Normalizes to -14. Too quiet = turned up (with noise). Too loud = turned down. |
| **Apple Podcasts** | -16 LUFS | Sound Check normalizes if enabled by the listener. Inconsistent. |
| **YouTube** | -14 LUFS | Normalizes down but won't normalize up. Quiet stays quiet. |
| **Overcast** | Voice Boost | Has its own processing. Applies compression and volume boost regardless. |

The problem: if you master to -20 LUFS (common for unprocessed recordings) and a listener is on Spotify, your episode gets cranked up — and every bit of background noise comes with it.

## The -16 LUFS target

Most podcast professionals recommend **-16 LUFS**. It's become the widely adopted standard for spoken word:

- On Spotify (-14 target), your episode is turned down by 2 dB — barely noticeable.
- On Apple Podcasts (-16 target), it matches perfectly.
- On YouTube (-14 target), it's turned down slightly — fine.
- It leaves enough headroom that your audio won't sound squashed or over-compressed.

This matters more for voice than for music. Music is already heavily compressed and mastered, so pushing it to -14 LUFS is fine. Voice is more dynamic — it has natural loud and quiet moments that you want to preserve. Pushing voice to -14 LUFS requires more compression, which can make it sound flat and unnatural. -16 gives voice room to breathe while still sounding present and professional.

## True peak: the other number that matters

LUFS measures average loudness. True peak (dBTP) measures the absolute maximum level, including peaks that happen between samples.

If your true peak is at 0 dBTP (maximum), when the platform re-encodes your file or the listener's device reconstructs the signal, those peaks can exceed 0 dB and distort. You hear it as a brief crackle on loud consonants.

**The safe limit: -1 dBTP.** Set your limiter's ceiling to -1.0 dBTP. This gives enough headroom for codec conversion and playback on any device.

## How to measure LUFS

You need a loudness meter:

- **Youlean Loudness Meter** (free plugin) — Shows integrated LUFS, short-term, momentary, true peak, and loudness range. The best free option.
- **ffmpeg** (free, command line) — `ffmpeg -i your_file.mp3 -af loudnorm=print_format=json -f null -` gives you integrated LUFS and true peak.
- **Most DAWs** have built-in loudness metering or support loudness meter plugins.

## How to hit your target

### Manual approach

1. **Process your audio first** — noise reduction, compression, EQ, everything else before loudness.
2. **Add a limiter** on the output. Set the ceiling to -1.0 dBTP.
3. **Normalize to -16 LUFS.** Most DAWs have a loudness normalization function.
4. **Check with a meter.** Play the full episode and read the integrated LUFS value. It should be within 1 dB of your target.

### Automated

Tools like Auphonic, Munchy Cow, and others can handle loudness targeting automatically. The advantage of doing it as part of a full processing chain (noise removal, compression, EQ) rather than just normalizing raw audio is that you're not just turning up the bad stuff along with the good.

## The most common mistakes

### Normalizing without processing first

If your raw audio is at -24 LUFS and you just normalize it to -16, you're turning everything up by 8 dB — including the noise, the reverb, and the mouth clicks. Process first, normalize last. Not sure what "process first" means in practice? Here's [the full chain a professional engineer runs](/blog/podcast-audio-cleanup).

### Over-compressing to hit the number

Heavy compression can get you to -16 LUFS, but it sounds flat and fatiguing. Use gentle compression (2:1 to 4:1 ratio) and let the normalizer bring up the overall level naturally.

### Ignoring true peak

You hit -16 LUFS but your true peak is at 0 dBTP. Listeners hear distortion on certain devices. Always use a true peak limiter set to -1.0 dBTP as the last thing in your chain.

### Inconsistent episode-to-episode loudness

Episode 1 is at -14 LUFS. Episode 2 is at -18 LUFS. Episode 3 is at -15 LUFS. Listeners have to adjust their volume every episode. Pick a target and stick to it.

## Quick reference

- **Target: -16 LUFS** integrated loudness
- **True peak ceiling: -1.0 dBTP**
- **Process before you normalize** — never just turn up raw audio
- **Be consistent** across every episode

---

Don't want to deal with meters and limiters? [Upload to Munchy Cow](/) and it handles loudness targeting automatically — along with the full processing chain that makes it actually sound good at that level. 3 free hours, no credit card.
