%{
  title: "How to Record Clean Audio in Audacity",
  description: "Practical tips on mic setup, gain staging, room treatment, and Audacity settings to get clean recordings from the start.",
  date: ~D[2026-03-05]
}
---

The golden rule of audio is simple: garbage in, garbage out. No amount of post-processing can rescue a recording that was captured with bad technique. The good news? Spending 15 minutes on proper setup saves hours of editing and produces dramatically better results.

Here's everything you need to know to record clean audio in Audacity.

## Pick the right microphone

If you're recording in a home office or bedroom, **use a dynamic microphone**. Models like the Rode PodMic, Shure SM58, or Samson Q2U are affordable and naturally reject background noise. They're less sensitive than condenser mics, which is exactly what you want in an untreated room.

Condenser microphones capture more detail, but they also pick up every keyboard click, fan hum, and passing car. Save those for acoustically treated spaces.

Whatever mic you choose, make sure it has a **cardioid pickup pattern**. Cardioid mics reject sound from the sides and rear, keeping room noise out of your recording.

## Get close to the mic

This is the single biggest improvement most people can make. Position yourself **6 to 12 inches (15-30 cm)** from the microphone. A quick rule of thumb: about four finger-widths from the capsule.

Dynamic mics can be worked even closer (2-3 inches / 5-8 cm) for a warmer, more intimate sound. But don't press your lips against it — that causes boomy low-end buildup from the proximity effect.

**Angle the mic about 30 degrees off-axis** from your mouth instead of speaking directly into it. This reduces plosive pops (those harsh P, B, and T sounds) while still capturing a full, natural tone. A pop filter helps too — grab one if you don't have one already.

## Set up Audacity correctly

Before you hit record, configure these settings:

**Sample rate:** 44,100 Hz is the standard for spoken word. Use 48,000 Hz if the audio will accompany video. There's no practical benefit to going higher for voice.

**Bit depth:** Keep Audacity's default of 32-bit float for recording and editing. This gives you massive headroom and a very low noise floor. Export at 16-bit for your final file.

**Channels:** Set recording to **1 (Mono)**. The human voice is a single source — stereo doubles file size with no benefit. A one-hour mono recording is about 310 MB raw, versus 620 MB in stereo.

## Nail your gain staging

This is where most beginners go wrong. Set your levels at the **hardware stage** — the gain knob on your audio interface or USB mic — not in software.

Here's the process:

1. In Audacity, click the microphone icon in the meter toolbar and select **Start Monitoring**
2. Speak at your normal recording volume
3. Adjust the input gain until peaks land between **-12 dBFS and -6 dBFS**
4. Leave enough headroom for when you laugh or get excited — you never want peaks hitting 0 dBFS

![Resize Audacity's input meter by dragging its edge](/images/blog/resize-meter.png)

**Tip:** Drag the edge of Audacity's input meter to make it wider — the default size is tiny and makes it hard to read the levels accurately.

Digital clipping is harsh distortion that can't be fixed. It's always better to record a little quiet than a little hot.

**A common trap:** recording too quietly and then boosting with Amplify or Normalize in post. That raises everything — including the noise floor. Capture a healthy signal at the source instead.

## Treat your room (cheaply)

You don't need a professional studio. A few simple changes make a huge difference:

**Free options:**
- Record in a **small room with soft furnishings** — carpet, curtains, a couch. A walk-in closet full of clothes is one of the best free "vocal booths" you'll find
- Hang **thick blankets or duvets** on the walls behind and beside your mic
- Put a **rug on hard floors** to reduce reflections
- Place **bookshelves filled with books** facing the mic — they naturally diffuse sound waves

**Budget options ($25-30 per panel):**
- Build DIY acoustic panels from lumber frames filled with Rockwool insulation and wrapped in fabric
- Place them at the first reflection points: behind the mic, behind you, and on the side walls

**Priority order:** Treat behind the microphone first, then behind yourself, then the side walls, then the ceiling.

## Kill the noise before recording

Noise reduction in post-processing is always a compromise. It's far better to eliminate noise at the source:

- **Turn off** air conditioning, fans, heaters, and any HVAC in the room
- **Close windows and doors** — even a cracked window lets in traffic and wind
- **Power down** unnecessary electronics — monitors, external hard drives, and desktop computers are common culprits
- If you must record near a computer, point the **rear of the mic** (its null point) toward the computer

**For electrical hum and buzz:**
- Plug all recording gear into the **same power strip** to avoid ground loops
- Use **balanced XLR cables** instead of 3.5mm aux — they reject electromagnetic interference
- If using a USB mic, plug directly into the computer — avoid USB hubs

Get these fundamentals right and your recordings will be cleaner than most podcasts out there — before you even touch a plugin.

Already recorded something that didn't turn out great? Read [How to Rescue a Bad Podcast Recording](/blog/fix-bad-podcast-recording) before you re-record.

And when you do want to polish things further, [give Munchy Cow a try](/). Upload your recording and we'll clean up whatever's left — background noise, room reverb, uneven levels — without making you sound like a robot.
