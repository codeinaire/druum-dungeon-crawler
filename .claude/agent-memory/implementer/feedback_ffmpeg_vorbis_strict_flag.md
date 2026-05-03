---
name: ffmpeg 8.x — native Vorbis encoder requires -strict -2
description: Recent homebrew ffmpeg builds do not include libvorbis; use -c:a vorbis -strict -2 to encode .ogg files for Bevy/rodio/lewton
type: feedback
---

The recipe `ffmpeg -f lavfi -i anullsrc=... -c:a libvorbis -q:a 0 out.ogg` **fails** on homebrew ffmpeg 8.x — the `libvorbis` external encoder is not included.

Use the native vorbis encoder with the experimental flag instead:

```bash
ffmpeg -f lavfi -i anullsrc=r=44100:cl=stereo -t 1 -c:a vorbis -strict -2 out.ogg
```

**Why:** The native `vorbis` encoder in ffmpeg 8.x is marked experimental and requires `-strict -2`. Without it, ffmpeg exits with "Experimental feature" error and produces a zero-byte file.

**How to apply:** Any time generating silent .ogg placeholder files for Bevy audio (Feature #6 recipe, Feature #25 replacements). Also: do NOT use `-c:a libopus` for .ogg — rodio's lewton is a Vorbis-only decoder; Opus-encoded .ogg will fail to decode at runtime.

Verified: files produced with `-c:a vorbis -strict -2` are accepted by bevy_audio/rodio/lewton (Vorbis magic `OggS` present, ~4.7 KB for 1-second 44.1kHz stereo silent track).
