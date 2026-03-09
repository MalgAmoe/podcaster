%{
  title: "How to Rescue a Bad Podcast Recording",
  description: "Your episode sounds terrible. Before you re-record or trash it, here's what's actually fixable — and what isn't.",
  date: ~D[2026-03-08]
}
---

You just finished recording a great episode. The conversation was perfect. Then you listen back and something is wrong. Maybe everything is wrong.

Before you panic, re-record, or throw it away — most recording problems are fixable in post-production. Some aren't. Here's how to tell the difference.

## Problems you can fix

### Background noise

Air conditioning hum, computer fans, traffic outside, refrigerator buzz. You didn't notice it while recording because your brain filtered it out. The microphone didn't.

**How bad can it be and still be fixable?** Pretty bad. Modern spectral noise reduction can remove steady-state noise (consistent hums, hisses, fan noise) almost completely without affecting your voice. Even moderate noise cleans up well.

**Tools:** iZotope RX is the industry standard for manual repair. Audacity has basic noise reduction that works for mild cases. Adobe Podcast Enhance Speech uses AI and handles noise well but can sound processed. Munchy Cow runs automated spectral noise removal tuned to each file.

### Volume problems

One person is loud, the other is quiet. Or the volume jumps around because someone keeps moving away from the mic.

**The fix:** Compression and normalization. A compressor evens out the dynamics. Then normalization sets the overall level.

If you have separate tracks for each speaker (you should — always record multitrack), you can level each person independently. This gives much better results than fixing a single mixed track.

### Mouth clicks and pops

Those wet clicking sounds between words, or the hard plosive pops on P and B sounds. More noticeable in headphones, and once you hear them, you can't unhear them.

**The fix:** De-clicking algorithms detect and remove these automatically. For plosives, a high-pass filter at 80 Hz removes the low-frequency thump without affecting the voice.

### Sibilance

Harsh, piercing S and SH sounds. Some microphones and voices are worse than others. It causes listener fatigue.

**The fix:** A de-esser detects sibilant frequencies (usually 4-8 kHz) and reduces them automatically.

### Dull or muddy sound

Your recording sounds like you're talking through a blanket. Usually from being too far from the mic, a low-quality microphone, or room reflections muddying the low-mid frequencies.

**The fix:** Corrective EQ can cut the muddy 200-400 Hz buildup and add presence in the 2-5 kHz range. The difference is dramatic — it's like wiping fog off a window.

### Low-level clipping

Brief moments where the audio distorted because the input was too hot. If it's occasional — a loud laugh or an excited shout — it's usually fixable.

**The fix:** De-clipping algorithms reconstruct the waveform peaks that were cut off. Works well for brief clips.

## Problems that are hard to fix

### Heavy room reverb

This is the big one. If your recording sounds like you're in a bathroom or a large empty room — that's extremely difficult to remove cleanly.

**Why it's hard:** Reverb is your voice mixed with hundreds of reflections of your voice, all at slightly different delays and frequencies. Trying to remove it is like trying to un-stir cream from coffee.

**What's realistic:**
- Mild room sound (small office, bedroom): De-reverb handles this well.
- Moderate reverb (large room, hard floors): Noticeable improvement but not perfect.
- Heavy reverb (tile bathroom, empty garage): No software fixes this cleanly. Consider re-recording.

**The real fix:** Treat your recording space. Blankets, rugs, foam panels, even recording in a closet full of clothes.

### Severe clipping

If the entire recording is distorted — the levels were way too hot the whole time — there's no saving it. De-clipping works on occasional peaks, not sustained distortion. The original waveform data is gone.

**Prevention:** Record at -12 to -6 dBFS peaks. Leave headroom. You can always make quiet audio louder, but you can't un-destroy clipped audio.

### Crosstalk and bleed

When one person's microphone picks up the other person's voice, you get a doubled, phasy sound. No reliable automated fix exists.

**Prevention:** Use closed-back headphones, keep mics close to mouths and far from each other, and always record separate tracks.

## The rescue workflow

If you have a bad recording that you need to save, here's the order of operations:

1. **Assess honestly.** Listen to the worst 30 seconds. Is there heavy reverb or sustained clipping? If yes, consider whether re-recording is better.
2. **Fix the fixable stuff first.** Noise removal, de-reverb (if mild), de-clicking — before any other processing.
3. **Even out dynamics.** Compression and leveling come after cleanup.
4. **Shape the tone.** Corrective EQ to fix frequency problems, then enhancement to add clarity.
5. **Set final levels.** Loudness normalization and limiting last.

The order matters — EQ before noise removal makes the noise harder to remove. Compression before de-clicking makes the clicks louder.

## The automated option

You can do all of this manually in a DAW with the right plugins. If you know what you're doing, you'll get great results. If you don't, you might make things worse — too much noise reduction sounds robotic, wrong EQ settings sound hollow, over-compression sounds squashed.

[Munchy Cow](/users/log-in) runs this entire chain automatically. Every file is analyzed first — noise floor, reverb characteristics, spectral balance, dynamics — then each processing stage is tuned to what was found.

3 free hours. No credit card. Upload your worst recording and see what comes back.

## How to avoid needing rescue next time

- **Get close to the mic.** 6-12 inches. The single biggest improvement most people can make.
- **Use a dynamic mic** if your room isn't treated. They reject more room sound than condensers.
- **Record separate tracks.** Always. It gives you infinitely more control in post.
- **Monitor with closed-back headphones.** So you hear problems as they happen.
- **Check your levels before you start.** Aim for peaks around -12 dBFS.
- **Do a 30-second test recording** and listen back before the real thing. This catches 90% of problems.
