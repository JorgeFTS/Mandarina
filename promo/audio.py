#!/usr/bin/env python3
"""Synthesizes the sound track of the demo video (soft effects and a quiet pad) with ffmpeg.
Usage: audio.py <output.wav>. The times are those of the scene (promo/scene.js), in seconds."""
import subprocess, sys

DUR = 29.0
clicks = [4.4, 9.5, 15.4, 17.7, 21.9, 22.8, 23.7]
thock = [5.8, 16.2]
pops = [6.3, 16.5]
whoosh = [2.4, 21.0, 25.4]
hovers = [7.3, 7.9, 8.5, 9.1, 17.1, 18.3, 19.0, 19.7]
chime = [12.3]
src = {
    "click": "aevalsrc='0.55*sin(2*PI*1500*t)*exp(-75*t)+0.25*sin(2*PI*700*t)*exp(-60*t)':d=0.12:s=48000",
    "thock": "aevalsrc='0.7*sin(2*PI*170*t)*exp(-30*t)+0.2*sin(2*PI*90*t)*exp(-18*t)':d=0.2:s=48000",
    "pop": "aevalsrc='0.5*sin(2*PI*(380+900*t)*t)*exp(-12*t)':d=0.3:s=48000",
    "hover": "aevalsrc='0.18*sin(2*PI*1900*t)*exp(-90*t)':d=0.08:s=48000",
    "whoosh": "anoisesrc=d=0.8:c=pink:a=0.5:r=48000,lowpass=f=1800,highpass=f=300,afade=t=in:d=0.3,afade=t=out:st=0.4:d=0.4,volume=0.45",
    "chime": "aevalsrc='0.4*sin(2*PI*880*t)*exp(-4*t)+0.35*sin(2*PI*1318.5*max(t-0.12,0))*exp(-4*max(t-0.12,0))*gt(t,0.12)+0.12*sin(2*PI*1760*t)*exp(-6*t)':d=1.6:s=48000",
    "pad": f"aevalsrc='0.05*(sin(2*PI*110*t)+0.8*sin(2*PI*164.8*t)+0.6*sin(2*PI*220*t)+0.45*sin(2*PI*277.2*t))':d={DUR}:s=48000,afade=t=in:d=3,afade=t=out:st={DUR - 3.5}:d=3.5,lowpass=f=900",
}
events = ([("click", t) for t in clicks] + [("thock", t) for t in thock] + [("pop", t) for t in pops]
          + [("whoosh", t) for t in whoosh] + [("hover", t) for t in hovers] + [("chime", t) for t in chime] + [("pad", 0)])
inputs, chains, labels = [], [], []
for i, (name, t) in enumerate(events):
    inputs += ["-f", "lavfi", "-i", src[name]]
    ms = int(t * 1000)
    chains.append(f"[{i}:a]adelay={ms}|{ms},apad=whole_dur={DUR}[a{i}]")
    labels.append(f"[a{i}]")
graph = ";".join(chains) + ";" + "".join(labels) + f"amix=inputs={len(events)}:normalize=0:duration=longest,alimiter=limit=0.9,atrim=0:{DUR}[out]"
cmd = ["ffmpeg", "-y", "-hide_banner", "-loglevel", "error"] + inputs + ["-filter_complex", graph, "-map", "[out]", "-ar", "48000", "-c:a", "pcm_s16le", sys.argv[1]]
result = subprocess.run(cmd, capture_output=True, text=True)
sys.exit(result.stderr or 0)
