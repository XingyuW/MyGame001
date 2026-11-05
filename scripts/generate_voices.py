#!/usr/bin/env python3
"""Generates the game's voice lines (assets/audio/voices/*.wav) with the Kokoro-82M speech model.

Every line is written for this game. The model and its voices are Apache-2.0
(https://huggingface.co/hexgrad/Kokoro-82M); the ONNX export used is the `model-files-v1.0`
release of kokoro-onnx (MIT). See assets/LICENSES.md and docs/DECISIONS.md (feel sprint 3).

Setup (outside the repository, nothing of it is committed):

    uv venv --python 3.12 .venv && uv pip install --python .venv/bin/python kokoro-onnx soundfile
    curl -LO https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx
    curl -LO https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin
    .venv/bin/python scripts/generate_voices.py --model-dir <dir with both files>

Each line is rendered at 24 kHz, then ffmpeg trims the silence, applies the speaker's filter
(radio band for units) and writes mono 16-bit PCM WAV at 22,050 Hz, the game's sample rate
(crates/game/src/voices.rs reads exactly that format).
"""

import argparse
import pathlib
import subprocess
import tempfile

import soundfile
from kokoro_onnx import Kokoro

# Speaker -> (Kokoro voice, speed, ffmpeg filter chain).
SPEAKERS = {
    "announcer": ("bf_emma", 1.0, "highpass=f=90,acompressor=threshold=0.2:ratio=3:attack=5:release=80"),
    "infantry": ("am_michael", 1.08,
                 "highpass=f=320,lowpass=f=3400,acompressor=threshold=0.12:ratio=6:attack=2:release=60,"
                 "volume=1.6,alimiter=limit=0.9"),
    "vehicle": ("bm_george", 1.0,
                "highpass=f=260,lowpass=f=3000,acompressor=threshold=0.1:ratio=8:attack=2:release=60,"
                "volume=1.8,alimiter=limit=0.85"),
    "enemy": ("am_onyx", 0.95,
              "highpass=f=300,lowpass=f=2800,acompressor=threshold=0.1:ratio=8:attack=2:release=60,"
              "volume=2.0,alimiter=limit=0.85,aecho=0.6:0.4:40:0.25"),
    "craft": ("am_puck", 1.05,
              "highpass=f=380,lowpass=f=3600,acompressor=threshold=0.12:ratio=6:attack=2:release=60,"
              "volume=1.6,alimiter=limit=0.9"),
}

