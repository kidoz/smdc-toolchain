# SMDC Toolchain Documentation

This documentation follows [Diátaxis](https://diataxis.fr/). Every page serves one
of four needs:

| | Learning | Working |
|---|---|---|
| **Practical** | [Tutorials](tutorials/) — learn by doing | [How-to guides](how-to/) — accomplish a task |
| **Theoretical** | [Explanation](explanation/) — understand why | [Reference](reference/) — look up facts |

## Start here

- **New to the toolchain?**
  [Build your first Mega Drive ROM](tutorials/build-your-first-rom.md)
- **Have a MIDI file?**
  [Render a MIDI file to a ROM](how-to/render-a-midi-file-to-rom.md)
- **Have a ROM to inspect or patch?**
  [Round-trip a ROM through assembly](how-to/roundtrip-a-rom.md)
- **Need exact behavior or limits?**
  [MIDI sound driver reference](reference/midi-sound-driver.md)
- **Want the design story?**
  [Mega Drive MIDI sound driver design](explanation/midi-sound-driver.md)

## Writing docs in this repository

Each page belongs to exactly one quadrant. Do not mix modes on a single page;
link to the other quadrants instead.

| Quadrant | Answers | Style |
|---|---|---|
| `tutorials/` | "Teach me to do this from scratch" | Lessons with verified, end-to-end steps |
| `how-to/` | "How do I accomplish X?" | Goal-driven steps; assumes basic competence |
| `reference/` | "What exactly does X do?" | Neutral, complete, lookup-oriented facts |
| `explanation/` | "Why is it designed this way?" | Background, trade-offs, design rationale |

`screenshots/` holds images used by these pages and by the repository README.