# File stem -> (speaker, text). The stems are the names crates/game/src/voices.rs includes.
LINES = {
    # The announcer.
    "announcer_match_start": ("announcer", "Command link established. Good hunting, commander."),
    "announcer_structure_ready": ("announcer", "Structure ready for deployment."),
    "announcer_unit_ready": ("announcer", "Reinforcements have arrived."),
    "announcer_low_power": ("announcer", "Warning. Power demand exceeds supply."),
    "announcer_no_credits": ("announcer", "Credits exhausted. Production on hold."),
    "announcer_base_attacked": ("announcer", "Our base is taking fire."),
    "announcer_forces_attacked": ("announcer", "Our forces are engaged."),
    "announcer_structure_lost": ("announcer", "We have lost a structure."),
    "announcer_promotion": ("announcer", "Field promotion granted."),
    "announcer_superweapon_ready": ("announcer", "Our superweapon is charged and ready."),
    "announcer_enemy_superweapon": ("announcer", "Warning. The enemy has built a superweapon."),
    "announcer_enemy_launch": ("announcer", "Alert! Enemy superweapon launched."),
    "announcer_victory": ("announcer", "The battlefield is ours. Well done, commander."),
    "announcer_defeat": ("announcer", "Our command has fallen."),
    # Soldiers.
    "infantry_select_1": ("infantry", "Standing by."),
    "infantry_select_2": ("infantry", "Squad ready."),
    "infantry_select_3": ("infantry", "What's the plan, sir?"),
    "infantry_move_1": ("infantry", "On the double!"),
    "infantry_move_2": ("infantry", "Heading there now."),
    "infantry_move_3": ("infantry", "Copy, relocating."),
    "infantry_attack_1": ("infantry", "Open fire!"),
    "infantry_attack_2": ("infantry", "Taking them down!"),
    "infantry_attack_3": ("infantry", "Engaging the enemy!"),
    # Vehicle crews.
    "vehicle_select_1": ("vehicle", "Crew ready."),
    "vehicle_select_2": ("vehicle", "Engines running."),
    "vehicle_select_3": ("vehicle", "Armour standing by."),
    "vehicle_move_1": ("vehicle", "Tracks turning."),
    "vehicle_move_2": ("vehicle", "Changing position."),
    "vehicle_move_3": ("vehicle", "On our way."),
    "vehicle_attack_1": ("vehicle", "Target locked."),
    "vehicle_attack_2": ("vehicle", "Main gun, fire!"),
    "vehicle_attack_3": ("vehicle", "Bringing the guns to bear."),
    # Aircraft and ships.
    "craft_select_1": ("craft", "Ready for tasking."),
    "craft_select_2": ("craft", "All systems green."),
    "craft_select_3": ("craft", "Awaiting coordinates."),
    "craft_move_1": ("craft", "Course laid in."),
    "craft_move_2": ("craft", "Heading to waypoint."),
    "craft_move_3": ("craft", "Under way."),
    "craft_attack_1": ("craft", "Starting attack run."),
    "craft_attack_2": ("craft", "Weapons free."),
    "craft_attack_3": ("craft", "Target in sight."),
    # The enemy command (crates/game/src/taunts.rs, same order).
    "taunt_building_1": ("enemy", "Your base is crumbling, Commander. Surrender while you can."),
    "taunt_building_2": ("enemy", "Another building falls. How predictable."),
    "taunt_building_3": ("enemy", "Our engineers will enjoy the scrap."),
    "taunt_building_4": ("enemy", "Was that meant to be a defence?"),
    "taunt_building_5": ("enemy", "We will plant our flag on your ruins."),
    "taunt_building_6": ("enemy", "Your factories make excellent target practice."),
    "taunt_loss_1": ("enemy", "Your soldiers fight bravely. It does not help them."),
    "taunt_loss_2": ("enemy", "Send more. We have plenty of ammunition."),
    "taunt_loss_3": ("enemy", "Is this the best your academy can produce?"),
    "taunt_loss_4": ("enemy", "Your army melts like snow in spring."),
    "taunt_loss_5": ("enemy", "Retreat now and we may let you live."),
}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-dir", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path,
                        default=pathlib.Path(__file__).resolve().parent.parent / "assets/audio/voices")
    parser.add_argument("--only", nargs="*", help="stems to regenerate")
    args = parser.parse_args()
    kokoro = Kokoro(str(args.model_dir / "kokoro-v1.0.onnx"), str(args.model_dir / "voices-v1.0.bin"))
    args.out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for stem, (speaker, text) in LINES.items():
            if args.only and stem not in args.only:
                continue
            voice, speed, chain = SPEAKERS[speaker]
            samples, rate = kokoro.create(text, voice=voice, speed=speed, lang="en-us" if voice[0] == "a" else "en-gb")
            raw = pathlib.Path(tmp) / f"{stem}.wav"
            soundfile.write(raw, samples, rate)
            trim = ("silenceremove=start_periods=1:start_threshold=-45dB,"
                    "areverse,silenceremove=start_periods=1:start_threshold=-45dB,areverse,"
                    "apad=pad_dur=0.06")
            subprocess.run(
                ["ffmpeg", "-loglevel", "error", "-y", "-i", str(raw),
                 "-af", f"{trim},{chain},loudnorm=I=-16:TP=-1.5,aresample=22050",
                 "-ac", "1", "-ar", "22050", "-c:a", "pcm_s16le", "-map_metadata", "-1",
                 "-fflags", "+bitexact", "-flags:a", "+bitexact",
                 str(args.out / f"{stem}.wav")],
                check=True)
            print(stem)


if __name__ == "__main__":
    main()
